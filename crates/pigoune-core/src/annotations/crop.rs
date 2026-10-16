use super::geometry::{Handle, handles};
use super::interaction::CursorKind;
use super::{Crop, Point, Rect};

pub const SMALLEST_SIDE: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropRatio {
    Free,
    Original,
    Square,
    FourThree,
    SixteenNine,
}

impl CropRatio {
    pub const ALL: [Self; 5] = [
        Self::Free,
        Self::Original,
        Self::Square,
        Self::FourThree,
        Self::SixteenNine,
    ];

    #[must_use]
    pub fn value(self, image: (f32, f32)) -> Option<f32> {
        match self {
            Self::Free => None,
            Self::Original => Some(image.0 / image.1),
            Self::Square => Some(1.0),
            Self::FourThree => Some(4.0 / 3.0),
            Self::SixteenNine => Some(16.0 / 9.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Idle,
    Moving { origin: Point, frame: Rect },
    Resizing { handle: Handle, frame: Rect },
}

#[derive(Debug, Clone)]
pub struct CropEditor {
    image: (f32, f32),
    committed: Option<Crop>,
    frame: Rect,
    ratio: CropRatio,
    state: State,
}

impl CropEditor {
    #[must_use]
    pub fn new(image_width: u32, image_height: u32, current: Option<Crop>) -> Self {
        let image = (to_f32(image_width), to_f32(image_height));
        let mut editor = Self {
            image,
            committed: current,
            frame: full_frame(image),
            ratio: CropRatio::Free,
            state: State::Idle,
        };
        editor.load(current);
        editor
    }

    pub fn load(&mut self, current: Option<Crop>) {
        self.committed = current;
        self.state = State::Idle;
        self.frame = current.map_or_else(
            || full_frame(self.image),
            |crop| {
                self.clamped(Rect {
                    x: to_f32(crop.x),
                    y: to_f32(crop.y),
                    width: to_f32(crop.width),
                    height: to_f32(crop.height),
                })
            },
        );
    }

    #[must_use]
    pub fn frame(&self) -> Rect {
        self.frame
    }

    #[must_use]
    pub fn ratio(&self) -> CropRatio {
        self.ratio
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        let crop = self.to_crop();
        (crop.width, crop.height)
    }

    #[must_use]
    pub fn crop(&self) -> Option<Crop> {
        let crop = self.to_crop();
        let full = full_frame(self.image);
        let covers_everything = crop.x == 0
            && crop.y == 0
            && to_f32(crop.width) >= full.width
            && to_f32(crop.height) >= full.height;
        (!covers_everything).then_some(crop)
    }

    #[must_use]
    pub fn is_changed(&self) -> bool {
        self.crop() != self.committed
    }

    fn to_crop(&self) -> Crop {
        let frame = self.clamped(self.frame);
        Crop {
            x: to_u32(frame.x.floor()),
            y: to_u32(frame.y.floor()),
            width: to_u32(frame.width.round()).max(1),
            height: to_u32(frame.height.round()).max(1),
        }
    }

    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.frame = full_frame(self.image);
    }

    pub fn discard(&mut self) {
        let committed = self.committed;
        self.load(committed);
    }

    pub fn set_ratio(&mut self, ratio: CropRatio) {
        self.ratio = ratio;
        let Some(wanted) = ratio.value(self.image) else {
            return;
        };
        let center = (
            self.frame.x + self.frame.width / 2.0,
            self.frame.y + self.frame.height / 2.0,
        );
        let (mut width, mut height) = (self.frame.width, self.frame.width / wanted);
        if height > self.frame.height {
            height = self.frame.height;
            width = height * wanted;
        }
        self.frame = self.fit_inside(center, width, height, wanted);
    }

    pub fn press(&mut self, point: Point, reach: f32) -> bool {
        if let Some(handle) = self.handle_at(point, reach) {
            self.state = State::Resizing {
                handle,
                frame: self.frame,
            };
            return true;
        }
        if inside(self.frame, point) {
            self.state = State::Moving {
                origin: point,
                frame: self.frame,
            };
            return true;
        }
        false
    }

    pub fn drag(&mut self, point: Point) {
        match self.state {
            State::Idle => {}
            State::Moving { origin, frame } => {
                self.frame = self.moved(frame, point.x - origin.x, point.y - origin.y);
            }
            State::Resizing { handle, frame } => {
                self.frame = self.resized(frame, handle, point);
            }
        }
    }

    pub fn release(&mut self, point: Point) {
        self.drag(point);
        self.state = State::Idle;
    }

    pub fn cancel_gesture(&mut self) {
        match self.state {
            State::Moving { frame, .. } | State::Resizing { frame, .. } => self.frame = frame,
            State::Idle => {}
        }
        self.state = State::Idle;
    }

    #[must_use]
    pub fn cursor(&self, point: Point, reach: f32) -> CursorKind {
        if let Some(handle) = self.handle_at(point, reach) {
            CursorKind::Resize(handle)
        } else if inside(self.frame, point) {
            CursorKind::Move
        } else {
            CursorKind::Default
        }
    }

    #[must_use]
    pub fn handle_points(&self) -> Vec<(Handle, Point)> {
        box_handles(self.frame)
    }

    fn handle_at(&self, point: Point, reach: f32) -> Option<Handle> {
        box_handles(self.frame)
            .into_iter()
            .find(|(_, at)| (at.x - point.x).hypot(at.y - point.y) <= reach)
            .map(|(handle, _)| handle)
    }

    fn clamped(&self, rect: Rect) -> Rect {
        let width = rect
            .width
            .clamp(SMALLEST_SIDE.min(self.image.0), self.image.0);
        let height = rect
            .height
            .clamp(SMALLEST_SIDE.min(self.image.1), self.image.1);
        Rect {
            x: rect.x.clamp(0.0, self.image.0 - width),
            y: rect.y.clamp(0.0, self.image.1 - height),
            width,
            height,
        }
    }

    fn moved(&self, frame: Rect, dx: f32, dy: f32) -> Rect {
        Rect {
            x: (frame.x + dx).clamp(0.0, self.image.0 - frame.width),
            y: (frame.y + dy).clamp(0.0, self.image.1 - frame.height),
            ..frame
        }
    }

    fn fit_inside(&self, center: (f32, f32), width: f32, height: f32, wanted: f32) -> Rect {
        let (mut width, mut height) = (width, height);
        if width > self.image.0 {
            width = self.image.0;
            height = width / wanted;
        }
        if height > self.image.1 {
            height = self.image.1;
            width = height * wanted;
        }
        Rect {
            x: (center.0 - width / 2.0).clamp(0.0, self.image.0 - width),
            y: (center.1 - height / 2.0).clamp(0.0, self.image.1 - height),
            width,
            height,
        }
    }

    fn resized(&self, frame: Rect, handle: Handle, to: Point) -> Rect {
        let to = Point {
            x: to.x.clamp(0.0, self.image.0),
            y: to.y.clamp(0.0, self.image.1),
        };
        let smallest_width = SMALLEST_SIDE.min(self.image.0);
        let smallest_height = SMALLEST_SIDE.min(self.image.1);
        let (mut left, mut top) = (frame.x, frame.y);
        let (mut right, mut bottom) = (frame.x + frame.width, frame.y + frame.height);
        match handle {
            Handle::TopLeft => (left, top) = (to.x, to.y),
            Handle::Top => top = to.y,
            Handle::TopRight => (right, top) = (to.x, to.y),
            Handle::Right => right = to.x,
            Handle::BottomRight => (right, bottom) = (to.x, to.y),
            Handle::Bottom => bottom = to.y,
            Handle::BottomLeft => (left, bottom) = (to.x, to.y),
            Handle::Left => left = to.x,
            Handle::Start | Handle::End | Handle::Curve => return frame,
        }
        if moves_left(handle) {
            left = left.min(right - smallest_width);
        } else if moves_right(handle) {
            right = right.max(left + smallest_width);
        }
        if moves_top(handle) {
            top = top.min(bottom - smallest_height);
        } else if moves_bottom(handle) {
            bottom = bottom.max(top + smallest_height);
        }
        let free = Rect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        };
        let Some(wanted) = self.ratio.value(self.image) else {
            return free;
        };
        self.with_ratio(free, handle, frame, wanted)
    }

    fn with_ratio(&self, free: Rect, handle: Handle, before: Rect, wanted: f32) -> Rect {
        let anchor_x = if moves_left(handle) {
            before.x + before.width
        } else {
            before.x
        };
        let anchor_y = if moves_top(handle) {
            before.y + before.height
        } else {
            before.y
        };
        let edge_only = matches!(
            handle,
            Handle::Top | Handle::Bottom | Handle::Left | Handle::Right
        );
        let mut width = if matches!(handle, Handle::Top | Handle::Bottom) {
            free.height * wanted
        } else if matches!(handle, Handle::Left | Handle::Right)
            || free.width / wanted >= free.height
        {
            free.width
        } else {
            free.height * wanted
        };
        width = width.max(SMALLEST_SIDE).max(SMALLEST_SIDE * wanted);
        let mut height = width / wanted;
        if edge_only {
            let center = (
                before.x + before.width / 2.0,
                before.y + before.height / 2.0,
            );
            return self.fit_edge(center, handle, before, width, height, wanted);
        }
        let toward_left = moves_left(handle);
        let toward_top = moves_top(handle);
        let room_x = if toward_left {
            anchor_x
        } else {
            self.image.0 - anchor_x
        };
        let room_y = if toward_top {
            anchor_y
        } else {
            self.image.1 - anchor_y
        };
        if width > room_x {
            width = room_x;
            height = width / wanted;
        }
        if height > room_y {
            height = room_y;
            width = height * wanted;
        }
        Rect {
            x: if toward_left {
                anchor_x - width
            } else {
                anchor_x
            },
            y: if toward_top {
                anchor_y - height
            } else {
                anchor_y
            },
            width,
            height,
        }
    }

    fn fit_edge(
        &self,
        center: (f32, f32),
        handle: Handle,
        before: Rect,
        width: f32,
        height: f32,
        wanted: f32,
    ) -> Rect {
        let (mut width, mut height) = (width, height);
        if width > self.image.0 {
            width = self.image.0;
            height = width / wanted;
        }
        if height > self.image.1 {
            height = self.image.1;
            width = height * wanted;
        }
        let mut rect = Rect {
            x: center.0 - width / 2.0,
            y: center.1 - height / 2.0,
            width,
            height,
        };
        match handle {
            Handle::Left => rect.x = before.x + before.width - width,
            Handle::Right => rect.x = before.x,
            Handle::Top => rect.y = before.y + before.height - height,
            Handle::Bottom => rect.y = before.y,
            _ => {}
        }
        rect.x = rect.x.clamp(0.0, self.image.0 - rect.width);
        rect.y = rect.y.clamp(0.0, self.image.1 - rect.height);
        rect
    }
}

fn moves_left(handle: Handle) -> bool {
    matches!(handle, Handle::TopLeft | Handle::Left | Handle::BottomLeft)
}

fn moves_right(handle: Handle) -> bool {
    matches!(
        handle,
        Handle::TopRight | Handle::Right | Handle::BottomRight
    )
}

fn moves_bottom(handle: Handle) -> bool {
    matches!(
        handle,
        Handle::BottomLeft | Handle::Bottom | Handle::BottomRight
    )
}

fn moves_top(handle: Handle) -> bool {
    matches!(handle, Handle::TopLeft | Handle::Top | Handle::TopRight)
}

fn full_frame(image: (f32, f32)) -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: image.0,
        height: image.1,
    }
}

fn inside(rect: Rect, point: Point) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}

fn box_handles(frame: Rect) -> Vec<(Handle, Point)> {
    let shape = super::Shape::new(
        "rectangle",
        super::Geometry::Box(frame),
        super::Options::new(),
    );
    handles(&shape)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "image sides are far below the 24 bits of an f32 mantissa"
)]
fn to_f32(value: u32) -> f32 {
    value as f32
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is a non-negative rounded pixel count inside the image"
)]
fn to_u32(value: f32) -> u32 {
    value.max(0.0) as u32
}
