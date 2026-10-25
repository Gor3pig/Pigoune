use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::geometry::shape_bounds;
use super::shape::Shape;
use super::{Point, Rect};

const OVAL_STEPS: u16 = 128;
const CORNER_STEPS: u16 = 12;
const SIDE_STEPS: u16 = 8;
const CORNER_SHARE: f32 = 0.3;
const TAIL_BASE_SHARE: f32 = 0.14;
const TAIL_BEND_SHARE: f32 = 0.12;
const TAIL_STEPS: u16 = 8;

#[must_use]
pub fn bubble_tail(shape: &Shape) -> Option<Point> {
    let rect = shape_bounds(shape)?;
    Some(Point {
        x: rect.x + rect.width / 2.0 + shape.number("tail_x") * rect.width,
        y: rect.y + rect.height / 2.0 + shape.number("tail_y") * rect.height,
    })
}

#[must_use]
pub fn bubble_outline(shape: &Shape) -> Option<Vec<Point>> {
    let rect = shape_bounds(shape)?;
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }
    let body = if shape.choice("form") == "rounded" {
        rounded_body(rect)
    } else {
        oval_body(rect)
    };
    let tip = bubble_tail(shape)?;
    Some(with_tail(&body, centre_of(rect), tip))
}

#[must_use]
pub fn bubble_hit(shape: &Shape, point: Point, reach: f32) -> bool {
    let Some(outline) = bubble_outline(shape) else {
        return false;
    };
    if inside(&outline, point) {
        return true;
    }
    let reach = reach.max(0.0);
    (0..outline.len()).any(|index| {
        let next = outline[(index + 1) % outline.len()];
        distance_to_segment(point, outline[index], next) <= reach
    })
}

fn centre_of(rect: Rect) -> Point {
    Point {
        x: rect.x + rect.width / 2.0,
        y: rect.y + rect.height / 2.0,
    }
}

fn step_ratio(index: u16, steps: u16) -> f32 {
    f32::from(index) / f32::from(steps)
}

fn oval_body(rect: Rect) -> Vec<Point> {
    let centre = centre_of(rect);
    (0..OVAL_STEPS)
        .map(|index| {
            let angle = step_ratio(index, OVAL_STEPS) * TAU;
            Point {
                x: centre.x + rect.width / 2.0 * angle.cos(),
                y: centre.y + rect.height / 2.0 * angle.sin(),
            }
        })
        .collect()
}

fn rounded_body(rect: Rect) -> Vec<Point> {
    let radius = rect.width.min(rect.height) * CORNER_SHARE;
    let (left, top) = (rect.x + radius, rect.y + radius);
    let (right, bottom) = (rect.x + rect.width - radius, rect.y + rect.height - radius);
    let arcs: Vec<Vec<Point>> = [
        (right, top, -FRAC_PI_2),
        (right, bottom, 0.0),
        (left, bottom, FRAC_PI_2),
        (left, top, PI),
    ]
    .into_iter()
    .map(|(x, y, start)| {
        (0..=CORNER_STEPS)
            .map(|index| {
                let angle = start + step_ratio(index, CORNER_STEPS) * FRAC_PI_2;
                Point {
                    x: x + radius * angle.cos(),
                    y: y + radius * angle.sin(),
                }
            })
            .collect()
    })
    .collect();
    let mut body = Vec::new();
    for (index, arc) in arcs.iter().enumerate() {
        body.extend(arc.iter().copied());
        let (from, to) = (arc[arc.len() - 1], arcs[(index + 1) % arcs.len()][0]);
        body.extend((1..SIDE_STEPS).map(|step| lerp(from, to, step_ratio(step, SIDE_STEPS))));
    }
    body
}

fn lerp(from: Point, to: Point, share: f32) -> Point {
    Point {
        x: from.x + (to.x - from.x) * share,
        y: from.y + (to.y - from.y) * share,
    }
}

fn cross(left: (f32, f32), right: (f32, f32)) -> f32 {
    left.0 * right.1 - left.1 * right.0
}

fn from_centre(centre: Point, point: Point) -> (f32, f32) {
    (point.x - centre.x, point.y - centre.y)
}

fn cumulative_lengths(body: &[Point]) -> Vec<f32> {
    let mut lengths = Vec::with_capacity(body.len() + 1);
    let mut total = 0.0;
    for (index, point) in body.iter().enumerate() {
        lengths.push(total);
        let next = body[(index + 1) % body.len()];
        total += (next.x - point.x).hypot(next.y - point.y);
    }
    lengths.push(total);
    lengths
}

fn arc_position(body: &[Point], lengths: &[f32], centre: Point, tip: Point) -> Option<f32> {
    let direction = from_centre(centre, tip);
    (0..body.len()).find_map(|index| {
        let (from, to) = (body[index], body[(index + 1) % body.len()]);
        let (before, after) = (
            cross(direction, from_centre(centre, from)),
            cross(direction, from_centre(centre, to)),
        );
        let ahead = direction.0 * (from.x + to.x - 2.0 * centre.x)
            + direction.1 * (from.y + to.y - 2.0 * centre.y)
            > 0.0;
        (ahead && before <= 0.0 && after > 0.0).then(|| {
            let share = before / (before - after);
            lengths[index] + share * (lengths[index + 1] - lengths[index])
        })
    })
}

fn point_at(body: &[Point], lengths: &[f32], position: f32) -> Point {
    let total = lengths[body.len()];
    let position = position.rem_euclid(total);
    let index = lengths[..body.len()]
        .partition_point(|length| *length <= position)
        .saturating_sub(1);
    let (from, to) = (body[index], body[(index + 1) % body.len()]);
    let span = lengths[index + 1] - lengths[index];
    let share = if span > 0.0 {
        (position - lengths[index]) / span
    } else {
        0.0
    };
    lerp(from, to, share)
}

fn quad_samples(from: Point, control: Point, to: Point) -> impl Iterator<Item = Point> {
    (1..=TAIL_STEPS).map(move |step| {
        let t = step_ratio(step, TAIL_STEPS);
        let (a, b, c) = ((1.0 - t) * (1.0 - t), 2.0 * (1.0 - t) * t, t * t);
        Point {
            x: a * from.x + b * control.x + c * to.x,
            y: a * from.y + b * control.y + c * to.y,
        }
    })
}

fn with_tail(body: &[Point], centre: Point, tip: Point) -> Vec<Point> {
    if inside(body, tip) {
        return body.to_vec();
    }
    let lengths = cumulative_lengths(body);
    let total = lengths[body.len()];
    let Some(attach) = arc_position(body, &lengths, centre, tip) else {
        return body.to_vec();
    };
    let half = (total * TAIL_BASE_SHARE / 2.0).min(total / 8.0);
    let (first, last) = (
        point_at(body, &lengths, attach - half),
        point_at(body, &lengths, attach + half),
    );
    let mut kept: Vec<(f32, Point)> = body
        .iter()
        .zip(&lengths)
        .map(|(point, length)| ((length - (attach + half)).rem_euclid(total), *point))
        .filter(|(along, _)| *along > 0.0 && *along < total - 2.0 * half)
        .collect();
    kept.sort_by(|left, right| left.0.total_cmp(&right.0));
    let root = point_at(body, &lengths, attach);
    let (axis_x, axis_y) = (tip.x - root.x, tip.y - root.y);
    let bend = axis_x.hypot(axis_y) * TAIL_BEND_SHARE;
    let length = axis_x.hypot(axis_y).max(f32::EPSILON);
    let sideways = (-axis_y / length * bend, axis_x / length * bend);
    let control = |from: Point, to: Point| Point {
        x: f32::midpoint(from.x, to.x) + sideways.0,
        y: f32::midpoint(from.y, to.y) + sideways.1,
    };
    let mut outline = vec![last];
    outline.extend(kept.into_iter().map(|(_, point)| point));
    outline.push(first);
    outline.extend(quad_samples(first, control(first, tip), tip));
    outline.extend(quad_samples(tip, control(tip, last), last));
    outline.pop();
    outline
}

fn inside(polygon: &[Point], point: Point) -> bool {
    let mut crossings = false;
    for (index, from) in polygon.iter().enumerate() {
        let to = polygon[(index + 1) % polygon.len()];
        if (from.y > point.y) != (to.y > point.y)
            && point.x < (to.x - from.x) * (point.y - from.y) / (to.y - from.y) + from.x
        {
            crossings = !crossings;
        }
    }
    crossings
}

fn distance_to_segment(point: Point, from: Point, to: Point) -> f32 {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length_squared = dx * dx + dy * dy;
    let along = if length_squared == 0.0 {
        0.0
    } else {
        (((point.x - from.x) * dx + (point.y - from.y) * dy) / length_squared).clamp(0.0, 1.0)
    };
    (point.x - (from.x + along * dx)).hypot(point.y - (from.y + along * dy))
}
