use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use pigoune_core::{AssetId, Dimensions, RgbaImage};

use crate::asset_colors;
use crate::load_slots::LoadSlots;

pub const THUMBNAIL_PIXELS: u32 = 256;
const REMEMBERED_THUMBNAILS: usize = 600;
const MOST_CONCURRENT_LOADS: usize = 4;
const SVG_MIME_TYPE: &str = "image/svg+xml";
const RGBA_CHANNELS: usize = 4;

thread_local! {
    static LOAD_SLOTS: LoadSlots = LoadSlots::new(MOST_CONCURRENT_LOADS);
}

#[derive(Default)]
pub struct ThumbnailCache {
    textures: RefCell<HashMap<AssetId, gdk::Texture>>,
    use_order: RefCell<VecDeque<AssetId>>,
}

impl ThumbnailCache {
    pub fn remembered(&self, id: AssetId) -> Option<gdk::Texture> {
        let texture = self.textures.borrow().get(&id).cloned()?;
        touch(&mut self.use_order.borrow_mut(), id);
        Some(texture)
    }

    pub fn remember(&self, id: AssetId, texture: gdk::Texture) {
        let mut textures = self.textures.borrow_mut();
        let mut use_order = self.use_order.borrow_mut();
        textures.insert(id, texture);
        touch(&mut use_order, id);
        while use_order.len() > REMEMBERED_THUMBNAILS {
            if let Some(least_used) = use_order.pop_front() {
                textures.remove(&least_used);
            }
        }
    }

    pub fn forget_all(&self) {
        self.textures.borrow_mut().clear();
        self.use_order.borrow_mut().clear();
    }
}

fn touch(use_order: &mut VecDeque<AssetId>, id: AssetId) {
    if let Some(position) = use_order.iter().position(|known| *known == id) {
        use_order.remove(position);
    }
    use_order.push_back(id);
}

pub async fn thumbnail(file: &Path, thumbnail_file: &Path) -> Option<gdk::Texture> {
    thumbnail_from(async { Some(file.to_path_buf()) }, thumbnail_file).await
}

pub async fn thumbnail_of(
    original: Option<&pigoune_core::OriginalFile>,
    thumbnail_file: &Path,
) -> Option<gdk::Texture> {
    let original = original?;
    original.check().ok()?;
    thumbnail(original.path(), thumbnail_file).await
}

pub async fn thumbnail_from(
    source: impl std::future::Future<Output = Option<PathBuf>>,
    thumbnail_file: &Path,
) -> Option<gdk::Texture> {
    let slots = LOAD_SLOTS.with(Clone::clone);
    let _slot = slots.acquire().await;
    if let Some(stored) = load_stored(thumbnail_file).await {
        return Some(stored);
    }
    let file = source.await?;
    let texture = render(&file, THUMBNAIL_PIXELS).await?;
    store(&texture, thumbnail_file);
    Some(texture)
}

pub async fn thumbnail_drawn(
    drawn: impl std::future::Future<Output = Option<gdk::Texture>>,
    thumbnail_file: &Path,
) -> Option<gdk::Texture> {
    let slots = LOAD_SLOTS.with(Clone::clone);
    let _slot = slots.acquire().await;
    if let Some(stored) = load_stored(thumbnail_file).await {
        return Some(stored);
    }
    let texture = drawn.await?;
    store(&texture, thumbnail_file);
    Some(texture)
}

pub fn side_of_thumbnails() -> i32 {
    i32::try_from(THUMBNAIL_PIXELS).unwrap_or(i32::MAX)
}

async fn load_stored(thumbnail_file: &Path) -> Option<gdk::Texture> {
    if pigoune_core::cache_file(thumbnail_file) != pigoune_core::CacheFile::Plain {
        return None;
    }
    let mut image = loader_for(thumbnail_file).load().await.ok()?;
    let texture = image.next_frame().await.ok()?.texture();
    fits_within(&texture, THUMBNAIL_PIXELS).then_some(texture)
}

fn store(texture: &gdk::Texture, thumbnail_file: &Path) {
    if pigoune_core::cache_file(thumbnail_file) == pigoune_core::CacheFile::Unsafe {
        return;
    }
    let texture = texture.clone();
    let thumbnail_file = thumbnail_file.to_path_buf();
    drop(gio::spawn_blocking(move || {
        let png = texture.save_to_png_bytes();
        let _ = pigoune_core::write_thumbnail_file(&thumbnail_file, &png);
    }));
}

pub fn loader_for(file: &Path) -> glycin::Loader {
    let mut loader = glycin::Loader::new(gio::File::for_path(file));
    loader.use_expose_base_dir(false);
    loader
}

pub async fn render(file: &Path, pixels: u32) -> Option<gdk::Texture> {
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
    Some(downscaled(texture, width, height).await)
}

pub struct DetailedImage {
    pub texture: gdk::Texture,
    pub width: u32,
    pub height: u32,
    pub is_vector: bool,
}

pub async fn load_detailed(file: &Path, vector_pixels: u32) -> Option<DetailedImage> {
    let mut image = loader_for(file).load().await.ok()?;
    let details = image.details();
    let (width, height) = (details.width(), details.height());
    let is_vector = image.mime_type().as_str() == SVG_MIME_TYPE;
    let request = if is_vector {
        let (scaled_width, scaled_height) = scaled_to(width, height, vector_pixels);
        glycin::FrameRequest::new().scale(scaled_width, scaled_height)
    } else {
        glycin::FrameRequest::new()
    };
    let frame = image.specific_frame(request).await.ok()?;
    Some(DetailedImage {
        texture: frame.texture(),
        width,
        height,
        is_vector,
    })
}

pub async fn load_icon_size(file: &Path, size: Dimensions) -> Option<DetailedImage> {
    let (bytes, _) = gio::File::for_path(file)
        .load_contents_future()
        .await
        .ok()?;
    let icon = pigoune_core::single_size_icon(&bytes, size)?;
    let mut image = glycin::Loader::new_vec(icon).load().await.ok()?;
    let frame = image.next_frame().await.ok()?;
    Some(DetailedImage {
        texture: frame.texture(),
        width: size.width(),
        height: size.height(),
        is_vector: false,
    })
}

fn fits_within(texture: &gdk::Texture, pixels: u32) -> bool {
    u32::try_from(texture.width()).is_ok_and(|width| width <= pixels)
        && u32::try_from(texture.height()).is_ok_and(|height| height <= pixels)
}

async fn downscaled(texture: gdk::Texture, width: u32, height: u32) -> gdk::Texture {
    let source = texture.clone();
    gio::spawn_blocking(move || shrink(&source, width, height))
        .await
        .ok()
        .flatten()
        .unwrap_or(texture)
}

fn shrink(texture: &gdk::Texture, width: u32, height: u32) -> Option<gdk::Texture> {
    let image = RgbaImage::new(
        u32::try_from(texture.width()).ok()?,
        u32::try_from(texture.height()).ok()?,
        asset_colors::rgba_pixels(texture),
    )?;
    let shrunk = image.shrunk_to(width, height);
    let stride = usize::try_from(shrunk.width()).ok()? * RGBA_CHANNELS;
    let (width, height) = (
        i32::try_from(shrunk.width()).ok()?,
        i32::try_from(shrunk.height()).ok()?,
    );
    Some(
        gdk::MemoryTexture::new(
            width,
            height,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(shrunk.into_pixels()),
            stride,
        )
        .upcast(),
    )
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

    use pigoune_core::AssetId;

    use super::{
        REMEMBERED_THUMBNAILS, ThumbnailCache, fit_within, load_stored, scaled_to, shrink,
    };
    use crate::asset_colors;

    fn asset_id(number: usize) -> AssetId {
        AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
    }

    fn one_pixel() -> gdk::Texture {
        gdk::MemoryTexture::new(
            1,
            1,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(vec![0_u8; 4]),
            4,
        )
        .upcast()
    }

    #[test]
    fn the_least_recently_used_thumbnail_is_forgotten_first() {
        let cache = ThumbnailCache::default();
        for number in 1..=REMEMBERED_THUMBNAILS {
            cache.remember(asset_id(number), one_pixel());
        }
        assert!(cache.remembered(asset_id(1)).is_some());

        cache.remember(asset_id(REMEMBERED_THUMBNAILS + 1), one_pixel());

        assert!(cache.remembered(asset_id(1)).is_some());
        assert!(cache.remembered(asset_id(2)).is_none());
        assert!(
            cache
                .remembered(asset_id(REMEMBERED_THUMBNAILS + 1))
                .is_some()
        );
    }

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
        fs::create_dir_all(&workspace).expect("workspace");
        let destination = workspace.join("cache/thumbnails/256/asset.png");

        pigoune_core::write_thumbnail_file(&destination, b"content").expect("thumbnail written");

        assert_eq!(fs::read(&destination).expect("readable"), b"content");
        let leftovers = fs::read_dir(destination.parent().expect("folder"))
            .expect("folder readable")
            .count();
        fs::remove_dir_all(&workspace).expect("workspace removed");
        assert_eq!(leftovers, 1);
    }

    #[test]
    fn a_large_texture_is_shrunk_to_the_asked_size_keeping_its_colors() {
        let pixels = glib::Bytes::from_owned([20_u8, 80, 200, 255].repeat(600 * 400));
        let texture: gdk::Texture =
            gdk::MemoryTexture::new(600, 400, gdk::MemoryFormat::R8g8b8a8, &pixels, 600 * 4)
                .upcast();

        let shrunk = shrink(&texture, 256, 171).expect("texture shrunk");

        assert_eq!((shrunk.width(), shrunk.height()), (256, 171));
        let colors = asset_colors::rgba_pixels(&shrunk);
        assert!(colors.chunks(4).all(|pixel| pixel == [20, 80, 200, 255]));
    }

    #[test]
    fn a_stored_thumbnail_is_read_back_and_a_damaged_one_is_ignored() {
        let workspace = std::env::temp_dir().join(format!("pigoune-stored-{}", std::process::id()));
        fs::create_dir_all(&workspace).expect("workspace");
        let stored = workspace.join("cache/stored.png");
        let damaged = workspace.join("cache/damaged.png");
        let missing = workspace.join("cache/missing.png");
        let pixels = glib::Bytes::from_owned(vec![200_u8; 3 * 4 * 4]);
        let texture = gdk::MemoryTexture::new(3, 4, gdk::MemoryFormat::R8g8b8a8, &pixels, 12);
        pigoune_core::write_thumbnail_file(&stored, &texture.save_to_png_bytes()).expect("stored");
        pigoune_core::write_thumbnail_file(&damaged, b"\x89PNG\r\n\x1a\nbroken")
            .expect("damaged written");

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

    #[test]
    fn a_thumbnail_that_is_a_link_is_never_loaded() {
        let workspace = std::env::temp_dir().join(format!("pigoune-linked-{}", std::process::id()));
        fs::create_dir_all(workspace.join("cache")).expect("workspace");
        let outside = workspace.join("outside.png");
        let pixels = glib::Bytes::from_owned(vec![200_u8; 3 * 4 * 4]);
        let texture = gdk::MemoryTexture::new(3, 4, gdk::MemoryFormat::R8g8b8a8, &pixels, 12);
        fs::write(&outside, texture.save_to_png_bytes()).expect("outside image");
        let linked = workspace.join("cache/linked.png");
        std::os::unix::fs::symlink(&outside, &linked).expect("link");
        let broken = workspace.join("cache/broken.png");
        std::os::unix::fs::symlink(workspace.join("nothing.png"), &broken).expect("link");

        let context = glib::MainContext::new();
        let from_link = context.block_on(load_stored(&linked));
        let from_broken = context.block_on(load_stored(&broken));
        let still_a_link = fs::symlink_metadata(&linked).expect("link").is_symlink();
        let outside_kept =
            fs::read(&outside).expect("outside") == texture.save_to_png_bytes().to_vec();

        fs::remove_dir_all(&workspace).expect("workspace removed");
        assert!(from_link.is_none());
        assert!(from_broken.is_none());
        assert!(still_a_link);
        assert!(outside_kept);
    }
}
