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
    use super::{Frame, screen_frame};
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
}
