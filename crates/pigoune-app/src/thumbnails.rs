use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::Path;

use gtk::prelude::*;
use gtk::{gdk, gio, graphene, gsk};
use pigoune_core::AssetId;

pub const THUMBNAIL_PIXELS: u32 = 256;
const REMEMBERED_THUMBNAILS: usize = 600;

#[derive(Default)]
pub struct ThumbnailCache {
    textures: RefCell<HashMap<AssetId, gdk::Texture>>,
    arrival_order: RefCell<VecDeque<AssetId>>,
}

impl ThumbnailCache {
    pub fn remembered(&self, id: AssetId) -> Option<gdk::Texture> {
        self.textures.borrow().get(&id).cloned()
    }

    pub fn remember(&self, id: AssetId, texture: gdk::Texture) {
        let mut textures = self.textures.borrow_mut();
        let mut arrival_order = self.arrival_order.borrow_mut();
        if textures.insert(id, texture).is_none() {
            arrival_order.push_back(id);
        }
        while arrival_order.len() > REMEMBERED_THUMBNAILS {
            if let Some(oldest) = arrival_order.pop_front() {
                textures.remove(&oldest);
            }
        }
    }

    pub fn forget_all(&self) {
        self.textures.borrow_mut().clear();
        self.arrival_order.borrow_mut().clear();
    }
}

pub async fn render(file: &Path, widget: &impl IsA<gtk::Widget>) -> Option<gdk::Texture> {
    let mut loader = glycin::Loader::new(gio::File::for_path(file));
    loader.use_expose_base_dir(false);
    let mut image = loader.load().await.ok()?;
    let details = image.details();
    let (width, height) = fit_within(details.width(), details.height(), THUMBNAIL_PIXELS);
    let frame = image
        .specific_frame(glycin::FrameRequest::new().scale(width, height))
        .await
        .ok()?;
    let texture = frame.texture();
    if fits_within(&texture, THUMBNAIL_PIXELS) {
        return Some(texture);
    }
    Some(downscale(&texture, width, height, widget).unwrap_or(texture))
}

fn fits_within(texture: &gdk::Texture, pixels: u32) -> bool {
    u32::try_from(texture.width()).is_ok_and(|width| width <= pixels)
        && u32::try_from(texture.height()).is_ok_and(|height| height <= pixels)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "thumbnail sides are at most a few hundred pixels"
)]
fn downscale(
    texture: &gdk::Texture,
    width: u32,
    height: u32,
    widget: &impl IsA<gtk::Widget>,
) -> Option<gdk::Texture> {
    let renderer = widget.native()?.renderer()?;
    let bounds = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let snapshot = gtk::Snapshot::new();
    snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Trilinear, &bounds);
    let node = snapshot.to_node()?;
    Some(renderer.render_texture(node, Some(&bounds)))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the result is rounded and bounded by the requested side"
)]
fn fit_within(width: u32, height: u32, pixels: u32) -> (u32, u32) {
    let longest = width.max(height).max(1);
    if longest <= pixels {
        return (width.max(1), height.max(1));
    }
    let ratio = f64::from(pixels) / f64::from(longest);
    let scaled = |side: u32| ((f64::from(side) * ratio).round() as u32).max(1);
    (scaled(width), scaled(height))
}

#[cfg(test)]
mod tests {
    use super::fit_within;

    #[test]
    fn small_images_keep_their_size() {
        assert_eq!(fit_within(48, 32, 256), (48, 32));
    }

    #[test]
    fn large_images_shrink_keeping_their_proportions() {
        assert_eq!(fit_within(4000, 3000, 256), (256, 192));
        assert_eq!(fit_within(300, 1200, 256), (64, 256));
    }

    #[test]
    fn extreme_proportions_keep_at_least_one_pixel() {
        assert_eq!(fit_within(10_000, 1, 256), (256, 1));
        assert_eq!(fit_within(0, 0, 256), (1, 1));
    }
}
