use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io;
use std::path::Path;

use gtk::prelude::*;
use gtk::{gdk, gio, graphene, gsk};
use pigoune_core::AssetId;

pub const THUMBNAIL_PIXELS: u32 = 256;
const REMEMBERED_THUMBNAILS: usize = 600;
const SVG_MIME_TYPE: &str = "image/svg+xml";

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

pub async fn thumbnail(
    file: &Path,
    thumbnail_file: &Path,
    widget: &impl IsA<gtk::Widget>,
) -> Option<gdk::Texture> {
    if let Some(stored) = load_stored(thumbnail_file).await {
        return Some(stored);
    }
    let texture = render(file, THUMBNAIL_PIXELS, widget).await?;
    store(&texture, thumbnail_file);
    Some(texture)
}

async fn load_stored(thumbnail_file: &Path) -> Option<gdk::Texture> {
    if !thumbnail_file.is_file() {
        return None;
    }
    let mut image = loader_for(thumbnail_file).load().await.ok()?;
    let texture = image.next_frame().await.ok()?.texture();
    fits_within(&texture, THUMBNAIL_PIXELS).then_some(texture)
}

fn store(texture: &gdk::Texture, thumbnail_file: &Path) {
    let png = texture.save_to_png_bytes();
    let thumbnail_file = thumbnail_file.to_path_buf();
    drop(gio::spawn_blocking(move || {
        let _ = write_atomically(&thumbnail_file, &png);
    }));
}

fn write_atomically(destination: &Path, content: &[u8]) -> io::Result<()> {
    let folder = destination
        .parent()
        .ok_or_else(|| io::Error::other("a thumbnail file always has a folder"))?;
    fs::create_dir_all(folder)?;
    let file_name = destination
        .file_name()
        .ok_or_else(|| io::Error::other("a thumbnail file always has a name"))?;
    let partial = folder.join(format!(".{}.partial", file_name.to_string_lossy()));
    fs::write(&partial, content)?;
    fs::rename(&partial, destination).inspect_err(|_| {
        let _ = fs::remove_file(&partial);
    })
}

fn loader_for(file: &Path) -> glycin::Loader {
    let mut loader = glycin::Loader::new(gio::File::for_path(file));
    loader.use_expose_base_dir(false);
    loader
}

pub async fn render(
    file: &Path,
    pixels: u32,
    widget: &impl IsA<gtk::Widget>,
) -> Option<gdk::Texture> {
    let mut image = loader_for(file).load().await.ok()?;
    let details = image.details();
    let (width, height) = if image.mime_type().as_str() == SVG_MIME_TYPE {
        scaled_to(details.width(), details.height(), pixels)
    } else {
        fit_within(details.width(), details.height(), pixels)
    };
    let frame = image
        .specific_frame(glycin::FrameRequest::new().scale(width, height))
        .await
        .ok()?;
    let texture = frame.texture();
    if fits_within(&texture, pixels) {
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

fn fit_within(width: u32, height: u32, pixels: u32) -> (u32, u32) {
    if width.max(height) <= pixels {
        (width.max(1), height.max(1))
    } else {
        scaled_to(width, height, pixels)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the result is rounded and bounded by the requested side"
)]
fn scaled_to(width: u32, height: u32, pixels: u32) -> (u32, u32) {
    let longest = width.max(height).max(1);
    let ratio = f64::from(pixels) / f64::from(longest);
    let scaled = |side: u32| ((f64::from(side) * ratio).round() as u32).max(1);
    (scaled(width), scaled(height))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use gtk::prelude::*;
    use gtk::{gdk, glib};

    use super::{fit_within, load_stored, scaled_to, write_atomically};

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
    fn drawings_are_scaled_up_to_the_full_size() {
        assert_eq!(scaled_to(100, 100, 256), (256, 256));
        assert_eq!(scaled_to(98, 49, 256), (256, 128));
    }

    #[test]
    fn extreme_proportions_keep_at_least_one_pixel() {
        assert_eq!(fit_within(10_000, 1, 256), (256, 1));
        assert_eq!(fit_within(0, 0, 256), (1, 1));
    }

    #[test]
    fn a_thumbnail_is_written_whole_with_its_folders() {
        let workspace = std::env::temp_dir().join(format!("pigoune-cache-{}", std::process::id()));
        let destination = workspace.join("thumbnails/256/asset.png");

        write_atomically(&destination, b"content").expect("thumbnail written");

        assert_eq!(fs::read(&destination).expect("readable"), b"content");
        let leftovers = fs::read_dir(destination.parent().expect("folder"))
            .expect("folder readable")
            .count();
        fs::remove_dir_all(&workspace).expect("workspace removed");
        assert_eq!(leftovers, 1);
    }

    #[test]
    fn a_stored_thumbnail_is_read_back_and_a_damaged_one_is_ignored() {
        let workspace = std::env::temp_dir().join(format!("pigoune-stored-{}", std::process::id()));
        let stored = workspace.join("stored.png");
        let damaged = workspace.join("damaged.png");
        let missing = workspace.join("missing.png");
        let pixels = glib::Bytes::from_owned(vec![200_u8; 3 * 4 * 4]);
        let texture = gdk::MemoryTexture::new(3, 4, gdk::MemoryFormat::R8g8b8a8, &pixels, 12);
        write_atomically(&stored, &texture.save_to_png_bytes()).expect("stored");
        write_atomically(&damaged, b"\x89PNG\r\n\x1a\nbroken").expect("damaged written");

        let context = glib::MainContext::new();
        let read_back = context.block_on(load_stored(&stored));
        let from_damaged = context.block_on(load_stored(&damaged));
        let from_missing = context.block_on(load_stored(&missing));

        fs::remove_dir_all(&workspace).expect("workspace removed");
        let read_back = read_back.expect("stored thumbnail is read back");
        assert_eq!((read_back.width(), read_back.height()), (3, 4));
        assert!(from_damaged.is_none());
        assert!(from_missing.is_none());
    }
}
