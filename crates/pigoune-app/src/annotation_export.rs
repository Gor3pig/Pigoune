use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use pigoune_core::annotations::AnnotationDocument;

use crate::annotation_render::{self, Backdrop};
use crate::tiled_render;

fn pixels(value: i32) -> f32 {
    f32::from(i16::try_from(value).unwrap_or(i16::MAX))
}

fn paint_image(
    snapshot: &gtk::Snapshot,
    backdrop: &Backdrop,
    document: &AnnotationDocument,
    context: &pango::Context,
) {
    let texture = backdrop.texture;
    snapshot.append_scaled_texture(
        texture,
        gsk::ScalingFilter::Nearest,
        &graphene::Rect::new(0.0, 0.0, pixels(texture.width()), pixels(texture.height())),
    );
    annotation_render::append_document(snapshot, document, context, Some(backdrop), None);
}

pub fn final_texture(
    backdrop: &Backdrop,
    document: &AnnotationDocument,
    context: &pango::Context,
) -> Option<gdk::Texture> {
    let (left, top, width, height) = match document.crop {
        Some(crop) => (
            i32::try_from(crop.x).ok()?,
            i32::try_from(crop.y).ok()?,
            i32::try_from(crop.width).ok()?,
            i32::try_from(crop.height).ok()?,
        ),
        None => (0, 0, backdrop.texture.width(), backdrop.texture.height()),
    };
    tiled_render::render(backdrop.renderer, width, height, &|snapshot| {
        snapshot.translate(&graphene::Point::new(-pixels(left), -pixels(top)));
        paint_image(snapshot, backdrop, document, context);
    })
}
