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
    if half_visible * 2.0 >= image {
        image / 2.0
    } else {
        center.clamp(half_visible, image - half_visible)
    }
}

pub fn can_pan(view: Size, image: Size, zoom: f64) -> bool {
    image.width * zoom > view.width.ceil() || image.height * zoom > view.height.ceil()
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
    fn a_small_image_stays_centered_and_a_large_one_cannot_leave_the_view() {
        let small = clamp_center(Point { x: 0.0, y: 0.0 }, VIEW, size(100.0, 100.0), 1.0);
        assert_eq!(small, Point { x: 50.0, y: 50.0 });
        let large = clamp_center(Point { x: 0.0, y: 5000.0 }, VIEW, size(4000.0, 3000.0), 1.0);
        assert_eq!(
            large,
            Point {
                x: 400.0,
                y: 2700.0
            }
        );
    }

    #[test]
    fn only_an_image_larger_than_the_view_can_be_moved() {
        assert!(!can_pan(VIEW, size(800.0, 600.0), 1.0));
        assert!(!can_pan(VIEW, size(1600.0, 1200.0), 0.5));
        assert!(can_pan(VIEW, size(801.0, 100.0), 1.0));
        assert!(can_pan(VIEW, size(100.0, 100.0), 7.0));
    }

    #[test]
    fn pixels_become_sharp_from_200_percent_on_pictures_only() {
        assert!(!shows_sharp_pixels(1.5, 16.0, 16.0, false));
        assert!(shows_sharp_pixels(2.0, 16.0, 16.0, false));
        assert!(!shows_sharp_pixels(8.0, 16.0, 16.0, true));
    }
}
