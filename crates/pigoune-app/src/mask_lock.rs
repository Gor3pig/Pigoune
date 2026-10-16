use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use pigoune_core::annotations::AnnotationDocument;

use crate::annotation_render::{self, Backdrop, Scene};
use crate::tiled_render;

fn whole_texture(texture: &gdk::Texture) -> graphene::Rect {
    graphene::Rect::new(0.0, 0.0, pixels(texture.width()), pixels(texture.height()))
}

fn pixels(value: i32) -> f32 {
    f32::from(i16::try_from(value).unwrap_or(i16::MAX))
}

fn paint_masks(
    snapshot: &gtk::Snapshot,
    backdrop: &Backdrop,
    document: &AnnotationDocument,
    context: &pango::Context,
) {
    snapshot.append_scaled_texture(
        backdrop.texture,
        gsk::ScalingFilter::Nearest,
        &whole_texture(backdrop.texture),
    );
    let base_only = Scene {
        backdrop,
        document,
        upto: 0,
    };
    for (_, mask) in document.masks() {
        annotation_render::append_shape(snapshot, &mask, 1, context, Some(&base_only));
    }
}

pub fn bake(
    backdrop: &Backdrop,
    document: &AnnotationDocument,
    context: &pango::Context,
) -> Option<gdk::Texture> {
    tiled_render::render(
        backdrop.renderer,
        backdrop.texture.width(),
        backdrop.texture.height(),
        &|snapshot| paint_masks(snapshot, backdrop, document, context),
    )
}
