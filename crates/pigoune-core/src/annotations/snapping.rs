use super::{Point, Rect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub at: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapping {
    pub tolerance: f32,
    pub area: Rect,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snap {
    pub shift: (f32, f32),
    pub guides: Vec<Guide>,
}

fn lines_of(low: f32, size: f32) -> [f32; 3] {
    [low, low + size / 2.0, low + size]
}

fn closest(moving: [f32; 3], targets: &[f32], tolerance: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for line in moving {
        for target in targets {
            let delta = target - line;
            if delta.abs() <= tolerance && best.is_none_or(|(kept, _)| delta.abs() < kept.abs()) {
                best = Some((delta, *target));
            }
        }
    }
    best
}

#[must_use]
pub fn snap_move(moving: Rect, others: &[Rect], snapping: Snapping) -> Snap {
    let area = snapping.area;
    let mut horizontal_targets = lines_of(area.x, area.width).to_vec();
    let mut vertical_targets = lines_of(area.y, area.height).to_vec();
    for other in others.iter().filter(|other| overlaps(area, other)) {
        horizontal_targets.extend(lines_of(other.x, other.width));
        vertical_targets.extend(lines_of(other.y, other.height));
    }
    let across = closest(
        lines_of(moving.x, moving.width),
        &horizontal_targets,
        snapping.tolerance,
    );
    let down = closest(
        lines_of(moving.y, moving.height),
        &vertical_targets,
        snapping.tolerance,
    );
    let mut guides = Vec::new();
    if let Some((_, at)) = across {
        guides.push(Guide { vertical: true, at });
    }
    if let Some((_, at)) = down {
        guides.push(Guide {
            vertical: false,
            at,
        });
    }
    Snap {
        shift: (
            across.map_or(0.0, |(delta, _)| delta),
            down.map_or(0.0, |(delta, _)| delta),
        ),
        guides,
    }
}

#[must_use]
pub fn guide_extent(guide: Guide, area: Rect) -> (Point, Point) {
    if guide.vertical {
        (
            Point {
                x: guide.at,
                y: area.y,
            },
            Point {
                x: guide.at,
                y: area.y + area.height,
            },
        )
    } else {
        (
            Point {
                x: area.x,
                y: guide.at,
            },
            Point {
                x: area.x + area.width,
                y: guide.at,
            },
        )
    }
}

fn overlaps(area: Rect, other: &Rect) -> bool {
    other.x <= area.x + area.width
        && other.x + other.width >= area.x
        && other.y <= area.y + area.height
        && other.y + other.height >= area.y
}
