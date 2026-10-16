use gtk::prelude::*;
use gtk::{gdk, glib, graphene, gsk, pango};
use pigoune_core::annotations::AnnotationDocument;

use crate::annotation_render::{self, Backdrop, Scene};

const TILE_SIDE: i32 = 2048;
const FORMAT: gdk::MemoryFormat = gdk::MemoryFormat::R8g8b8a8Premultiplied;
const BYTES_PER_PIXEL: usize = 4;

struct Source<'a> {
    backdrop: &'a Backdrop<'a>,
    document: &'a AnnotationDocument,
    context: &'a pango::Context,
}

fn tile_origins(length: i32) -> impl Iterator<Item = i32> {
    (0..length).step_by(usize::try_from(TILE_SIDE).unwrap_or(1))
}

fn paint_masks(snapshot: &gtk::Snapshot, source: &Source) {
    let texture = source.backdrop.texture;
    let whole = graphene::Rect::new(0.0, 0.0, to_f32(texture.width()), to_f32(texture.height()));
    snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Nearest, &whole);
    let base_only = Scene {
        backdrop: source.backdrop,
        document: source.document,
        upto: 0,
    };
    for (_, mask) in source.document.masks() {
        annotation_render::append_shape(snapshot, &mask, 1, source.context, Some(&base_only));
    }
}

fn to_f32(value: i32) -> f32 {
    f32::from(i16::try_from(value).unwrap_or(i16::MAX))
}

fn render_tile(
    source: &Source,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
) -> Option<gdk::Texture> {
    let snapshot = gtk::Snapshot::new();
    snapshot.translate(&graphene::Point::new(-to_f32(left), -to_f32(top)));
    paint_masks(&snapshot, source);
    let node = snapshot.to_node()?;
    let viewport = graphene::Rect::new(0.0, 0.0, to_f32(width), to_f32(height));
    Some(
        source
            .backdrop
            .renderer
            .render_texture(&node, Some(&viewport)),
    )
}

fn copy_tile(tile: &gdk::Texture, pixels: &mut [u8], row_bytes: usize, left: i32, top: i32) {
    let mut downloader = gdk::TextureDownloader::new(tile);
    downloader.set_format(FORMAT);
    let (bytes, stride) = downloader.download_bytes();
    let columns = usize::try_from(tile.width()).unwrap_or(0) * BYTES_PER_PIXEL;
    let rows = usize::try_from(tile.height()).unwrap_or(0);
    let left = usize::try_from(left).unwrap_or(0) * BYTES_PER_PIXEL;
    let top = usize::try_from(top).unwrap_or(0);
    for row in 0..rows {
        let from = row * stride;
        let to = (top + row) * row_bytes + left;
        pixels[to..to + columns].copy_from_slice(&bytes[from..from + columns]);
    }
}

pub fn bake(
    backdrop: &Backdrop,
    document: &AnnotationDocument,
    context: &pango::Context,
) -> Option<gdk::Texture> {
    let source = Source {
        backdrop,
        document,
        context,
    };
    let (width, height) = (backdrop.texture.width(), backdrop.texture.height());
    let row_bytes = usize::try_from(width).ok()? * BYTES_PER_PIXEL;
    let mut pixels = vec![0_u8; row_bytes * usize::try_from(height).ok()?];
    for top in tile_origins(height) {
        for left in tile_origins(width) {
            let tile = render_tile(
                &source,
                left,
                top,
                TILE_SIDE.min(width - left),
                TILE_SIDE.min(height - top),
            )?;
            copy_tile(&tile, &mut pixels, row_bytes, left, top);
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
