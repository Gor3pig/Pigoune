use super::{Color, Point, Rect, Shape, Stroke};

const SMALLEST_DRAG: f32 = 3.0;
const SMALLEST_ARROW_HEAD: f32 = 12.0;
const ARROW_HEAD_PER_WIDTH: f32 = 3.5;
const ARROW_HEAD_SPREAD: f32 = 0.53;
const LONGEST_HEAD_SHARE: f32 = 0.9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingTool {
    Rectangle,
    Ellipse,
    Line,
    Arrow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrowHead {
    pub tip: Point,
    pub left: Point,
    pub right: Point,
    pub base: Point,
}

#[must_use]
pub fn arrow_head(from: Point, to: Point, width: f32) -> ArrowHead {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx.hypot(dy);
    let (direction_x, direction_y) = if length > 0.0 {
        (dx / length, dy / length)
    } else {
        (1.0, 0.0)
    };
    let wanted = (width * ARROW_HEAD_PER_WIDTH).max(SMALLEST_ARROW_HEAD);
    let head = if length > 0.0 {
        wanted.min(length * LONGEST_HEAD_SHARE)
    } else {
        wanted
    };
    let base = Point {
        x: to.x - direction_x * head,
        y: to.y - direction_y * head,
    };
    let spread = head * ARROW_HEAD_SPREAD;
    ArrowHead {
        tip: to,
        left: Point {
            x: base.x - direction_y * spread,
            y: base.y + direction_x * spread,
        },
        right: Point {
            x: base.x + direction_y * spread,
            y: base.y - direction_x * spread,
        },
        base,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub color: Color,
    pub width: f32,
    pub opacity: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            color: Color {
                r: 224,
                g: 27,
                b: 36,
                a: 255,
            },
            width: 6.0,
            opacity: 1.0,
        }
    }
}

impl Style {
    #[must_use]
    pub fn stroke(&self) -> Stroke {
        Stroke {
            color: self.color_with_opacity(),
            width: self.width,
        }
    }

    #[must_use]
    pub fn color_with_opacity(&self) -> Color {
        Color {
            a: opacity_to_alpha(self.opacity),
            ..self.color
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the opacity is clamped to 0..=1 before the conversion"
)]
fn opacity_to_alpha(opacity: f32) -> u8 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[must_use]
pub fn style_of(shape: &Shape) -> Option<Style> {
    let (color, width) = match shape {
        Shape::Rectangle { stroke, .. }
        | Shape::Ellipse { stroke, .. }
        | Shape::Line { stroke, .. }
        | Shape::Arrow { stroke, .. }
        | Shape::Pen { stroke, .. }
        | Shape::Highlighter { stroke, .. } => (stroke.color, stroke.width),
        Shape::Text { color, font, .. } => (*color, font.size),
        Shape::Step { color, radius, .. } => (*color, *radius),
        Shape::Blur { .. } | Shape::Pixelate { .. } => return None,
    };
    Some(Style {
        color: Color { a: 255, ..color },
        width,
        opacity: f32::from(color.a) / 255.0,
    })
}

#[must_use]
pub fn restyled(shape: &Shape, style: &Style) -> Shape {
    let mut shape = shape.clone();
    match &mut shape {
        Shape::Rectangle { stroke, .. }
        | Shape::Ellipse { stroke, .. }
        | Shape::Line { stroke, .. }
        | Shape::Arrow { stroke, .. }
        | Shape::Pen { stroke, .. }
        | Shape::Highlighter { stroke, .. } => *stroke = style.stroke(),
        Shape::Text { color, font, .. } => {
            *color = style.color_with_opacity();
            font.size = style.width;
        }
        Shape::Step { color, radius, .. } => {
            *color = style.color_with_opacity();
            *radius = style.width;
        }
        Shape::Blur { .. } | Shape::Pixelate { .. } => {}
    }
    shape
}

#[must_use]
pub fn shape_from_drag(tool: DrawingTool, from: Point, to: Point, style: &Style) -> Option<Shape> {
    if (to.x - from.x).abs().max((to.y - from.y).abs()) < SMALLEST_DRAG {
        return None;
    }
    let stroke = style.stroke();
    Some(match tool {
        DrawingTool::Rectangle => Shape::Rectangle {
            bounds: rect_between(from, to),
            stroke,
            fill: None,
        },
        DrawingTool::Ellipse => Shape::Ellipse {
            bounds: rect_between(from, to),
            stroke,
            fill: None,
        },
        DrawingTool::Line => Shape::Line { from, to, stroke },
        DrawingTool::Arrow => Shape::Arrow { from, to, stroke },
    })
}

#[must_use]
pub fn rect_between(a: Point, b: Point) -> Rect {
    Rect {
        x: a.x.min(b.x),
        y: a.y.min(b.y),
        width: (a.x - b.x).abs(),
        height: (a.y - b.y).abs(),
    }
}

fn normalized(rect: Rect) -> Rect {
    rect_between(
        Point {
            x: rect.x,
            y: rect.y,
        },
        Point {
            x: rect.x + rect.width,
            y: rect.y + rect.height,
        },
    )
}

fn box_of(shape: &Shape) -> Option<Rect> {
    match shape {
        Shape::Rectangle { bounds, .. }
        | Shape::Ellipse { bounds, .. }
        | Shape::Blur { bounds, .. }
        | Shape::Pixelate { bounds, .. } => Some(normalized(*bounds)),
        _ => None,
    }
}

fn ends_of(shape: &Shape) -> Option<(Point, Point)> {
    match shape {
        Shape::Line { from, to, .. } | Shape::Arrow { from, to, .. } => Some((*from, *to)),
        _ => None,
    }
}

#[must_use]
pub fn shape_bounds(shape: &Shape) -> Option<Rect> {
    if let Some(rect) = box_of(shape) {
        return Some(rect);
    }
    if let Some((from, to)) = ends_of(shape) {
        return Some(rect_between(from, to));
    }
    match shape {
        Shape::Pen { points, .. } | Shape::Highlighter { points, .. } => {
            let first = *points.first()?;
            let (mut low, mut high) = (first, first);
            for point in points {
                low.x = low.x.min(point.x);
                low.y = low.y.min(point.y);
                high.x = high.x.max(point.x);
                high.y = high.y.max(point.y);
            }
            Some(rect_between(low, high))
        }
        _ => None,
    }
}

#[must_use]
pub fn translated(shape: &Shape, dx: f32, dy: f32) -> Shape {
    let shift = |point: &mut Point| {
        point.x += dx;
        point.y += dy;
    };
    let mut shape = shape.clone();
    match &mut shape {
        Shape::Rectangle { bounds, .. }
        | Shape::Ellipse { bounds, .. }
        | Shape::Blur { bounds, .. }
        | Shape::Pixelate { bounds, .. } => {
            bounds.x += dx;
            bounds.y += dy;
        }
        Shape::Line { from, to, .. } | Shape::Arrow { from, to, .. } => {
            shift(from);
            shift(to);
        }
        Shape::Text { origin, .. } => shift(origin),
        Shape::Step { center, .. } => shift(center),
        Shape::Pen { points, .. } | Shape::Highlighter { points, .. } => {
            points.iter_mut().for_each(shift);
        }
    }
    shape
}

#[must_use]
pub fn handles(shape: &Shape) -> Vec<(Handle, Point)> {
    if let Some(rect) = box_of(shape) {
        let (left, top) = (rect.x, rect.y);
        let (right, bottom) = (rect.x + rect.width, rect.y + rect.height);
        let (middle_x, middle_y) = (left + rect.width / 2.0, top + rect.height / 2.0);
        let at = |handle, x, y| (handle, Point { x, y });
        return vec![
            at(Handle::TopLeft, left, top),
            at(Handle::Top, middle_x, top),
            at(Handle::TopRight, right, top),
            at(Handle::Right, right, middle_y),
            at(Handle::BottomRight, right, bottom),
            at(Handle::Bottom, middle_x, bottom),
            at(Handle::BottomLeft, left, bottom),
            at(Handle::Left, left, middle_y),
        ];
    }
    ends_of(shape).map_or_else(Vec::new, |(from, to)| {
        vec![(Handle::Start, from), (Handle::End, to)]
    })
}

#[must_use]
pub fn resized(shape: &Shape, handle: Handle, to: Point) -> Shape {
    let mut shape = shape.clone();
    if let Some(rect) = box_of(&shape) {
        let (mut left, mut top) = (rect.x, rect.y);
        let (mut right, mut bottom) = (rect.x + rect.width, rect.y + rect.height);
        match handle {
            Handle::TopLeft => (left, top) = (to.x, to.y),
            Handle::Top => top = to.y,
            Handle::TopRight => (right, top) = (to.x, to.y),
            Handle::Right => right = to.x,
            Handle::BottomRight => (right, bottom) = (to.x, to.y),
            Handle::Bottom => bottom = to.y,
            Handle::BottomLeft => (left, bottom) = (to.x, to.y),
            Handle::Left => left = to.x,
            Handle::Start | Handle::End => return shape,
        }
        let new_bounds = rect_between(
            Point { x: left, y: top },
            Point {
                x: right,
                y: bottom,
            },
        );
        match &mut shape {
            Shape::Rectangle { bounds, .. }
            | Shape::Ellipse { bounds, .. }
            | Shape::Blur { bounds, .. }
            | Shape::Pixelate { bounds, .. } => *bounds = new_bounds,
            _ => {}
        }
        return shape;
    }
    match (&mut shape, handle) {
        (Shape::Line { from, .. } | Shape::Arrow { from, .. }, Handle::Start) => *from = to,
        (Shape::Line { to: end, .. } | Shape::Arrow { to: end, .. }, Handle::End) => *end = to,
        _ => {}
    }
    shape
}

fn distance_to_segment(point: Point, from: Point, to: Point) -> f32 {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length_squared = dx * dx + dy * dy;
    let along = if length_squared == 0.0 {
        0.0
    } else {
        (((point.x - from.x) * dx + (point.y - from.y) * dy) / length_squared).clamp(0.0, 1.0)
    };
    let (nearest_x, nearest_y) = (from.x + along * dx, from.y + along * dy);
    (point.x - nearest_x).hypot(point.y - nearest_y)
}

#[must_use]
pub fn hit(shape: &Shape, point: Point, tolerance: f32) -> bool {
    match shape {
        Shape::Rectangle {
            bounds,
            stroke,
            fill,
        } => {
            let rect = normalized(*bounds);
            let reach = stroke.width / 2.0 + tolerance;
            let outer = expanded(rect, reach);
            if !contains(outer, point) {
                return false;
            }
            fill.is_some() || !contains(expanded(rect, -reach), point)
        }
        Shape::Ellipse {
            bounds,
            stroke,
            fill,
        } => {
            let rect = normalized(*bounds);
            let (radius_x, radius_y) = (rect.width / 2.0, rect.height / 2.0);
            if radius_x <= 0.0 || radius_y <= 0.0 {
                return false;
            }
            let (offset_x, offset_y) =
                (point.x - (rect.x + radius_x), point.y - (rect.y + radius_y));
            let scaled = (offset_x / radius_x).hypot(offset_y / radius_y);
            let reach = stroke.width / 2.0 + tolerance;
            let distance = (scaled - 1.0).abs() * radius_x.min(radius_y);
            distance <= reach || (fill.is_some() && scaled <= 1.0)
        }
        Shape::Line { from, to, stroke } | Shape::Arrow { from, to, stroke } => {
            distance_to_segment(point, *from, *to) <= stroke.width / 2.0 + tolerance
        }
        Shape::Blur { bounds, .. } | Shape::Pixelate { bounds, .. } => {
            contains(expanded(normalized(*bounds), tolerance), point)
        }
        Shape::Pen { points, stroke } | Shape::Highlighter { points, stroke } => {
            let reach = stroke.width / 2.0 + tolerance;
            points
                .windows(2)
                .any(|pair| distance_to_segment(point, pair[0], pair[1]) <= reach)
        }
        Shape::Text { .. } | Shape::Step { .. } => false,
    }
}

fn expanded(rect: Rect, amount: f32) -> Rect {
    Rect {
        x: rect.x - amount,
        y: rect.y - amount,
        width: rect.width + 2.0 * amount,
        height: rect.height + 2.0 * amount,
    }
}

fn contains(rect: Rect, point: Point) -> bool {
    rect.width >= 0.0
        && rect.height >= 0.0
        && point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}
