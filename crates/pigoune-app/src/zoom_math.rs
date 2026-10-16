use std::ops::Range;

pub const LARGEST_ZOOM: f64 = 32.0;
pub const ACTUAL_SIZE: f64 = 1.0;
pub const SHARP_PIXELS_FROM: f64 = 2.0;
pub const PIXEL_GRID_FROM: f64 = 8.0;
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
    if image <= 2.0 * half_visible {
        image / 2.0
    } else {
        center.clamp(half_visible, image - half_visible)
    }
}

pub fn can_pan(view: Size, image: Size, zoom: f64) -> bool {
    image.width * zoom > view.width || image.height * zoom > view.height
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
        assert!((moved.x - 875.0).abs() < 1e-9);
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

    fn centered(image: Size) -> Point {
        Point {
            x: image.width / 2.0,
            y: image.height / 2.0,
        }
    }

    #[test]
    fn an_image_that_fits_on_both_axes_stays_centered_and_immobile() {
        for image in [size(100.0, 100.0), size(800.0, 600.0), size(799.0, 1.0)] {
            for asked in [
                Point {
                    x: -900.0,
                    y: -900.0,
                },
                Point { x: 900.0, y: 900.0 },
                Point { x: 17.0, y: 311.0 },
                centered(image),
            ] {
                assert_eq!(clamp_center(asked, VIEW, image, 1.0), centered(image));
            }
            assert!(!can_pan(VIEW, image, 1.0));
        }
    }

    #[test]
    fn an_image_fitted_exactly_to_the_view_is_immobile() {
        let image = size(1600.0, 1200.0);
        let fit = fit_zoom(VIEW, image, false);
        assert!((fit - 0.5).abs() < 1e-9);
        for asked in [
            Point { x: 0.0, y: 0.0 },
            Point {
                x: 5000.0,
                y: -5000.0,
            },
        ] {
            assert_eq!(clamp_center(asked, VIEW, image, fit), centered(image));
        }
        assert!(!can_pan(VIEW, image, fit));
    }

    #[test]
    fn an_image_wider_than_the_view_only_moves_horizontally_within_its_edges() {
        let image = size(4000.0, 300.0);
        let left = clamp_center(Point { x: -50.0, y: 0.0 }, VIEW, image, 1.0);
        let right = clamp_center(
            Point {
                x: 9000.0,
                y: 900.0,
            },
            VIEW,
            image,
            1.0,
        );
        assert_eq!(left, Point { x: 400.0, y: 150.0 });
        assert_eq!(
            right,
            Point {
                x: 3600.0,
                y: 150.0
            }
        );
        let inside = Point { x: 1234.0, y: 77.0 };
        assert_eq!(
            clamp_center(inside, VIEW, image, 1.0),
            Point {
                x: 1234.0,
                y: 150.0
            }
        );
        assert!(can_pan(VIEW, image, 1.0));
    }

    #[test]
    fn an_image_taller_than_the_view_only_moves_vertically_within_its_edges() {
        let image = size(300.0, 5000.0);
        let top = clamp_center(Point { x: 0.0, y: -10.0 }, VIEW, image, 1.0);
        let bottom = clamp_center(
            Point {
                x: 900.0,
                y: 9000.0,
            },
            VIEW,
            image,
            1.0,
        );
        assert_eq!(top, Point { x: 150.0, y: 300.0 });
        assert_eq!(
            bottom,
            Point {
                x: 150.0,
                y: 4700.0
            }
        );
        assert!(can_pan(VIEW, image, 1.0));
    }

    #[test]
    fn an_image_larger_on_both_axes_moves_on_both_but_never_shows_empty_space() {
        let image = size(4000.0, 3000.0);
        let corner = clamp_center(
            Point {
                x: -50.0,
                y: 5000.0,
            },
            VIEW,
            image,
            2.0,
        );
        assert_eq!(
            corner,
            Point {
                x: 200.0,
                y: 2850.0
            }
        );
        let visible_left = corner.x - VIEW.width / 2.0 / 2.0;
        let visible_bottom = corner.y + VIEW.height / 2.0 / 2.0;
        assert!((visible_left - 0.0).abs() < 1e-9);
        assert!((visible_bottom - 3000.0).abs() < 1e-9);
    }

    #[test]
    fn zooming_out_until_the_image_fits_recenters_it() {
        let image = size(1600.0, 1200.0);
        let corner = clamp_center(Point { x: 0.0, y: 0.0 }, VIEW, image, 2.0);
        assert_eq!(corner, Point { x: 200.0, y: 150.0 });
        let zoomed = zoom_around(
            corner,
            Point {
                x: -400.0,
                y: -300.0,
            },
            2.0,
            0.5,
        );
        assert_eq!(clamp_center(zoomed, VIEW, image, 0.5), centered(image));
    }

    #[test]
    fn a_view_that_grows_until_the_image_fits_recenters_it() {
        let image = size(1000.0, 700.0);
        let small = size(500.0, 400.0);
        let scrolled = clamp_center(
            Point {
                x: 9000.0,
                y: 9000.0,
            },
            small,
            image,
            1.0,
        );
        assert_eq!(scrolled, Point { x: 750.0, y: 500.0 });
        assert_eq!(
            clamp_center(scrolled, size(1200.0, 900.0), image, 1.0),
            centered(image)
        );
    }

    #[test]
    fn a_wide_view_keeps_a_tall_image_centered_on_the_fitting_axis_only() {
        let image = size(600.0, 4000.0);
        let moved = clamp_center(Point { x: 10.0, y: 3000.0 }, VIEW, image, 1.0);
        assert!((moved.x - 300.0).abs() < 1e-9);
        assert!((moved.y - 3000.0).abs() < 1e-9);
    }

    #[test]
    fn the_point_under_the_pointer_is_kept_on_the_axes_that_overflow() {
        let image = size(4000.0, 3000.0);
        let center = Point {
            x: 2000.0,
            y: 1500.0,
        };
        let offset = Point { x: 120.0, y: -80.0 };
        let moved = clamp_center(zoom_around(center, offset, 1.0, 2.0), VIEW, image, 2.0);
        assert!((center.x + offset.x - (moved.x + offset.x / 2.0)).abs() < 1e-9);
        assert!((center.y + offset.y - (moved.y + offset.y / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn extreme_sizes_and_zooms_stay_finite_inside_the_image_and_idempotent() {
        let images = [
            size(1.0, 1.0),
            size(100_000.0, 10.0),
            size(10.0, 100_000.0),
            size(8000.0, 8000.0),
            size(799.0, 601.0),
        ];
        let views = [VIEW, size(1.0, 1.0), size(0.0, 0.0), size(5000.0, 20.0)];
        let zooms = [0.01, 0.5, 1.0, 3.0, LARGEST_ZOOM];
        let asked = [
            Point { x: -1e9, y: -1e9 },
            Point { x: 0.0, y: 0.0 },
            Point { x: 1e9, y: 1e9 },
        ];
        for image in images {
            for view in views {
                for zoom in zooms {
                    for request in asked {
                        let held = clamp_center(request, view, image, zoom);
                        assert!(held.x.is_finite() && held.y.is_finite());
                        assert_eq!(clamp_center(held, view, image, zoom), held);
                        assert!(held.x >= 0.0 && held.x <= image.width);
                        assert!(held.y >= 0.0 && held.y <= image.height);
                        let span_x = view.width / zoom;
                        if image.width > span_x {
                            assert!(held.x - span_x / 2.0 >= -1e-6);
                            assert!(held.x + span_x / 2.0 <= image.width + 1e-6);
                        } else {
                            assert!((held.x - image.width / 2.0).abs() < 1e-9);
                        }
                    }
                }
            }
        }
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
