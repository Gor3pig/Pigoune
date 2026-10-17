use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use pigoune_core::annotations::{AnnotationDocument, AnnotationObject};

use crate::annotation_render::{self, Backdrop};
use crate::tiled_render;

fn pixels(value: i32) -> f32 {
    f32::from(i16::try_from(value).unwrap_or(i16::MAX))
}

fn masks_only(document: &AnnotationDocument, ids: &[u32]) -> AnnotationDocument {
    let objects: Vec<AnnotationObject> = document
        .objects
        .iter()
        .filter(|object| ids.contains(&object.id))
        .cloned()
        .collect();
    AnnotationDocument::new(None, objects)
}

fn paint_masks(
    snapshot: &gtk::Snapshot,
    backdrop: &Backdrop,
    masks: &AnnotationDocument,
    context: &pango::Context,
) {
    snapshot.append_scaled_texture(
        backdrop.texture,
        gsk::ScalingFilter::Nearest,
        &graphene::Rect::new(
            0.0,
            0.0,
            pixels(backdrop.texture.width()),
            pixels(backdrop.texture.height()),
        ),
    );
    annotation_render::append_document(snapshot, masks, context, Some(backdrop), None);
}

pub fn bake(
    backdrop: &Backdrop,
    document: &AnnotationDocument,
    ids: &[u32],
    context: &pango::Context,
) -> Option<gdk::Texture> {
    let masks = masks_only(document, ids);
    tiled_render::render(
        backdrop.renderer,
        backdrop.texture.width(),
        backdrop.texture.height(),
        &|snapshot| paint_masks(snapshot, backdrop, &masks, context),
    )
}
