use super::geometry::{self, Handle};
use super::options::number_to_value;
use super::session::EditSession;
use super::snapping::{self, Guide, Snapping};
use super::tools::{GeometryKind, ToolDefinition};
use super::{Options, Point, Rect, Shape};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    Select,
    Draw(&'static ToolDefinition),
    Place,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorKind {
    Default,
    Crosshair,
    Move,
    Resize(Handle),
}

#[derive(Debug, Clone)]
enum State {
    Idle,
    Drawing {
        definition: &'static ToolDefinition,
        from: Point,
        options: Options,
        points: Vec<Point>,
    },
    Moving {
        id: u32,
        origin: Point,
        original: Shape,
    },
    Resizing {
        handle: Handle,
        original: Shape,
    },
}

#[derive(Debug, Clone)]
pub struct Interaction {
    state: State,
    pending: Option<Shape>,
    constrained: bool,
    temporary_select: bool,
    snapping: Option<Snapping>,
    guides: Vec<Guide>,
}

impl Default for Interaction {
    fn default() -> Self {
        Self {
            state: State::Idle,
            pending: None,
            constrained: false,
            temporary_select: false,
            snapping: None,
            guides: Vec::new(),
        }
    }
}

impl Interaction {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_constrained(&mut self, constrained: bool) {
        self.constrained = constrained;
    }

    pub fn set_temporary_select(&mut self, temporary: bool) {
        self.temporary_select = temporary;
    }

    #[must_use]
    pub fn acts_as_select(&self, tool: Tool) -> bool {
        tool == Tool::Select || (self.temporary_select && tool != Tool::Unsupported)
    }

    pub fn set_snapping(&mut self, snapping: Option<Snapping>) {
        self.snapping = snapping;
    }

    #[must_use]
    pub fn guides(&self) -> &[Guide] {
        &self.guides
    }

    #[must_use]
    pub fn pending(&self) -> Option<&Shape> {
        self.pending.as_ref()
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    pub fn press(
        &mut self,
        session: &mut EditSession,
        tool: Tool,
        options: &Options,
        point: Point,
        reach: f32,
    ) -> bool {
        self.cancel(session);
        if self.acts_as_select(tool) {
            return self.press_select(session, point, reach);
        }
        match tool {
            Tool::Draw(definition) => {
                self.state = State::Drawing {
                    definition,
                    from: point,
                    options: options.clone(),
                    points: vec![point],
                };
                self.pending = geometry::shape_from_anchor(definition, point, options);
                true
            }
            _ => false,
        }
    }

    fn press_select(&mut self, session: &mut EditSession, point: Point, reach: f32) -> bool {
        if let Some((id, handle)) = handle_at(session, point, reach) {
            if session.begin_change(id)
                && let Some(original) = session.shape_of(id).cloned()
            {
                self.state = State::Resizing { handle, original };
            }
            return true;
        }
        if let Some(id) = session.pick(point, reach) {
            session.select(Some(id));
            self.begin_move(session, id, point);
            return true;
        }
        session.select(None);
        false
    }

    fn begin_move(&mut self, session: &mut EditSession, id: u32, point: Point) {
        if session.begin_change(id)
            && let Some(original) = session.shape_of(id).cloned()
        {
            self.state = State::Moving {
                id,
                origin: point,
                original,
            };
        }
    }

    pub fn drag(&mut self, session: &mut EditSession, point: Point) {
        let constrained = self.constrained;
        let snapping = self.snapping;
        match &mut self.state {
            State::Idle => {}
            State::Drawing {
                definition,
                from,
                options,
                points,
            } => {
                if definition.geometry == GeometryKind::Anchor {
                    self.pending = geometry::shape_from_anchor(definition, point, options);
                } else if definition.geometry == GeometryKind::Polyline {
                    if geometry::next_stroke_point(points, point) {
                        points.push(point);
                    }
                    self.pending = geometry::shape_from_points(definition, points, options, false);
                } else {
                    let end = if constrained {
                        geometry::constrained_end(*from, point, definition.geometry)
                    } else {
                        point
                    };
                    self.pending = geometry::shape_from_drag(definition, *from, end, options);
                }
            }
            State::Moving {
                id,
                origin,
                original,
            } => {
                let moved = geometry::translated(original, point.x - origin.x, point.y - origin.y);
                let (moved, guides) = snapped(session, *id, moved, snapping);
                self.guides = guides;
                session.preview(moved);
            }
            State::Resizing { handle, original } => {
                session.preview(geometry::resized(original, *handle, point));
            }
        }
    }

    pub fn release(&mut self, session: &mut EditSession, point: Point) -> Option<u32> {
        self.drag(session, point);
        let mut created = None;
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Idle => {}
            State::Drawing {
                definition,
                options,
                points,
                ..
            } => {
                let finished = if definition.geometry == GeometryKind::Polyline {
                    geometry::shape_from_points(definition, &points, &options, true)
                } else {
                    self.pending.take()
                };
                if let Some(shape) = finished {
                    created = Some(session.add(shape));
                }
            }
            State::Moving { .. } | State::Resizing { .. } => {
                session.end_change();
            }
        }
        self.pending = None;
        self.guides.clear();
        created
    }

    pub fn straighten(
        &mut self,
        session: &mut EditSession,
        tool: Tool,
        point: Point,
        reach: f32,
    ) -> bool {
        self.cancel(session);
        self.acts_as_select(tool)
            && matches!(handle_at(session, point, reach), Some((_, Handle::Curve)))
            && session.set_option_on_selected("curve", number_to_value(0.0))
    }

    pub fn cancel(&mut self, session: &mut EditSession) {
        if matches!(self.state, State::Moving { .. } | State::Resizing { .. }) {
            session.cancel_change();
        }
        self.state = State::Idle;
        self.pending = None;
        self.guides.clear();
    }

    #[must_use]
    pub fn cursor(
        &self,
        session: &EditSession,
        tool: Tool,
        point: Point,
        reach: f32,
    ) -> CursorKind {
        if tool == Tool::Unsupported {
            return CursorKind::Default;
        }
        if !self.acts_as_select(tool) {
            return CursorKind::Crosshair;
        }
        if let Some((_, handle)) = handle_at(session, point, reach) {
            CursorKind::Resize(handle)
        } else if session.pick(point, reach).is_some() {
            CursorKind::Move
        } else {
            CursorKind::Default
        }
    }
}

fn snapped(
    session: &EditSession,
    moving: u32,
    shape: Shape,
    snapping: Option<Snapping>,
) -> (Shape, Vec<Guide>) {
    let (Some(snapping), Some(bounds)) = (snapping, geometry::shape_bounds(&shape)) else {
        return (shape, Vec::new());
    };
    let others: Vec<Rect> = session
        .document()
        .objects
        .iter()
        .filter(|object| object.id != moving)
        .filter_map(|object| object.shape().and_then(geometry::shape_bounds))
        .collect();
    let snap = snapping::snap_move(bounds, &others, snapping);
    (
        geometry::translated(&shape, snap.shift.0, snap.shift.1),
        snap.guides,
    )
}

fn handle_at(session: &EditSession, point: Point, reach: f32) -> Option<(u32, Handle)> {
    let id = session.selected()?;
    if session.refusal(id).is_some() {
        return None;
    }
    let shape = session.shape_of(id)?;
    geometry::handles(shape)
        .into_iter()
        .map(|(handle, at)| (handle, (at.x - point.x).hypot(at.y - point.y)))
        .filter(|(_, distance)| *distance <= reach)
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(handle, _)| (id, handle))
}
