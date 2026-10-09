use std::ops::Range;

pub const LARGEST_ZOOM: f64 = 32.0;
pub const ACTUAL_SIZE: f64 = 1.0;
pub const SHARP_PIXELS_FROM: f64 = 2.0;
pub const PIXEL_GRID_FROM: f64 = 8.0;
const STRETCH_RESISTANCE: f64 = 0.55;
const PAN_STEP: f64 = 0.1;

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

pub fn pan_center(center: Point, view: Size, image: Size, zoom: f64, direction: Point) -> Point {
    Point {
        x: pan_axis(center.x, view.width, image.width, zoom, direction.x),
        y: pan_axis(center.y, view.height, image.height, zoom, direction.y),
    }
}

fn pan_axis(center: f64, view: f64, image: f64, zoom: f64, direction: f64) -> f64 {
    if image * zoom <= view {
        return center;
    }
    clamp_axis(
        center + direction * PAN_STEP * view / zoom,
        view,
        image,
        zoom,
    )
}

pub fn stretch_center(center: Point, view: Size, image: Size, zoom: f64) -> Point {
    Point {
        x: stretch_axis(center.x, view.width, image.width, zoom),
        y: stretch_axis(center.y, view.height, image.height, zoom),
    }
}

fn stretch_axis(center: f64, view: f64, image: f64, zoom: f64) -> f64 {
    let held = clamp_axis(center, view, image, zoom);
    let excess = (center - held) * zoom;
    if excess == 0.0 || view <= 0.0 {
        return held;
    }
    let resisted = view * (1.0 - 1.0 / (excess.abs() * STRETCH_RESISTANCE / view + 1.0));
    held + resisted.copysign(excess) / zoom
}

pub fn shows_pixel_grid(zoom: f64, is_vector: bool) -> bool {
    !is_vector && zoom >= PIXEL_GRID_FROM
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "line indexes are kept between zero and the image size"
)]
pub fn visible_grid_lines(start: f64, spacing: f64, pixels: u32, view: f64) -> Range<u32> {
    let first = (-start / spacing).ceil().max(0.0);
    let last = ((view - start) / spacing).floor().min(f64::from(pixels));
    if last < first {
        return 0..0;
    }
    (first as u32)..(last as u32 + 1)
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
    fn panning_moves_the_view_by_a_tenth_of_its_size() {
        let image = size(2000.0, 2000.0);
        let center = Point {
            x: 1000.0,
            y: 1000.0,
        };
        let moved = pan_center(
            center,
            size(500.0, 400.0),
            image,
            2.0,
            Point { x: 1.0, y: -1.0 },
        );
        assert!((moved.x - 1025.0).abs() < 1e-9);
        assert!((moved.y - 980.0).abs() < 1e-9);
    }

    #[test]
    fn panning_stops_at_the_edge_of_the_image() {
        let image = size(1000.0, 1000.0);
        let edge = Point { x: 990.0, y: 500.0 };
        let moved = pan_center(
            edge,
            size(500.0, 500.0),
            image,
            2.0,
            Point { x: 1.0, y: 0.0 },
        );
        assert!((moved.x - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn an_axis_where_the_image_fits_does_not_move() {
        let image = size(100.0, 2000.0);
        let center = Point { x: 50.0, y: 1000.0 };
        let moved = pan_center(
            center,
            size(500.0, 400.0),
            image,
            2.0,
            Point { x: 1.0, y: 1.0 },
        );
        assert!((moved.x - 50.0).abs() < 1e-9);
        assert!(moved.y > 1000.0);
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
    fn a_drag_within_the_limits_is_followed_exactly() {
        let image = size(400.0, 300.0);
        let inside = Point { x: 250.0, y: 160.0 };
        assert_eq!(stretch_center(inside, VIEW, image, 1.0), inside);
    }

    #[test]
    fn a_drag_beyond_the_limits_meets_growing_resistance() {
        let image = size(400.0, 300.0);
        let limit = clamp_center(
            Point {
                x: 10_000.0,
                y: 150.0,
            },
            VIEW,
            image,
            1.0,
        )
        .x;
        let little = stretch_center(
            Point {
                x: limit + 50.0,
                y: 150.0,
            },
            VIEW,
            image,
            1.0,
        )
        .x;
        let far = stretch_center(
            Point {
                x: limit + 5_000.0,
                y: 150.0,
            },
            VIEW,
            image,
            1.0,
        )
        .x;
        assert!(little > limit && little < limit + 50.0);
        assert!(far > little && far < limit + VIEW.width);
    }

    #[test]
    fn the_stretch_goes_the_way_of_the_drag() {
        let image = size(400.0, 300.0);
        let limit = clamp_center(
            Point {
                x: -10_000.0,
                y: 150.0,
            },
            VIEW,
            image,
            2.0,
        )
        .x;
        let stretched = stretch_center(
            Point {
                x: limit - 100.0,
                y: 150.0,
            },
            VIEW,
            image,
            2.0,
        )
        .x;
        assert!(stretched < limit && stretched > limit - 100.0);
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
    fn the_pixel_grid_appears_from_800_percent_on_pictures_only() {
        assert!(!shows_pixel_grid(7.9, false));
        assert!(shows_pixel_grid(8.0, false));
        assert!(!shows_pixel_grid(32.0, true));
    }

    #[test]
    fn only_the_grid_lines_inside_the_view_are_drawn() {
        assert_eq!(visible_grid_lines(100.0, 10.0, 16, 800.0), 0..17);
        assert_eq!(visible_grid_lines(-35.0, 10.0, 100, 50.0), 4..9);
        assert_eq!(visible_grid_lines(-5000.0, 10.0, 1000, 800.0), 500..581);
    }

    #[test]
    fn an_image_outside_the_view_draws_no_grid_line() {
        assert!(visible_grid_lines(900.0, 10.0, 16, 800.0).is_empty());
        assert!(visible_grid_lines(-500.0, 10.0, 16, 800.0).is_empty());
    }

    #[test]
    fn pixels_become_sharp_from_200_percent_on_pictures_only() {
        assert!(!shows_sharp_pixels(1.5, 16.0, 16.0, false));
        assert!(shows_sharp_pixels(2.0, 16.0, 16.0, false));
        assert!(!shows_sharp_pixels(8.0, 16.0, 16.0, true));
    }
}
