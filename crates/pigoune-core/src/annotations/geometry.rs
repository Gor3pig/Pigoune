use super::options::number_to_value;
use super::shape::{Geometry, Shape};
use super::tools::{
    GeometryKind, LARGEST_BADGE, LARGEST_FONT, Outline, SMALLEST_BADGE, SMALLEST_FONT,
    ToolDefinition,
};
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
const ROUNDED_SPOT_SHARE: f32 = 0.25;
const ROTATE_HANDLE_REACHES: f32 = 3.0;
pub const TEXT_BACKDROP_SHARE: f32 = 0.25;
pub const TEXT_OUTLINE_SHARE: f32 = 0.06;

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
    Curve,
    Rotate,
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

const CURVE_STEPS: u8 = 24;
const FLAT_CURVE: f32 = 0.5;

#[must_use]
pub fn curve_control(from: Point, to: Point, bend: f32) -> Option<Point> {
    if bend.abs() < FLAT_CURVE {
        return None;
    }
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx.hypot(dy);
    if length <= 0.0 {
        return None;
    }
    let offset = bend / 100.0 * length / 2.0;
    Some(Point {
        x: f32::midpoint(from.x, to.x) - dy / length * offset,
        y: f32::midpoint(from.y, to.y) + dx / length * offset,
    })
}

#[must_use]
pub fn arrow_curve_control(shape: &Shape) -> Option<Point> {
    match shape.geometry {
        Geometry::TwoPoints { from, to } if shape.kind == "arrow" => {
            curve_control(from, to, shape.number("curve"))
        }
        _ => None,
    }
}

#[must_use]
pub fn curve_point(from: Point, control: Point, to: Point, t: f32) -> Point {
    let (a, b, c) = ((1.0 - t) * (1.0 - t), 2.0 * (1.0 - t) * t, t * t);
    Point {
        x: a * from.x + b * control.x + c * to.x,
        y: a * from.y + b * control.y + c * to.y,
    }
}

#[must_use]
pub fn curve_polyline(from: Point, control: Point, to: Point) -> Vec<Point> {
    (0..=CURVE_STEPS)
        .map(|step| curve_point(from, control, to, f32::from(step) / f32::from(CURVE_STEPS)))
        .collect()
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

fn points_bounds(points: &[Point]) -> Option<Rect> {
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
        Geometry::TwoPoints { from, to } => match arrow_curve_control(shape) {
            Some(control) => points_bounds(&curve_polyline(*from, control, *to)),
            None => Some(rect_between(*from, *to)),
        },
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
    handles_with_extent(shape, None)
}

#[must_use]
pub fn rotation_of(shape: &Shape) -> f32 {
    shape.number("rotation")
}

#[must_use]
pub fn rotate_point(point: Point, pivot: Point, degrees: f32) -> Point {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (dx, dy) = (point.x - pivot.x, point.y - pivot.y);
    Point {
        x: pivot.x + dx * cos - dy * sin,
        y: pivot.y + dx * sin + dy * cos,
    }
}

fn unturned_box(shape: &Shape, extent: Option<(f32, f32)>) -> Option<Rect> {
    match &shape.geometry {
        Geometry::Box(rect) => Some(normalized(*rect)),
        Geometry::TwoPoints { .. } | Geometry::Polyline(_) => shape_bounds(shape),
        Geometry::Anchor(_) if shape.kind == "text" => text_visual_bounds(
            shape,
            extent.unwrap_or_else(|| estimated_text_extent(shape)),
        ),
        Geometry::Anchor(_) => None,
    }
}

#[must_use]
pub fn shape_pivot(shape: &Shape, extent: Option<(f32, f32)>) -> Option<Point> {
    unturned_box(shape, extent).map(|rect| Point {
        x: rect.x + rect.width / 2.0,
        y: rect.y + rect.height / 2.0,
    })
}

fn is_turned(shape: &Shape) -> bool {
    rotation_of(shape).abs() > f32::EPSILON
}

fn turned_back(shape: &Shape, point: Point, extent: Option<(f32, f32)>) -> Point {
    if !is_turned(shape) {
        return point;
    }
    shape_pivot(shape, extent).map_or(point, |pivot| {
        rotate_point(point, pivot, -rotation_of(shape))
    })
}

#[must_use]
pub fn rotated_bounds(shape: &Shape, extent: Option<(f32, f32)>) -> Option<Rect> {
    let rect = unturned_box(shape, extent)?;
    let pivot = shape_pivot(shape, extent)?;
    let angle = rotation_of(shape);
    let corners = [
        Point {
            x: rect.x,
            y: rect.y,
        },
        Point {
            x: rect.x + rect.width,
            y: rect.y,
        },
        Point {
            x: rect.x + rect.width,
            y: rect.y + rect.height,
        },
        Point {
            x: rect.x,
            y: rect.y + rect.height,
        },
    ]
    .map(|corner| rotate_point(corner, pivot, angle));
    let left = corners.iter().map(|c| c.x).fold(f32::INFINITY, f32::min);
    let right = corners
        .iter()
        .map(|c| c.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let top = corners.iter().map(|c| c.y).fold(f32::INFINITY, f32::min);
    let bottom = corners
        .iter()
        .map(|c| c.y)
        .fold(f32::NEG_INFINITY, f32::max);
    Some(Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

#[must_use]
pub fn turns_by_option(shape: &Shape) -> bool {
    shape
        .definition()
        .is_some_and(|definition| definition.spec("rotation").is_some())
}

#[must_use]
pub fn turned_points(shape: &Shape, pivot: Point, degrees: f32) -> Shape {
    match &shape.geometry {
        Geometry::TwoPoints { from, to } => shape.with_geometry(Geometry::TwoPoints {
            from: rotate_point(*from, pivot, degrees),
            to: rotate_point(*to, pivot, degrees),
        }),
        Geometry::Polyline(points) => shape.with_geometry(Geometry::Polyline(
            points
                .iter()
                .map(|point| rotate_point(*point, pivot, degrees))
                .collect(),
        )),
        _ => shape.clone(),
    }
}

#[must_use]
pub fn rotation_handle(shape: &Shape, extent: Option<(f32, f32)>, reach: f32) -> Option<Point> {
    if !turns_by_option(shape)
        && !matches!(
            shape.geometry,
            Geometry::TwoPoints { .. } | Geometry::Polyline(_)
        )
    {
        return None;
    }
    let rect = unturned_box(shape, extent)?;
    let pivot = shape_pivot(shape, extent)?;
    let above = Point {
        x: pivot.x,
        y: rect.y - reach * ROTATE_HANDLE_REACHES,
    };
    Some(rotate_point(above, pivot, rotation_of(shape)))
}

#[must_use]
pub fn handles_with_extent(shape: &Shape, extent: Option<(f32, f32)>) -> Vec<(Handle, Point)> {
    let found = unturned_handles(shape, extent);
    if !is_turned(shape) {
        return found;
    }
    let Some(pivot) = shape_pivot(shape, extent) else {
        return found;
    };
    let angle = rotation_of(shape);
    found
        .into_iter()
        .map(|(handle, at)| (handle, rotate_point(at, pivot, angle)))
        .collect()
}

fn unturned_handles(shape: &Shape, extent: Option<(f32, f32)>) -> Vec<(Handle, Point)> {
    if shape.kind == "text" {
        return extent
            .and_then(|extent| text_visual_bounds(shape, extent))
            .map_or_else(Vec::new, box_handles);
    }
    match &shape.geometry {
        Geometry::Box(rect) => {
            let rect = normalized(*rect);
            box_handles(rect)
        }
        Geometry::TwoPoints { from, to } => {
            let mut found = vec![(Handle::Start, *from), (Handle::End, *to)];
            if shape.kind == "arrow" {
                found.push((Handle::Curve, arrow_middle(shape, *from, *to)));
            }
            found
        }
        Geometry::Polyline(_) => scalable_bounds(shape).map_or_else(Vec::new, box_handles),
        Geometry::Anchor(_) => scalable_bounds(shape)
            .map(|bounds| {
                (
                    Handle::BottomRight,
                    Point {
                        x: bounds.x + bounds.width,
                        y: bounds.y + bounds.height,
                    },
                )
            })
            .into_iter()
            .collect(),
    }
}

fn scalable_bounds(shape: &Shape) -> Option<Rect> {
    match &shape.geometry {
        Geometry::Polyline(_) => shape_bounds(shape),
        Geometry::Anchor(_) if shape.definition()?.outline == Outline::Disc => shape_bounds(shape),
        _ => None,
    }
}

fn box_handles(rect: Rect) -> Vec<(Handle, Point)> {
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

fn resized_rect(rect: Rect, handle: Handle, to: Point) -> Option<Rect> {
    let rect = normalized(rect);
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
        Handle::Start | Handle::End | Handle::Curve | Handle::Rotate => return None,
    }
    Some(rect_between(
        Point { x: left, y: top },
        Point {
            x: right,
            y: bottom,
        },
    ))
}

fn scaled_points(points: &[Point], from: Rect, to: Rect) -> Vec<Point> {
    let scale = |value: f32, origin: f32, size: f32, target_origin: f32, target_size: f32| {
        if size > 0.0 {
            target_origin + (value - origin) * target_size / size
        } else {
            value + target_origin - origin
        }
    };
    points
        .iter()
        .map(|point| Point {
            x: scale(point.x, from.x, from.width, to.x, to.width),
            y: scale(point.y, from.y, from.height, to.y, to.height),
        })
        .collect()
}

fn resized_badge(shape: &Shape, at: Point, to: Point) -> Shape {
    let size = (to.x - at.x)
        .abs()
        .max((to.y - at.y).abs())
        .clamp(SMALLEST_BADGE, LARGEST_BADGE);
    shape.with_option("size", number_to_value(size))
}

fn sides_of(handle: Handle) -> (bool, bool, bool, bool) {
    match handle {
        Handle::TopLeft => (true, false, true, false),
        Handle::Top => (false, false, true, false),
        Handle::TopRight => (false, true, true, false),
        Handle::Right => (false, true, false, false),
        Handle::BottomRight => (false, true, false, true),
        Handle::Bottom => (false, false, false, true),
        Handle::BottomLeft => (true, false, false, true),
        Handle::Left => (true, false, false, false),
        Handle::Start | Handle::End | Handle::Curve | Handle::Rotate => {
            (false, false, false, false)
        }
    }
}

fn resized_text(shape: &Shape, handle: Handle, to: Point, extent: (f32, f32)) -> Shape {
    let (Geometry::Anchor(at), Some(visual)) = (&shape.geometry, text_visual_bounds(shape, extent))
    else {
        return shape.clone();
    };
    let (left, right, top, bottom) = sides_of(handle);
    let size = shape.number("font_size");
    if !(left || right || top || bottom)
        || visual.width <= 0.0
        || visual.height <= 0.0
        || size <= 0.0
    {
        return shape.clone();
    }
    let fixed = Point {
        x: if left {
            visual.x + visual.width
        } else {
            visual.x
        },
        y: if top {
            visual.y + visual.height
        } else {
            visual.y
        },
    };
    let along_x = if left { fixed.x - to.x } else { to.x - fixed.x } / visual.width;
    let along_y = if top { fixed.y - to.y } else { to.y - fixed.y } / visual.height;
    let scale = match (left || right, top || bottom) {
        (true, true) => along_x.max(along_y),
        (true, false) => along_x,
        _ => along_y,
    };
    let new_size = (size * scale).clamp(SMALLEST_FONT, LARGEST_FONT);
    let factor = new_size / size;
    shape
        .with_option("font_size", number_to_value(new_size))
        .with_geometry(Geometry::Anchor(Point {
            x: fixed.x + (at.x - fixed.x) * factor,
            y: fixed.y + (at.y - fixed.y) * factor,
        }))
}

fn resized_unturned(shape: &Shape, handle: Handle, to: Point, extent: Option<(f32, f32)>) -> Shape {
    match extent {
        Some(extent) if shape.kind == "text" => resized_text(shape, handle, to, extent),
        _ => resized_geometry(shape, handle, to),
    }
}

fn resized_turned(shape: &Shape, handle: Handle, to: Point, extent: Option<(f32, f32)>) -> Shape {
    if !is_turned(shape) || (shape.kind == "text" && extent.is_none()) {
        return resized_unturned(shape, handle, to, extent);
    }
    let Some(pivot) = shape_pivot(shape, extent) else {
        return resized_unturned(shape, handle, to, extent);
    };
    let angle = rotation_of(shape);
    let local = resized_unturned(shape, handle, rotate_point(to, pivot, -angle), extent);
    let size = shape.number("font_size");
    let new_extent = extent.map(|(width, height)| {
        if shape.kind == "text" && size > 0.0 {
            let factor = local.number("font_size") / size;
            (width * factor, height * factor)
        } else {
            (width, height)
        }
    });
    let Some(new_pivot) = shape_pivot(&local, new_extent) else {
        return local;
    };
    let moved = rotate_point(new_pivot, pivot, angle);
    translated(&local, moved.x - new_pivot.x, moved.y - new_pivot.y)
}

#[must_use]
pub fn resized_with_extent(shape: &Shape, handle: Handle, to: Point, extent: (f32, f32)) -> Shape {
    resized_turned(shape, handle, to, Some(extent))
}

#[must_use]
pub fn rounded_spot_radius(bounds: Rect) -> f32 {
    bounds.width.min(bounds.height).max(0.0) * ROUNDED_SPOT_SHARE
}

#[must_use]
pub fn resized(shape: &Shape, handle: Handle, to: Point) -> Shape {
    resized_turned(shape, handle, to, None)
}

fn resized_geometry(shape: &Shape, handle: Handle, to: Point) -> Shape {
    match &shape.geometry {
        Geometry::Box(rect) => resized_rect(*rect, handle, to).map_or_else(
            || shape.clone(),
            |bounds| shape.with_geometry(Geometry::Box(bounds)),
        ),
        Geometry::TwoPoints { from, to: end } => match handle {
            Handle::Curve if shape.kind == "arrow" => {
                shape.with_option("curve", number_to_value(bend_through(*from, *end, to)))
            }
            Handle::Start => shape.with_geometry(Geometry::TwoPoints { from: to, to: *end }),
            Handle::End => shape.with_geometry(Geometry::TwoPoints { from: *from, to }),
            _ => shape.clone(),
        },
        Geometry::Polyline(points) => {
            let (Some(old), Some(new)) = (
                scalable_bounds(shape),
                scalable_bounds(shape).and_then(|bounds| resized_rect(bounds, handle, to)),
            ) else {
                return shape.clone();
            };
            shape.with_geometry(Geometry::Polyline(scaled_points(points, old, new)))
        }
        Geometry::Anchor(at) => {
            if scalable_bounds(shape).is_some()
                && !matches!(
                    handle,
                    Handle::Start | Handle::End | Handle::Curve | Handle::Rotate
                )
            {
                resized_badge(shape, *at, to)
            } else {
                shape.clone()
            }
        }
    }
}

fn arrow_middle(shape: &Shape, from: Point, to: Point) -> Point {
    match arrow_curve_control(shape) {
        Some(control) => curve_point(from, control, to, 0.5),
        None => Point {
            x: f32::midpoint(from.x, to.x),
            y: f32::midpoint(from.y, to.y),
        },
    }
}

const STRAIGHT_SNAP: f32 = 4.0;

fn bend_through(from: Point, to: Point, through: Point) -> f32 {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx.hypot(dy);
    if length <= 0.0 {
        return 0.0;
    }
    let (middle_x, middle_y) = (f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y));
    let side = ((through.x - middle_x) * -dy + (through.y - middle_y) * dx) / length;
    let bend = (side * 400.0 / length).clamp(-100.0, 100.0);
    if bend.abs() < STRAIGHT_SNAP {
        0.0
    } else {
        bend
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
pub fn text_ink_margin(shape: &Shape) -> f32 {
    let size = shape.number("font_size");
    let backdrop = if shape.choice("background") == "none" {
        0.0
    } else {
        size * TEXT_BACKDROP_SHARE
    };
    let outline = if shape.toggle("outline") {
        (size * TEXT_OUTLINE_SHARE).max(1.0)
    } else {
        0.0
    };
    backdrop.max(outline)
}

#[must_use]
pub fn text_visual_bounds(shape: &Shape, extent: (f32, f32)) -> Option<Rect> {
    let Geometry::Anchor(at) = shape.geometry else {
        return None;
    };
    let margin = text_ink_margin(shape);
    Some(Rect {
        x: at.x - margin,
        y: at.y - margin,
        width: extent.0 + 2.0 * margin,
        height: extent.1 + 2.0 * margin,
    })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitMode {
    Outline,
    Body,
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
    hit_in_mode(shape, point, tolerance, extent, HitMode::Outline)
}

#[must_use]
pub fn hit_in_mode(
    shape: &Shape,
    point: Point,
    tolerance: f32,
    extent: Option<(f32, f32)>,
    mode: HitMode,
) -> bool {
    let point = turned_back(shape, point, extent);
    hit_unturned(shape, point, tolerance, extent, mode)
}

fn hit_unturned(
    shape: &Shape,
    point: Point,
    tolerance: f32,
    extent: Option<(f32, f32)>,
    mode: HitMode,
) -> bool {
    let solid = mode == HitMode::Body || (mode == HitMode::Outline && shape.toggle("fill"));
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
            solid || !contains(expanded(rect, -reach), point)
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
            distance <= reach || (solid && scaled <= 1.0)
        }
        (Geometry::TwoPoints { from, to }, Outline::Segment) => {
            let on_shaft = match arrow_curve_control(shape) {
                Some(control) => curve_polyline(*from, control, *to)
                    .windows(2)
                    .any(|pair| distance_to_segment(point, pair[0], pair[1]) <= reach),
                None => distance_to_segment(point, *from, *to) <= reach,
            };
            on_shaft || on_arrow_head(shape, *from, *to, point, tolerance)
        }
        (Geometry::Box(bounds), Outline::Region) => {
            let rect = normalized(*bounds);
            contains(expanded(rect, tolerance), point)
        }
        (Geometry::Anchor(_), Outline::TextBox) => {
            let extent = extent.unwrap_or_else(|| estimated_text_extent(shape));
            text_visual_bounds(shape, extent)
                .is_some_and(|bounds| contains(expanded(bounds, tolerance), point))
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

fn on_arrow_head(shape: &Shape, from: Point, to: Point, point: Point, tolerance: f32) -> bool {
    if shape.kind != "arrow" || shape.choice("head") == "none" {
        return false;
    }
    let head = arrow_head(
        from,
        to,
        shape.number("width"),
        head_size_factor(shape.choice("head_size")),
    );
    distance_to_triangle(point, head.tip, head.left, head.right) <= tolerance
}

fn distance_to_triangle(point: Point, a: Point, b: Point, c: Point) -> f32 {
    let side = |from: Point, to: Point| {
        (to.x - from.x) * (point.y - from.y) - (to.y - from.y) * (point.x - from.x)
    };
    let (first, second, third) = (side(a, b), side(b, c), side(c, a));
    let inside = (first >= 0.0 && second >= 0.0 && third >= 0.0)
        || (first <= 0.0 && second <= 0.0 && third <= 0.0);
    if inside {
        return 0.0;
    }
    distance_to_segment(point, a, b)
        .min(distance_to_segment(point, b, c))
        .min(distance_to_segment(point, c, a))
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
