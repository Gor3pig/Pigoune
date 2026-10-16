use gtk::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

const TILE_SIDE: i32 = 2048;
const FORMAT: gdk::MemoryFormat = gdk::MemoryFormat::R8g8b8a8Premultiplied;
const BYTES_PER_PIXEL: usize = 4;

fn tile_origins(length: i32) -> impl Iterator<Item = i32> {
    (0..length).step_by(usize::try_from(TILE_SIDE).unwrap_or(1))
}

fn to_f32(value: i32) -> f32 {
    f32::from(i16::try_from(value).unwrap_or(i16::MAX))
}

fn render_tile(
    renderer: &gsk::Renderer,
    paint: &dyn Fn(&gtk::Snapshot),
    origin: (i32, i32),
    size: (i32, i32),
) -> Option<gdk::Texture> {
    let snapshot = gtk::Snapshot::new();
    snapshot.translate(&graphene::Point::new(-to_f32(origin.0), -to_f32(origin.1)));
    paint(&snapshot);
    let node = snapshot.to_node()?;
    let viewport = graphene::Rect::new(0.0, 0.0, to_f32(size.0), to_f32(size.1));
    Some(renderer.render_texture(&node, Some(&viewport)))
}

fn copy_tile(tile: &gdk::Texture, pixels: &mut [u8], row_bytes: usize, origin: (i32, i32)) {
    let mut downloader = gdk::TextureDownloader::new(tile);
    downloader.set_format(FORMAT);
    let (bytes, stride) = downloader.download_bytes();
    let columns = usize::try_from(tile.width()).unwrap_or(0) * BYTES_PER_PIXEL;
    let rows = usize::try_from(tile.height()).unwrap_or(0);
    let left = usize::try_from(origin.0).unwrap_or(0) * BYTES_PER_PIXEL;
    let top = usize::try_from(origin.1).unwrap_or(0);
    for row in 0..rows {
        let from = row * stride;
        let to = (top + row) * row_bytes + left;
        pixels[to..to + columns].copy_from_slice(&bytes[from..from + columns]);
    }
}

pub fn render(
    renderer: &gsk::Renderer,
    width: i32,
    height: i32,
    paint: &dyn Fn(&gtk::Snapshot),
) -> Option<gdk::Texture> {
    let row_bytes = usize::try_from(width).ok()? * BYTES_PER_PIXEL;
    let mut pixels = vec![0_u8; row_bytes * usize::try_from(height).ok()?];
    for top in tile_origins(height) {
        for left in tile_origins(width) {
            let size = (TILE_SIDE.min(width - left), TILE_SIDE.min(height - top));
            let tile = render_tile(renderer, paint, (left, top), size)?;
            copy_tile(&tile, &mut pixels, row_bytes, (left, top));
        }
    }
    Some(
        gdk::MemoryTexture::new(
            width,
            height,
            FORMAT,
            &glib::Bytes::from_owned(pixels),
            row_bytes,
        )
        .upcast(),
    )
}
