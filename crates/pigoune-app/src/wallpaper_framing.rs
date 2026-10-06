use crate::desktop_frame::Frame;
use crate::zoom_math::{Point, Size};

pub const SMALLEST_SCALE: f64 = 0.01;
pub const LARGEST_SCALE: f64 = 8.0;
pub const ACTUAL_SCALE: f64 = 1.0;
pub const BLUR_SHARE: f64 = 0.03;
pub const BLUR_BRIGHTNESS: f64 = 0.85;

pub const LARGEST_DARKNESS: f64 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub backdrop: Backdrop,
    pub mirrored: bool,
    pub darkness: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backdrop {
    Color([u8; 3]),
    Gradient([u8; 3], [u8; 3], u16),
    Blur,
    Mosaic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Framing {
    pub scale: f64,
    pub offset: Point,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisiblePart {
    pub source: (u32, u32, u32, u32),
    pub target: (u32, u32, u32, u32),
}

impl Framing {
    #[must_use]
    pub fn filling(image: Size, screen: Size) -> Self {
        centered((screen.width / image.width.max(1.0)).max(screen.height / image.height.max(1.0)))
    }

    #[must_use]
    pub fn whole(image: Size, screen: Size) -> Self {
        centered((screen.width / image.width.max(1.0)).min(screen.height / image.height.max(1.0)))
    }

    #[must_use]
    pub fn image_rect(self, image: Size, screen: Size) -> Frame {
        let width = image.width * self.scale;
        let height = image.height * self.scale;
        Frame {
            x: screen.width / 2.0 + self.offset.x - width / 2.0,
            y: screen.height / 2.0 + self.offset.y - height / 2.0,
            width,
            height,
        }
    }

    #[must_use]
    pub fn moved(self, by: Point) -> Self {
        Self {
            offset: Point {
                x: self.offset.x + by.x,
                y: self.offset.y + by.y,
            },
            ..self
        }
    }

    #[must_use]
    pub fn zoomed_to(self, scale: f64, anchor: Point, screen: Size) -> Self {
        let scale = scale.clamp(SMALLEST_SCALE, LARGEST_SCALE);
        let ratio = scale / self.scale;
        let center = Point {
            x: screen.width / 2.0 + self.offset.x,
            y: screen.height / 2.0 + self.offset.y,
        };
        let new_center = Point {
            x: anchor.x + (center.x - anchor.x) * ratio,
            y: anchor.y + (center.y - anchor.y) * ratio,
        };
        Self {
            scale,
            offset: Point {
                x: new_center.x - screen.width / 2.0,
                y: new_center.y - screen.height / 2.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Guides {
    pub across: Option<f64>,
    pub down: Option<f64>,
}

impl Framing {
    #[must_use]
    pub fn snapped(self, image: Size, screen: Size, tolerance: f64) -> (Self, Guides) {
        let rect = self.image_rect(image, screen);
        let (shift_x, across) = snap_axis(rect.x, rect.width, screen.width, tolerance);
        let (shift_y, down) = snap_axis(rect.y, rect.height, screen.height, tolerance);
        (
            self.moved(Point {
                x: shift_x,
                y: shift_y,
            }),
            Guides { across, down },
        )
    }
}

fn snap_axis(start: f64, length: f64, screen: f64, tolerance: f64) -> (f64, Option<f64>) {
    let center = start + length / 2.0;
    let candidates = [
        (screen / 2.0 - center, screen / 2.0),
        (-start, 0.0),
        (screen - (start + length), screen),
    ];
    candidates
        .into_iter()
        .filter(|(shift, _)| shift.abs() <= tolerance)
        .min_by(|first, second| first.0.abs().total_cmp(&second.0.abs()))
        .map_or((0.0, None), |(shift, line)| (shift, Some(line)))
}

fn centered(scale: f64) -> Framing {
    Framing {
        scale,
        offset: Point { x: 0.0, y: 0.0 },
    }
}

#[must_use]
pub fn visible_part(texture: (u32, u32), image: Frame, screen: (u32, u32)) -> Option<VisiblePart> {
    let left = image.x.max(0.0);
    let top = image.y.max(0.0);
    let right = (image.x + image.width).min(f64::from(screen.0));
    let bottom = (image.y + image.height).min(f64::from(screen.1));
    let target = (
        whole(left),
        whole(top),
        whole(right).saturating_sub(whole(left)),
        whole(bottom).saturating_sub(whole(top)),
    );
    if target.2 == 0 || target.3 == 0 {
        return None;
    }
    let across = f64::from(texture.0) / image.width;
    let down = f64::from(texture.1) / image.height;
    let source_left = whole((left - image.x) * across).min(texture.0 - 1);
    let source_top = whole((top - image.y) * down).min(texture.1 - 1);
    let source_right = whole((right - image.x) * across).clamp(source_left + 1, texture.0);
    let source_bottom = whole((bottom - image.y) * down).clamp(source_top + 1, texture.1);
    Some(VisiblePart {
        source: (
            source_left,
            source_top,
            source_right - source_left,
            source_bottom - source_top,
        ),
        target,
    })
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the position is rounded and clamped to the range of a side"
)]
fn whole(value: f64) -> u32 {
    value.round().clamp(0.0, f64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::{Framing, Guides, VisiblePart, visible_part};
    use crate::desktop_frame::Frame;
    use crate::zoom_math::{Point, Size};

    const SCREEN: Size = Size {
        width: 1920.0,
        height: 1080.0,
    };
    const PHOTO: Size = Size {
        width: 3000.0,
        height: 2000.0,
    };

    #[test]
    fn filling_covers_the_screen_and_whole_shows_the_entire_image() {
        let filling = Framing::filling(PHOTO, SCREEN);
        assert!((filling.scale - 0.64).abs() < f64::EPSILON);
        assert_eq!(
            filling.image_rect(PHOTO, SCREEN),
            Frame {
                x: 0.0,
                y: -100.0,
                width: 1920.0,
                height: 1280.0,
            }
        );
        assert!((Framing::whole(PHOTO, SCREEN).scale - 0.54).abs() < f64::EPSILON);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer_in_place() {
        let start = Framing {
            scale: 1.0,
            offset: Point { x: 0.0, y: 0.0 },
        };
        let anchor = Point { x: 0.0, y: 0.0 };
        let zoomed = start.zoomed_to(2.0, anchor, SCREEN);
        let rect = zoomed.image_rect(PHOTO, SCREEN);
        let before = start.image_rect(PHOTO, SCREEN);
        assert_eq!((rect.x, rect.y), (before.x * 2.0, before.y * 2.0));
        assert!((start.zoomed_to(100.0, anchor, SCREEN).scale - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn moving_shifts_the_image() {
        let moved = Framing::filling(PHOTO, SCREEN).moved(Point { x: 10.0, y: -5.0 });
        assert_eq!(moved.offset, Point { x: 10.0, y: -5.0 });
    }

    #[test]
    fn only_the_part_on_the_screen_is_kept() {
        let rect = Frame {
            x: -960.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        assert_eq!(
            visible_part((3840, 2160), rect, (1920, 1080)),
            Some(VisiblePart {
                source: (1920, 0, 1920, 2160),
                target: (0, 0, 960, 1080),
            })
        );
        let gone = Frame { x: 2000.0, ..rect };
        assert_eq!(visible_part((3840, 2160), gone, (1920, 1080)), None);
    }

    #[test]
    fn a_nearby_image_snaps_to_the_center_and_the_edges() {
        let near_center = Framing {
            scale: 0.5,
            offset: Point { x: 6.0, y: -80.0 },
        };
        let (snapped, guides) = near_center.snapped(PHOTO, SCREEN, 10.0);
        assert_eq!(snapped.offset, Point { x: 0.0, y: -80.0 });
        assert_eq!(
            guides,
            Guides {
                across: Some(960.0),
                down: None,
            }
        );
        let near_left = Framing {
            scale: 0.5,
            offset: Point { x: -205.0, y: 0.0 },
        };
        let (snapped, guides) = near_left.snapped(PHOTO, SCREEN, 10.0);
        assert!(snapped.image_rect(PHOTO, SCREEN).x.abs() < f64::EPSILON);
        assert_eq!(guides.across, Some(0.0));
        assert_eq!(guides.down, Some(540.0));
        let far = Framing {
            scale: 0.5,
            offset: Point { x: 100.0, y: 100.0 },
        };
        assert_eq!(far.snapped(PHOTO, SCREEN, 10.0), (far, Guides::default()));
    }
}
