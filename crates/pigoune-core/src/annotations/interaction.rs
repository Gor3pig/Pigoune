use super::geometry::{self, Handle, HitMode};
use super::session::EditSession;
use super::snapping::{self, Guide, Snapping};
use super::tools::{GeometryKind, ToolDefinition};
use super::{Options, Point, Rect, Shape};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    Select,
    Draw(&'static ToolDefinition),
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
    snapping: Option<Snapping>,
    guides: Vec<Guide>,
}

impl Default for Interaction {
    fn default() -> Self {
        Self {
            state: State::Idle,
            pending: None,
            constrained: false,
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
        if tool == Tool::Unsupported {
            return false;
        }
        if self.grab_selected(session, point, reach) {
            return true;
        }
        let mode = match tool {
            Tool::Select => HitMode::Body,
            _ => HitMode::Outline,
        };
        if let Some(id) = session.pick(point, reach, mode) {
            session.select(Some(id));
            self.begin_move(session, id, point);
            return true;
        }
        session.select(None);
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

    fn grab_selected(&mut self, session: &mut EditSession, point: Point, reach: f32) -> bool {
        if let Some((id, handle)) = handle_at(session, point, reach)
            && session.begin_change(id)
            && let Some(original) = session.shape_of(id).cloned()
        {
            self.state = State::Resizing { handle, original };
            return true;
        }
        let Some(id) = session.selected() else {
            return false;
        };
        if session.pick(point, reach, HitMode::Body) != Some(id) {
            return false;
        }
        self.begin_move(session, id, point);
        true
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

    pub fn release(&mut self, session: &mut EditSession, point: Point) {
        self.drag(session, point);
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
                    session.add(shape);
                }
            }
            State::Moving { .. } | State::Resizing { .. } => {
                session.end_change();
            }
        }
        self.pending = None;
        self.guides.clear();
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
    pub fn cursor(session: &EditSession, tool: Tool, point: Point, reach: f32) -> CursorKind {
        if tool == Tool::Unsupported {
            return CursorKind::Default;
        }
        if let Some((_, handle)) = handle_at(session, point, reach) {
            return CursorKind::Resize(handle);
        }
        let selected_body = session
            .selected()
            .is_some_and(|id| session.pick(point, reach, HitMode::Body) == Some(id));
        let mode = if tool == Tool::Select {
            HitMode::Body
        } else {
            HitMode::Outline
        };
        if selected_body || session.pick(point, reach, mode).is_some() {
            CursorKind::Move
        } else if tool == Tool::Select {
            CursorKind::Default
        } else {
            CursorKind::Crosshair
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
    let shape = session.shape_of(id)?;
    geometry::handles(shape)
        .into_iter()
        .find(|(_, handle_point)| {
            (handle_point.x - point.x).hypot(handle_point.y - point.y) <= reach
        })
        .map(|(handle, _)| (id, handle))
}
