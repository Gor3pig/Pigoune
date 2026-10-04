pub const LARGEST_ZOOM: f64 = 32.0;
pub const ACTUAL_SIZE: f64 = 1.0;
pub const SHARP_PIXELS_FROM: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub fn fit_zoom(view: Size, image: Size, is_vector: bool) -> f64 {
    if image.width <= 0.0 || image.height <= 0.0 || view.width <= 0.0 || view.height <= 0.0 {
        return ACTUAL_SIZE;
    }
    let filling = (view.width / image.width).min(view.height / image.height);
    if is_vector {
        filling
    } else {
        filling.min(ACTUAL_SIZE)
    }
}

pub fn allowed_zoom(zoom: f64, fit: f64) -> f64 {
    zoom.clamp(fit.min(ACTUAL_SIZE), LARGEST_ZOOM)
}

pub fn zoom_around(center: Point, anchor_offset: Point, old_zoom: f64, new_zoom: f64) -> Point {
    let anchored = Point {
        x: center.x + anchor_offset.x / old_zoom,
        y: center.y + anchor_offset.y / old_zoom,
    };
    Point {
        x: anchored.x - anchor_offset.x / new_zoom,
        y: anchored.y - anchor_offset.y / new_zoom,
    }
}

pub fn clamp_center(center: Point, view: Size, image: Size, zoom: f64) -> Point {
    Point {
        x: clamp_axis(center.x, view.width, image.width, zoom),
        y: clamp_axis(center.y, view.height, image.height, zoom),
    }
}

fn clamp_axis(center: f64, view: f64, image: f64, zoom: f64) -> f64 {
    let half_visible = view / zoom / 2.0;
    center.clamp((image - half_visible).min(0.0), half_visible.max(image))
}

pub fn shows_sharp_pixels(
    zoom: f64,
    image_width: f64,
    texture_width: f64,
    is_vector: bool,
) -> bool {
    !is_vector && texture_width > 0.0 && zoom * image_width / texture_width >= SHARP_PIXELS_FROM
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Size = Size {
        width: 800.0,
        height: 600.0,
    };

    fn size(width: f64, height: f64) -> Size {
        Size { width, height }
    }

    #[test]
    fn large_images_shrink_to_fit_the_view() {
        assert!((fit_zoom(VIEW, size(1600.0, 600.0), false) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn small_pictures_keep_their_actual_size_but_drawings_fill_the_view() {
        assert!((fit_zoom(VIEW, size(16.0, 16.0), false) - 1.0).abs() < 1e-9);
        assert!((fit_zoom(VIEW, size(100.0, 100.0), true) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn zoom_stays_between_the_smallest_view_and_3200_percent() {
        assert!((allowed_zoom(100.0, 0.5) - LARGEST_ZOOM).abs() < 1e-9);
        assert!((allowed_zoom(0.1, 0.5) - 0.5).abs() < 1e-9);
        assert!((allowed_zoom(0.1, 6.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_point_under_the_pointer_stays_under_the_pointer() {
        let center = Point { x: 50.0, y: 50.0 };
        let offset = Point { x: 100.0, y: -40.0 };
        let moved = zoom_around(center, offset, 2.0, 4.0);
        let before_x = center.x + offset.x / 2.0;
        let after_x = moved.x + offset.x / 4.0;
        let before_y = center.y + offset.y / 2.0;
        let after_y = moved.y + offset.y / 4.0;
        assert!((before_x - after_x).abs() < 1e-9);
        assert!((before_y - after_y).abs() < 1e-9);
    }

    #[test]
    fn a_small_image_moves_anywhere_but_stays_whole_in_the_view() {
        let image = size(100.0, 100.0);
        let pushed_right_down = clamp_center(
            Point {
                x: -900.0,
                y: -900.0,
            },
            VIEW,
            image,
            1.0,
        );
        assert_eq!(
            pushed_right_down,
            Point {
                x: -300.0,
                y: -200.0
            }
        );
        let pushed_left_up = clamp_center(Point { x: 900.0, y: 900.0 }, VIEW, image, 1.0);
        assert_eq!(pushed_left_up, Point { x: 400.0, y: 300.0 });
        let moved = Point { x: 20.0, y: 70.0 };
        assert_eq!(clamp_center(moved, VIEW, image, 1.0), moved);
    }

    #[test]
    fn a_large_image_brings_any_corner_to_the_center_but_no_further() {
        let image = size(4000.0, 3000.0);
        let corner = clamp_center(
            Point {
                x: -50.0,
                y: 5000.0,
            },
            VIEW,
            image,
            1.0,
        );
        assert_eq!(corner, Point { x: 0.0, y: 3000.0 });
        let zoomed = clamp_center(
            Point {
                x: -50.0,
                y: 5000.0,
            },
            VIEW,
            image,
            2.0,
        );
        assert_eq!(zoomed, Point { x: 0.0, y: 3000.0 });
    }

    #[test]
    fn a_fitted_image_is_already_inside_the_limits() {
        let image = size(1600.0, 600.0);
        let fit = fit_zoom(VIEW, image, false);
        let centered = Point { x: 800.0, y: 300.0 };
        assert_eq!(clamp_center(centered, VIEW, image, fit), centered);
    }

    #[test]
    fn pixels_become_sharp_from_200_percent_on_pictures_only() {
        assert!(!shows_sharp_pixels(1.5, 16.0, 16.0, false));
        assert!(shows_sharp_pixels(2.0, 16.0, 16.0, false));
        assert!(!shows_sharp_pixels(8.0, 16.0, 16.0, true));
    }
}
