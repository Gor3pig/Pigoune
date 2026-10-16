use super::shape::{Geometry, Shape};
use super::tools::{GeometryKind, Outline, ToolDefinition};
use super::{Options, Point, Rect};

const SMALLEST_DRAG: f32 = 3.0;
const SMALLEST_POINT_STEP: f32 = 1.0;
const SIMPLIFY_TOLERANCE: f32 = 0.8;
const SMALLEST_ARROW_HEAD: f32 = 12.0;
const ARROW_HEAD_SPREAD: f32 = 0.53;
const LONGEST_HEAD_SHARE: f32 = 0.9;
const DASH_LENGTH_PER_WIDTH: f32 = 3.0;
const DASH_GAP_PER_WIDTH: f32 = 2.0;
const DOT_LENGTH: f32 = 0.01;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrowHead {
    pub tip: Point,
    pub left: Point,
    pub right: Point,
    pub base: Point,
}

#[must_use]
pub fn head_size_factor(choice: &str) -> f32 {
    match choice {
        "small" => 2.5,
        "large" => 5.5,
        _ => 3.5,
    }
}

#[must_use]
pub fn dash_pattern(choice: &str, width: f32) -> Option<[f32; 2]> {
    match choice {
        "dashed" => Some([width * DASH_LENGTH_PER_WIDTH, width * DASH_GAP_PER_WIDTH]),
        "dotted" => Some([DOT_LENGTH, width * DASH_GAP_PER_WIDTH]),
        _ => None,
    }
}

#[must_use]
pub fn arrow_head(from: Point, to: Point, width: f32, size_factor: f32) -> ArrowHead {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx.hypot(dy);
    let (direction_x, direction_y) = if length > 0.0 {
        (dx / length, dy / length)
    } else {
        (1.0, 0.0)
    };
    let wanted = (width * size_factor).max(SMALLEST_ARROW_HEAD);
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

#[must_use]
pub fn shape_from_drag(
    definition: &ToolDefinition,
    from: Point,
    to: Point,
    options: &Options,
) -> Option<Shape> {
    if (to.x - from.x).abs().max((to.y - from.y).abs()) < SMALLEST_DRAG {
        return None;
    }
    let geometry = match definition.geometry {
        GeometryKind::TwoPoints => Geometry::TwoPoints { from, to },
        GeometryKind::Box => Geometry::Box(rect_between(from, to)),
        GeometryKind::Polyline | GeometryKind::Anchor => return None,
    };
    Some(Shape::new(definition.kind, geometry, options.clone()))
}

#[must_use]
pub fn constrained_end(from: Point, to: Point, kind: GeometryKind) -> Point {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    match kind {
        GeometryKind::Box => {
            let side = dx.abs().max(dy.abs());
            Point {
                x: from.x + side * dx.signum(),
                y: from.y + side * dy.signum(),
            }
        }
        GeometryKind::TwoPoints => {
            let step = std::f32::consts::FRAC_PI_4;
            let angle = (dy.atan2(dx) / step).round() * step;
            let length = dx.hypot(dy);
            Point {
                x: from.x + length * angle.cos(),
                y: from.y + length * angle.sin(),
            }
        }
        GeometryKind::Polyline | GeometryKind::Anchor => to,
    }
}

#[must_use]
pub fn shape_from_anchor(
    definition: &ToolDefinition,
    at: Point,
    options: &Options,
) -> Option<Shape> {
    (definition.geometry == GeometryKind::Anchor)
        .then(|| Shape::new(definition.kind, Geometry::Anchor(at), options.clone()))
}

#[must_use]
pub fn shape_from_points(
    definition: &ToolDefinition,
    points: &[Point],
    options: &Options,
    finished: bool,
) -> Option<Shape> {
    if definition.geometry != GeometryKind::Polyline {
        return None;
    }
    let first = *points.first()?;
    let reach = points
        .iter()
        .map(|point| (point.x - first.x).hypot(point.y - first.y))
        .fold(0.0_f32, f32::max);
    if reach < SMALLEST_DRAG {
        return None;
    }
    let kept = if finished {
        simplified(points, SIMPLIFY_TOLERANCE)
    } else {
        points.to_vec()
    };
    Some(Shape::new(
        definition.kind,
        Geometry::Polyline(kept),
        options.clone(),
    ))
}

#[must_use]
pub fn next_stroke_point(points: &[Point], candidate: Point) -> bool {
    points.last().is_none_or(|last| {
        (candidate.x - last.x).hypot(candidate.y - last.y) >= SMALLEST_POINT_STEP
    })
}

#[must_use]
pub fn simplified(points: &[Point], tolerance: f32) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut pending = vec![(0, points.len() - 1)];
    while let Some((start, end)) = pending.pop() {
        let mut farthest = (0.0_f32, start);
        for index in start + 1..end {
            let distance = distance_to_segment(points[index], points[start], points[end]);
            if distance > farthest.0 {
                farthest = (distance, index);
            }
        }
        if farthest.0 > tolerance {
            keep[farthest.1] = true;
            pending.push((start, farthest.1));
            pending.push((farthest.1, end));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(point, kept)| kept.then_some(*point))
        .collect()
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

#[must_use]
pub fn shape_bounds(shape: &Shape) -> Option<Rect> {
    match &shape.geometry {
        Geometry::Box(rect) => Some(normalized(*rect)),
        Geometry::TwoPoints { from, to } => Some(rect_between(*from, *to)),
        Geometry::Polyline(points) => {
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
        Geometry::Anchor(at) => {
            let definition = shape.definition()?;
            (definition.outline == Outline::Disc).then(|| {
                let radius = shape.number("size");
                Rect {
                    x: at.x - radius,
                    y: at.y - radius,
                    width: radius * 2.0,
                    height: radius * 2.0,
                }
            })
        }
    }
}

#[must_use]
pub fn translated(shape: &Shape, dx: f32, dy: f32) -> Shape {
    let shift = |point: &Point| Point {
        x: point.x + dx,
        y: point.y + dy,
    };
    let geometry = match &shape.geometry {
        Geometry::Box(rect) => Geometry::Box(Rect {
            x: rect.x + dx,
            y: rect.y + dy,
            ..*rect
        }),
        Geometry::TwoPoints { from, to } => Geometry::TwoPoints {
            from: shift(from),
            to: shift(to),
        },
        Geometry::Polyline(points) => Geometry::Polyline(points.iter().map(shift).collect()),
        Geometry::Anchor(at) => Geometry::Anchor(shift(at)),
    };
    shape.with_geometry(geometry)
}

#[must_use]
pub fn handles(shape: &Shape) -> Vec<(Handle, Point)> {
    match &shape.geometry {
        Geometry::Box(rect) => {
            let rect = normalized(*rect);
            let (left, top) = (rect.x, rect.y);
            let (right, bottom) = (rect.x + rect.width, rect.y + rect.height);
            let (middle_x, middle_y) = (left + rect.width / 2.0, top + rect.height / 2.0);
            let at = |handle, x, y| (handle, Point { x, y });
            vec![
                at(Handle::TopLeft, left, top),
                at(Handle::Top, middle_x, top),
                at(Handle::TopRight, right, top),
                at(Handle::Right, right, middle_y),
                at(Handle::BottomRight, right, bottom),
                at(Handle::Bottom, middle_x, bottom),
                at(Handle::BottomLeft, left, bottom),
                at(Handle::Left, left, middle_y),
            ]
        }
        Geometry::TwoPoints { from, to } => {
            vec![(Handle::Start, *from), (Handle::End, *to)]
        }
        Geometry::Polyline(_) | Geometry::Anchor(_) => Vec::new(),
    }
}

#[must_use]
pub fn resized(shape: &Shape, handle: Handle, to: Point) -> Shape {
    match &shape.geometry {
        Geometry::Box(rect) => {
            let rect = normalized(*rect);
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
                Handle::Start | Handle::End => return shape.clone(),
            }
            let bounds = rect_between(
                Point { x: left, y: top },
                Point {
                    x: right,
                    y: bottom,
                },
            );
            shape.with_geometry(Geometry::Box(bounds))
        }
        Geometry::TwoPoints { from, to: end } => match handle {
            Handle::Start => shape.with_geometry(Geometry::TwoPoints { from: to, to: *end }),
            Handle::End => shape.with_geometry(Geometry::TwoPoints { from: *from, to }),
            _ => shape.clone(),
        },
        Geometry::Polyline(_) | Geometry::Anchor(_) => shape.clone(),
    }
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
pub fn estimated_text_extent(shape: &Shape) -> (f32, f32) {
    let size = shape.number("font_size");
    let lines: Vec<&str> = shape.text("content").split('\n').collect();
    let longest = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let width = f32::from(u16::try_from(longest).unwrap_or(u16::MAX)) * size * 0.55;
    let height = f32::from(u16::try_from(lines.len()).unwrap_or(u16::MAX)) * size * 1.25;
    (width.max(size * 0.5), height.max(size * 1.25))
}

#[must_use]
pub fn hit(shape: &Shape, point: Point, tolerance: f32) -> bool {
    hit_with_extent(shape, point, tolerance, None)
}

#[must_use]
pub fn hit_with_extent(
    shape: &Shape,
    point: Point,
    tolerance: f32,
    extent: Option<(f32, f32)>,
) -> bool {
    let Some(definition) = shape.definition() else {
        return false;
    };
    let reach = shape.number("width") / 2.0 + tolerance;
    match (&shape.geometry, definition.outline) {
        (Geometry::Box(bounds), Outline::Rectangle) => {
            let rect = normalized(*bounds);
            if !contains(expanded(rect, reach), point) {
                return false;
            }
            shape.toggle("fill") || !contains(expanded(rect, -reach), point)
        }
        (Geometry::Box(bounds), Outline::Ellipse) => {
            let rect = normalized(*bounds);
            let (radius_x, radius_y) = (rect.width / 2.0, rect.height / 2.0);
            if radius_x <= 0.0 || radius_y <= 0.0 {
                return false;
            }
            let (offset_x, offset_y) =
                (point.x - (rect.x + radius_x), point.y - (rect.y + radius_y));
            let scaled = (offset_x / radius_x).hypot(offset_y / radius_y);
            let distance = (scaled - 1.0).abs() * radius_x.min(radius_y);
            distance <= reach || (shape.toggle("fill") && scaled <= 1.0)
        }
        (Geometry::TwoPoints { from, to }, Outline::Segment) => {
            distance_to_segment(point, *from, *to) <= reach
        }
        (Geometry::Box(bounds), Outline::Region) => {
            contains(expanded(normalized(*bounds), tolerance), point)
        }
        (Geometry::Anchor(at), Outline::TextBox) => {
            let (width, height) = extent.unwrap_or_else(|| estimated_text_extent(shape));
            contains(
                expanded(
                    Rect {
                        x: at.x,
                        y: at.y,
                        width,
                        height,
                    },
                    tolerance,
                ),
                point,
            )
        }
        (Geometry::Anchor(at), Outline::Disc) => {
            (at.x - point.x).hypot(at.y - point.y) <= shape.number("size") + tolerance
        }
        (Geometry::Polyline(points), Outline::Stroke) => match points.as_slice() {
            [] => false,
            [only] => (only.x - point.x).hypot(only.y - point.y) <= reach,
            _ => points
                .windows(2)
                .any(|pair| distance_to_segment(point, pair[0], pair[1]) <= reach),
        },
        _ => false,
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
