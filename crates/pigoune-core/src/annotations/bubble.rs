use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::geometry::shape_bounds;
use super::shape::Shape;
use super::{Point, Rect};

const OVAL_STEPS: u16 = 128;
const CORNER_STEPS: u16 = 12;
const SIDE_STEPS: u16 = 8;
const CORNER_SHARE: f32 = 0.3;
const TAIL_BASE_SHARE: f32 = 0.038;
const TAIL_BEND_SHARE: f32 = 0.2;
const TAIL_FAR_SIDE_SHARE: f32 = 0.35;
const TAIL_STEPS: u16 = 8;
const OVAL_TEXT_SHARE: f32 = 0.68;
const ROUNDED_TEXT_SHARE: f32 = 0.84;
const CLOUD_TEXT_SHARE: f32 = 0.6;
const SMALL_BUMP_SHARE: f32 = 0.26;
const MEDIUM_BUMP_SHARE: f32 = 0.33;
const BUMP_PUFF: f32 = 0.62;
const SMALLEST_BUMP_COUNT: u16 = 8;
const LARGEST_BUMP_COUNT: u16 = 48;
const BUMP_STEPS: u16 = 10;
const BUBBLE_STEPS: u16 = 32;
const SMALL_THOUGHT_WEIGHTS: [f32; 3] = [1.0, 0.64, 0.36];
const MEDIUM_THOUGHT_WEIGHTS: [f32; 2] = [1.0, 0.6];
const THOUGHT_GAP_SHARE: f32 = 0.35;
const LARGEST_THOUGHT_SHARE: f32 = 0.2;
const SMALLEST_THOUGHT_REACH: f32 = 2.0;

#[must_use]
pub fn bubble_tail(shape: &Shape) -> Option<Point> {
    let rect = shape_bounds(shape)?;
    Some(Point {
        x: rect.x + rect.width / 2.0 + shape.number("tail_x") * rect.width,
        y: rect.y + rect.height / 2.0 + shape.number("tail_y") * rect.height,
    })
}

#[must_use]
pub fn bubble_text_box(shape: &Shape) -> Option<Rect> {
    let rect = shape_bounds(shape)?;
    let fraction = match shape.choice("form") {
        "cloud" => CLOUD_TEXT_SHARE,
        "rounded" => ROUNDED_TEXT_SHARE,
        _ => OVAL_TEXT_SHARE,
    };
    let (width, height) = (rect.width * fraction, rect.height * fraction);
    Some(Rect {
        x: rect.x + (rect.width - width) / 2.0,
        y: rect.y + (rect.height - height) / 2.0,
        width,
        height,
    })
}

#[must_use]
pub fn bubble_parts(shape: &Shape) -> Option<Vec<Vec<Point>>> {
    let rect = shape_bounds(shape)?;
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }
    let tip = bubble_tail(shape)?;
    let centre = centre_of(rect);
    Some(match shape.choice("form") {
        "cloud" => {
            let medium = shape.choice("bumps") == "medium";
            let body = cloud_body(rect, medium);
            let weights: &[f32] = if medium {
                &MEDIUM_THOUGHT_WEIGHTS
            } else {
                &SMALL_THOUGHT_WEIGHTS
            };
            let circles = thought_circles(&body, centre, tip, weights);
            std::iter::once(body).chain(circles).collect()
        }
        "rounded" => vec![with_tail(&rounded_body(rect), centre, tip, false)],
        _ => vec![with_tail(&oval_body(rect), centre, tip, true)],
    })
}

#[must_use]
pub fn bubble_outline(shape: &Shape) -> Option<Vec<Point>> {
    bubble_parts(shape)?.into_iter().next()
}

#[must_use]
pub fn bubble_hit(shape: &Shape, point: Point, reach: f32) -> bool {
    let Some(parts) = bubble_parts(shape) else {
        return false;
    };
    let reach = reach.max(0.0);
    parts.iter().any(|outline| {
        inside(outline, point)
            || (0..outline.len()).any(|index| {
                let next = outline[(index + 1) % outline.len()];
                distance_to_segment(point, outline[index], next) <= reach
            })
    })
}

fn cloud_body(rect: Rect, medium: bool) -> Vec<Point> {
    let centre = centre_of(rect);
    let (half_width, half_height) = (rect.width / 2.0, rect.height / 2.0);
    let perimeter = PI
        * (3.0 * (half_width + half_height)
            - ((3.0 * half_width + half_height) * (half_width + 3.0 * half_height)).sqrt());
    let share = if medium {
        MEDIUM_BUMP_SHARE
    } else {
        SMALL_BUMP_SHARE
    };
    let wanted = (perimeter / (share * rect.width.min(rect.height))).round();
    let count = (0..=LARGEST_BUMP_COUNT)
        .rev()
        .find(|count| f32::from(*count) <= wanted)
        .unwrap_or(SMALLEST_BUMP_COUNT)
        .max(SMALLEST_BUMP_COUNT);
    let sagitta = BUMP_PUFF - (BUMP_PUFF * BUMP_PUFF - 0.25).sqrt();
    let base_with = |inset: f32| -> Vec<Point> {
        let (base_width, base_height) = (
            (half_width - inset).max(half_width / 2.0),
            (half_height - inset).max(half_height / 2.0),
        );
        (0..count)
            .map(|index| {
                let angle = step_ratio(index, count) * TAU;
                Point {
                    x: centre.x + base_width * angle.cos(),
                    y: centre.y + base_height * angle.sin(),
                }
            })
            .collect()
    };
    let longest_chord = |base: &[Point]| {
        (0..base.len())
            .map(|index| {
                let next = base[(index + 1) % base.len()];
                (next.x - base[index].x).hypot(next.y - base[index].y)
            })
            .fold(0.0, f32::max)
    };
    let first_pass = base_with(perimeter / f32::from(count) * sagitta);
    let base = base_with(longest_chord(&first_pass) * sagitta);
    let mut body = Vec::new();
    for (index, from) in base.iter().enumerate() {
        let to = base[(index + 1) % base.len()];
        let (chord_x, chord_y) = (to.x - from.x, to.y - from.y);
        let length = chord_x.hypot(chord_y).max(f32::EPSILON);
        let radius = length * BUMP_PUFF;
        let apothem = (radius * radius - length * length / 4.0).max(0.0).sqrt();
        let middle = lerp(*from, to, 0.5);
        let mut normal = (chord_y / length, -chord_x / length);
        if normal.0 * (middle.x - centre.x) + normal.1 * (middle.y - centre.y) < 0.0 {
            normal = (-normal.0, -normal.1);
        }
        body.extend((0..BUMP_STEPS).map(|step| {
            let offset = (step_ratio(step, BUMP_STEPS) - 0.5) * length;
            let swell = (radius * radius - offset * offset).max(0.0).sqrt() - apothem;
            let on_chord = lerp(*from, to, step_ratio(step, BUMP_STEPS));
            Point {
                x: on_chord.x + normal.0 * swell,
                y: on_chord.y + normal.1 * swell,
            }
        }));
    }
    body
}

fn farthest_crossing(body: &[Point], centre: Point, tip: Point) -> Option<Point> {
    let direction = from_centre(centre, tip);
    (0..body.len())
        .filter_map(|index| {
            let (from, to) = (body[index], body[(index + 1) % body.len()]);
            let (before, after) = (
                cross(direction, from_centre(centre, from)),
                cross(direction, from_centre(centre, to)),
            );
            let ahead = direction.0 * (from.x + to.x - 2.0 * centre.x)
                + direction.1 * (from.y + to.y - 2.0 * centre.y)
                > 0.0;
            (ahead && before <= 0.0 && after > 0.0)
                .then(|| lerp(from, to, before / (before - after)))
        })
        .max_by(|left, right| {
            let reach = |point: &Point| (point.x - centre.x).hypot(point.y - centre.y);
            reach(left).total_cmp(&reach(right))
        })
}

fn circle_points(centre: Point, radius: f32) -> Vec<Point> {
    (0..BUBBLE_STEPS)
        .map(|index| {
            let angle = step_ratio(index, BUBBLE_STEPS) * TAU;
            Point {
                x: centre.x + radius * angle.cos(),
                y: centre.y + radius * angle.sin(),
            }
        })
        .collect()
}

fn thought_circles(body: &[Point], centre: Point, tip: Point, weights: &[f32]) -> Vec<Vec<Point>> {
    if inside(body, tip) {
        return Vec::new();
    }
    let Some(root) = farthest_crossing(body, centre, tip) else {
        return Vec::new();
    };
    let reach = (tip.x - root.x).hypot(tip.y - root.y);
    if reach < SMALLEST_THOUGHT_REACH {
        return Vec::new();
    }
    let direction = ((tip.x - root.x) / reach, (tip.y - root.y) / reach);
    let total_weight: f32 = weights.iter().sum::<f32>() * 2.0;
    let widest = (reach / (total_weight + 2.0 * THOUGHT_GAP_SHARE))
        .min(centre_span(body, centre) * LARGEST_THOUGHT_SHARE);
    let gap = (reach - widest * total_weight) / 2.0;
    let mut travelled = 0.0;
    weights
        .iter()
        .map(|weight| {
            let radius = widest * weight;
            let middle = travelled + radius;
            travelled += 2.0 * radius + gap;
            circle_points(
                Point {
                    x: root.x + direction.0 * middle,
                    y: root.y + direction.1 * middle,
                },
                radius,
            )
        })
        .collect()
}

fn centre_span(body: &[Point], centre: Point) -> f32 {
    body.iter()
        .map(|point| (point.x - centre.x).hypot(point.y - centre.y))
        .fold(f32::INFINITY, f32::min)
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

fn with_tail(body: &[Point], centre: Point, tip: Point, curved: bool) -> Vec<Point> {
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
    let bend = if curved {
        axis_x.hypot(axis_y) * TAIL_BEND_SHARE
    } else {
        0.0
    };
    let length = axis_x.hypot(axis_y).max(f32::EPSILON);
    let sideways = (-axis_y / length * bend, axis_x / length * bend);
    let control = |from: Point, to: Point, share: f32| Point {
        x: f32::midpoint(from.x, to.x) + sideways.0 * share,
        y: f32::midpoint(from.y, to.y) + sideways.1 * share,
    };
    let mut outline = vec![last];
    outline.extend(kept.into_iter().map(|(_, point)| point));
    outline.push(first);
    outline.extend(quad_samples(first, control(first, tip, 1.0), tip));
    outline.extend(quad_samples(
        tip,
        control(tip, last, TAIL_FAR_SIDE_SHARE),
        last,
    ));
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
