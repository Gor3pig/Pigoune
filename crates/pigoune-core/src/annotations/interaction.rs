use super::geometry::{self, DrawingTool, Handle, Style};
use super::session::EditSession;
use super::{Point, Shape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Draw(DrawingTool),
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
        tool: DrawingTool,
        from: Point,
        style: Style,
    },
    Moving {
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
}

impl Default for Interaction {
    fn default() -> Self {
        Self {
            state: State::Idle,
            pending: None,
        }
    }
}

impl Interaction {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
        style: &Style,
        point: Point,
        reach: f32,
    ) -> bool {
        self.cancel(session);
        match tool {
            Tool::Unsupported => false,
            Tool::Draw(drawing) => {
                session.select(None);
                self.state = State::Drawing {
                    tool: drawing,
                    from: point,
                    style: *style,
                };
                true
            }
            Tool::Select => self.press_select(session, point, reach),
        }
    }

    fn press_select(&mut self, session: &mut EditSession, point: Point, reach: f32) -> bool {
        if let Some((id, handle)) = handle_at(session, point, reach)
            && session.begin_change(id)
            && let Some(original) = session.shape_of(id).cloned()
        {
            self.state = State::Resizing { handle, original };
            return true;
        }
        let Some(id) = session.hit_test(point, reach) else {
            session.select(None);
            return false;
        };
        session.select(Some(id));
        if session.begin_change(id)
            && let Some(original) = session.shape_of(id).cloned()
        {
            self.state = State::Moving {
                origin: point,
                original,
            };
        }
        true
    }

    pub fn drag(&mut self, session: &mut EditSession, point: Point) {
        match &self.state {
            State::Idle => {}
            State::Drawing { tool, from, style } => {
                self.pending = geometry::shape_from_drag(*tool, *from, point, style);
            }
            State::Moving { origin, original } => {
                session.preview(geometry::translated(
                    original,
                    point.x - origin.x,
                    point.y - origin.y,
                ));
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
            State::Drawing { .. } => {
                if let Some(shape) = self.pending.take() {
                    session.add(shape);
                }
            }
            State::Moving { .. } | State::Resizing { .. } => {
                session.end_change();
            }
        }
        self.pending = None;
    }

    pub fn cancel(&mut self, session: &mut EditSession) {
        if matches!(self.state, State::Moving { .. } | State::Resizing { .. }) {
            session.cancel_change();
        }
        self.state = State::Idle;
        self.pending = None;
    }

    #[must_use]
    pub fn cursor(session: &EditSession, tool: Tool, point: Point, reach: f32) -> CursorKind {
        match tool {
            Tool::Unsupported => CursorKind::Default,
            Tool::Draw(_) => CursorKind::Crosshair,
            Tool::Select => {
                if let Some((_, handle)) = handle_at(session, point, reach) {
                    CursorKind::Resize(handle)
                } else if session.hit_test(point, reach).is_some() {
                    CursorKind::Move
                } else {
                    CursorKind::Default
                }
            }
        }
    }
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
