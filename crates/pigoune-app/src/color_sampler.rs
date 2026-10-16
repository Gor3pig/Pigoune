use gtk::prelude::*;
use gtk::{gdk, graphene, gsk};
use pigoune_core::annotations::Color;

const OPAQUE: u8 = 255;

pub fn pixel_color(
    renderer: &gsk::Renderer,
    texture: &gdk::Texture,
    x: f32,
    y: f32,
) -> Option<Color> {
    let (width, height) = (texture.width(), texture.height());
    let (column, row) = (floor_to_pixel(x, width)?, floor_to_pixel(y, height)?);
    let snapshot = gtk::Snapshot::new();
    snapshot.translate(&graphene::Point::new(-whole(column), -whole(row)));
    snapshot.append_scaled_texture(
        texture,
        gsk::ScalingFilter::Nearest,
        &graphene::Rect::new(0.0, 0.0, whole(width), whole(height)),
    );
    let node = snapshot.to_node()?;
    let sample = renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, 1.0, 1.0)));
    let mut downloader = gdk::TextureDownloader::new(&sample);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, _) = downloader.download_bytes();
    match bytes.as_ref() {
        [red, green, blue, ..] => Some(Color {
            r: *red,
            g: *green,
            b: *blue,
            a: OPAQUE,
        }),
        _ => None,
    }
}

fn whole(value: i32) -> f32 {
    f32::from(i16::try_from(value).unwrap_or(i16::MAX))
}

fn floor_to_pixel(coordinate: f32, size: i32) -> Option<i32> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the coordinate is checked against the size of the image first"
    )]
    let pixel = coordinate.floor() as i32;
    (coordinate >= 0.0 && pixel < size).then_some(pixel)
}

#[cfg(test)]
mod tests {
    use super::floor_to_pixel;

    #[test]
    fn a_coordinate_inside_the_image_gives_its_pixel() {
        assert_eq!(floor_to_pixel(0.0, 10), Some(0));
        assert_eq!(floor_to_pixel(3.9, 10), Some(3));
        assert_eq!(floor_to_pixel(9.99, 10), Some(9));
    }

    #[test]
    fn a_coordinate_outside_the_image_gives_nothing() {
        assert_eq!(floor_to_pixel(-0.1, 10), None);
        assert_eq!(floor_to_pixel(10.0, 10), None);
        assert_eq!(floor_to_pixel(250.0, 10), None);
    }
}
