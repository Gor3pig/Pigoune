use super::{Point, Rect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub at: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapping {
    pub tolerance: f32,
    pub image: (f32, f32),
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
    let (width, height) = snapping.image;
    let mut horizontal_targets = vec![0.0, width / 2.0, width];
    let mut vertical_targets = vec![0.0, height / 2.0, height];
    for other in others {
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
pub fn guide_extent(guide: Guide, image: (f32, f32)) -> (Point, Point) {
    if guide.vertical {
        (
            Point {
                x: guide.at,
                y: 0.0,
            },
            Point {
                x: guide.at,
                y: image.1,
            },
        )
    } else {
        (
            Point {
                x: 0.0,
                y: guide.at,
            },
            Point {
                x: image.0,
                y: guide.at,
            },
        )
    }
}
