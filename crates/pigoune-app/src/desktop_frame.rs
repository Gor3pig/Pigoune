use crate::zoom_math::Size;

const MARGIN: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[must_use]
pub fn screen_frame(view: Size, screen: Size) -> Frame {
    let room = Size {
        width: (view.width - 2.0 * MARGIN).max(1.0),
        height: (view.height - 2.0 * MARGIN).max(1.0),
    };
    let scale = (room.width / screen.width.max(1.0)).min(room.height / screen.height.max(1.0));
    centered(view.width / 2.0, view.height / 2.0, screen, scale)
}

#[must_use]
pub fn covering_frame(screen: Frame, image: Size) -> Frame {
    let scale = (screen.width / image.width.max(1.0)).max(screen.height / image.height.max(1.0));
    centered(
        screen.x + screen.width / 2.0,
        screen.y + screen.height / 2.0,
        image,
        scale,
    )
}

fn centered(center_x: f64, center_y: f64, size: Size, scale: f64) -> Frame {
    let width = size.width * scale;
    let height = size.height * scale;
    Frame {
        x: center_x - width / 2.0,
        y: center_y - height / 2.0,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::{Frame, covering_frame, screen_frame};
    use crate::zoom_math::Size;

    const FULL_HD: Size = Size {
        width: 1920.0,
        height: 1080.0,
    };

    #[test]
    fn the_screen_is_as_large_as_the_view_allows_with_a_margin() {
        let wide = screen_frame(
            Size {
                width: 1000.0,
                height: 1000.0,
            },
            FULL_HD,
        );
        assert_eq!(
            wide,
            Frame {
                x: 24.0,
                y: 232.25,
                width: 952.0,
                height: 535.5,
            }
        );
        let tall = screen_frame(
            Size {
                width: 2000.0,
                height: 588.0,
            },
            FULL_HD,
        );
        assert_eq!((tall.width, tall.height), (960.0, 540.0));
        assert_eq!((tall.x, tall.y), (520.0, 24.0));
    }

    #[test]
    fn the_image_covers_the_screen_like_the_desktop_zoom() {
        let screen = Frame {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let portrait = covering_frame(
            screen,
            Size {
                width: 600.0,
                height: 800.0,
            },
        );
        assert_eq!(
            portrait,
            Frame {
                x: 0.0,
                y: -740.0,
                width: 1920.0,
                height: 2560.0,
            }
        );
        let panorama = covering_frame(
            screen,
            Size {
                width: 4000.0,
                height: 1000.0,
            },
        );
        assert_eq!((panorama.x, panorama.height), (-1200.0, 1080.0));
    }
}
