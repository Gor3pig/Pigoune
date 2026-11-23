use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use pigoune_core::{AssetId, Dimensions, RgbaImage};

use crate::asset_colors;
use crate::load_slots::LoadSlots;
use crate::page_renderer::{self, PageProblem};

pub const THUMBNAIL_PIXELS: u32 = 256;
const REMEMBERED_THUMBNAILS: usize = 600;
const MOST_CONCURRENT_LOADS: usize = 4;
const VECTOR_MIME_TYPES: [&str; 2] = ["image/svg+xml", "image/svg+xml-compressed"];
const RGBA_CHANNELS: usize = 4;
const SMALLEST_PROBE_PIXELS: u32 = 64;

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

#[cfg(test)]
pub fn hold_every_load_slot() -> Vec<crate::load_slots::Slot> {
    let slots = LOAD_SLOTS.with(Clone::clone);
    (0..MOST_CONCURRENT_LOADS)
        .filter_map(|_| slots.try_acquire())
        .collect()
}

pub async fn thumbnail(file: &Path, thumbnail_file: &Path) -> Option<gdk::Texture> {
    thumbnail_from(async { Some(file.to_path_buf()) }, thumbnail_file).await
}

pub async fn thumbnail_of(
    original: Option<&pigoune_core::OriginalFile>,
    thumbnail_file: &Path,
) -> Option<gdk::Texture> {
    let original = original?;
    thumbnail_from(
        async {
            original.check().ok()?;
            Some(original.path().to_path_buf())
        },
        thumbnail_file,
    )
    .await
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

async fn source_loader(file: &Path) -> Option<glycin::Loader> {
    if pigoune_core::container_format_of(file).is_none() {
        return Some(loader_for(file));
    }
    let file = file.to_path_buf();
    let image = gio::spawn_blocking(move || pigoune_core::container_preview(&file))
        .await
        .ok()
        .flatten()?;
    Some(glycin::Loader::new_vec(image))
}

pub fn loader_for(file: &Path) -> glycin::Loader {
    let mut loader = glycin::Loader::new(gio::File::for_path(file));
    loader.use_expose_base_dir(false);
    loader
}

pub async fn render(file: &Path, pixels: u32) -> Option<gdk::Texture> {
    if pigoune_core::is_photoshop_file(file) {
        return photoshop_texture(file, Some(pixels)).await;
    }
    if pigoune_core::is_gimp_file(file) {
        return gimp_texture(file, Some(pixels))
            .await
            .map(|(texture, _)| texture);
    }
    if pigoune_core::page_file(file).is_some() {
        return pages_texture(file, pixels).await.ok();
    }
    let mut image = source_loader(file).await?.load().await.ok()?;
    let details = image.details();
    let (width, height) = if VECTOR_MIME_TYPES.contains(&image.mime_type().as_str()) {
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
    pub approximate: bool,
}

pub async fn load_detailed(file: &Path, vector_pixels: u32) -> Option<DetailedImage> {
    if pigoune_core::is_photoshop_file(file) {
        let texture = photoshop_texture(file, None).await?;
        return Some(DetailedImage {
            width: u32::try_from(texture.width()).ok()?,
            height: u32::try_from(texture.height()).ok()?,
            texture,
            is_vector: false,
            approximate: false,
        });
    }
    if let Some(page) = pigoune_core::page_file(file) {
        let texture = pages_texture(file, vector_pixels).await.ok()?;
        let natural = page
            .dimensions
            .map(|size| (size.width(), size.height()))
            .or_else(|| {
                Some((
                    u32::try_from(texture.width()).ok()?,
                    u32::try_from(texture.height()).ok()?,
                ))
            })?;
        return Some(DetailedImage {
            width: natural.0,
            height: natural.1,
            texture,
            is_vector: true,
            approximate: false,
        });
    }
    if pigoune_core::is_gimp_file(file) {
        let (texture, approximate) = gimp_texture(file, None).await?;
        return Some(DetailedImage {
            width: u32::try_from(texture.width()).ok()?,
            height: u32::try_from(texture.height()).ok()?,
            texture,
            is_vector: false,
            approximate,
        });
    }
    let mut image = source_loader(file).await?.load().await.ok()?;
    let details = image.details();
    let (width, height) = (details.width(), details.height());
    let is_vector = VECTOR_MIME_TYPES.contains(&image.mime_type().as_str());
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
        approximate: false,
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
        approximate: false,
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
    texture_of(image.shrunk_to(width, height))
}

fn texture_of(image: RgbaImage) -> Option<gdk::Texture> {
    let stride = usize::try_from(image.width()).ok()? * RGBA_CHANNELS;
    let (width, height) = (
        i32::try_from(image.width()).ok()?,
        i32::try_from(image.height()).ok()?,
    );
    Some(
        gdk::MemoryTexture::new(
            width,
            height,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(image.into_pixels()),
            stride,
        )
        .upcast(),
    )
}

async fn photoshop_texture(file: &Path, longest_side: Option<u32>) -> Option<gdk::Texture> {
    let file = file.to_path_buf();
    gio::spawn_blocking(move || photoshop_picture(&file, longest_side))
        .await
        .ok()
        .flatten()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewProblem {
    Photoshop(pigoune_core::PsdError),
    Gimp(pigoune_core::XcfError),
    Pages(PageProblem),
}

pub async fn preview_problem(file: &Path) -> Option<PreviewProblem> {
    let file = file.to_path_buf();
    gio::spawn_blocking(move || {
        if pigoune_core::is_photoshop_file(&file) {
            return pigoune_core::photoshop_problem(&file).map(PreviewProblem::Photoshop);
        }
        if pigoune_core::is_gimp_file(&file) {
            return pigoune_core::gimp_problem(&file).map(PreviewProblem::Gimp);
        }
        let page = pigoune_core::page_file(&file)?;
        page_renderer::render_first_page(&file, &page, SMALLEST_PROBE_PIXELS)
            .err()
            .map(PreviewProblem::Pages)
    })
    .await
    .ok()
    .flatten()
}

async fn pages_texture(file: &Path, longest_side: u32) -> Result<gdk::Texture, PageProblem> {
    let file = file.to_path_buf();
    gio::spawn_blocking(move || pages_picture(&file, longest_side))
        .await
        .map_err(|_| PageProblem::Damaged)?
}

pub fn pages_picture(file: &Path, longest_side: u32) -> Result<gdk::Texture, PageProblem> {
    let page = pigoune_core::page_file(file).ok_or(PageProblem::Damaged)?;
    let png = page_renderer::render_first_page(file, &page, longest_side)?;
    gdk::Texture::from_bytes(&glib::Bytes::from_owned(png)).map_err(|_| PageProblem::Damaged)
}

async fn gimp_texture(file: &Path, longest_side: Option<u32>) -> Option<(gdk::Texture, bool)> {
    let file = file.to_path_buf();
    gio::spawn_blocking(move || gimp_picture(&file, longest_side))
        .await
        .ok()
        .flatten()
}

pub fn gimp_picture(file: &Path, longest_side: Option<u32>) -> Option<(gdk::Texture, bool)> {
    let render = pigoune_core::render_gimp_file(file).ok()?;
    let approximate = render.approximate;
    let image = render.image;
    let image = match longest_side {
        Some(longest) if image.width().max(image.height()) > longest => {
            let (width, height) = fit_within(image.width(), image.height(), longest);
            image.shrunk_to(width, height)
        }
        _ => image,
    };
    texture_of(image).map(|texture| (texture, approximate))
}

pub fn photoshop_picture(file: &Path, longest_side: Option<u32>) -> Option<gdk::Texture> {
    let image = pigoune_core::read_merged_image_file(file).ok()?;
    let image = match longest_side {
        Some(longest) if image.width().max(image.height()) > longest => {
            let (width, height) = fit_within(image.width(), image.height(), longest);
            image.shrunk_to(width, height)
        }
        _ => image,
    };
    texture_of(image)
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
        REMEMBERED_THUMBNAILS, ThumbnailCache, fit_within, load_detailed, load_stored, render,
        scaled_to, shrink, thumbnail_of,
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

    fn photoshop_fixture() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures/rle-gradient.psd")
    }

    #[test]
    fn a_photoshop_image_is_drawn_from_its_merged_image() {
        let context = glib::MainContext::new();

        let thumbnail = context.block_on(render(&photoshop_fixture(), 8));
        let detailed = context.block_on(load_detailed(&photoshop_fixture(), 2048));

        let thumbnail = thumbnail.expect("the thumbnail is drawn");
        assert_eq!((thumbnail.width(), thumbnail.height()), (8, 4));
        let detailed = detailed.expect("the full image is drawn");
        assert_eq!((detailed.width, detailed.height), (16, 8));
        assert!(!detailed.is_vector);
    }

    fn ghostscript_is_installed() -> bool {
        std::process::Command::new("gs")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    #[test]
    fn a_pdf_page_is_drawn_sharp_at_the_requested_size() {
        if !ghostscript_is_installed() {
            return;
        }
        let context = glib::MainContext::new();
        let file = layered_fixture("pages-cairo.pdf");

        let thumbnail = context.block_on(render(&file, 100));
        let detailed = context.block_on(load_detailed(&file, 2048));

        let thumbnail = thumbnail.expect("the thumbnail is drawn");
        assert_eq!((thumbnail.width(), thumbnail.height()), (100, 71));
        let detailed = detailed.expect("the page is drawn");
        assert_eq!((detailed.width, detailed.height), (842, 595));
        assert!(detailed.is_vector);
        assert_eq!(detailed.texture.width(), 2048);
    }

    #[test]
    fn a_postscript_drawing_is_drawn_with_a_transparent_background() {
        if !ghostscript_is_installed() {
            return;
        }
        let context = glib::MainContext::new();
        let file = layered_fixture("pages-old-illustrator.ai");

        let detailed = context.block_on(load_detailed(&file, 640));

        let detailed = detailed.expect("the drawing is drawn");
        assert_eq!((detailed.width, detailed.height), (320, 200));
        assert!(detailed.is_vector);
        assert_eq!(detailed.texture.width(), 640);
    }

    #[test]
    fn a_protected_or_damaged_page_file_gives_no_picture_and_its_problem() {
        if !ghostscript_is_installed() {
            return;
        }
        let context = glib::MainContext::new();
        for (name, expected) in [
            (
                "pages-locked.pdf",
                crate::page_renderer::PageProblem::Protected,
            ),
            (
                "pages-damaged.pdf",
                crate::page_renderer::PageProblem::Damaged,
            ),
        ] {
            let file = layered_fixture(name);
            assert!(context.block_on(render(&file, 64)).is_none(), "{name}");
            assert_eq!(
                context.block_on(super::preview_problem(&file)),
                Some(super::PreviewProblem::Pages(expected)),
                "{name}"
            );
        }
    }

    #[test]
    fn a_gimp_image_is_drawn_from_its_layers() {
        let context = glib::MainContext::new();
        let file = layered_fixture("gimp-basic.xcf");

        let thumbnail = context.block_on(render(&file, 20));
        let detailed = context.block_on(load_detailed(&file, 2048));

        let thumbnail = thumbnail.expect("the thumbnail is drawn");
        assert_eq!((thumbnail.width(), thumbnail.height()), (20, 15));
        let detailed = detailed.expect("the full image is drawn");
        assert_eq!((detailed.width, detailed.height), (200, 150));
        assert!(!detailed.is_vector);
        assert!(!detailed.approximate);
    }

    #[test]
    fn a_gimp_image_using_an_unreproduced_mode_is_flagged_as_approximate() {
        let context = glib::MainContext::new();
        let file = layered_fixture("gimp-composed/mode_color_erase.xcf");

        let detailed = context.block_on(load_detailed(&file, 2048));

        assert!(detailed.expect("the image is drawn").approximate);
    }

    #[test]
    fn a_damaged_gimp_image_gives_no_picture_and_a_problem() {
        let context = glib::MainContext::new();
        let file = layered_fixture("gimp-cut.xcf");

        assert!(context.block_on(render(&file, 20)).is_none());
        assert_eq!(
            context.block_on(super::preview_problem(&file)),
            Some(super::PreviewProblem::Gimp(
                pigoune_core::XcfError::Unreadable
            ))
        );
    }

    fn layered_fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures")
            .join(name)
    }

    #[test]
    fn a_krita_or_openraster_image_is_drawn_from_its_merged_image() {
        let context = glib::MainContext::new();
        for name in ["gradient.kra", "gradient.ora"] {
            let thumbnail = context.block_on(render(&layered_fixture(name), 8));
            let detailed = context.block_on(load_detailed(&layered_fixture(name), 2048));

            let thumbnail = thumbnail.expect(name);
            assert_eq!((thumbnail.width(), thumbnail.height()), (8, 4), "{name}");
            let detailed = detailed.expect(name);
            assert_eq!((detailed.width, detailed.height), (16, 8), "{name}");
        }
    }

    #[test]
    fn files_written_by_krita_and_gimp_are_drawn() {
        let context = glib::MainContext::new();
        for name in [
            "krita-real.kra",
            "krita-real.ora",
            "gimp-real.ora",
            "gimp-real.psd",
            "gimp-real.tga",
            "gimp-real.qoi",
        ] {
            let drawn = context.block_on(render(&layered_fixture(name), 256));

            let drawn = drawn.expect(name);
            assert_eq!((drawn.width(), drawn.height()), (64, 40), "{name}");
        }
    }

    #[test]
    fn a_real_krita_file_without_a_merged_image_is_drawn_from_its_preview() {
        let drawn = glib::MainContext::new()
            .block_on(render(&layered_fixture("krita-real-no-merged.kra"), 256));

        let drawn = drawn.expect("the preview is drawn");
        assert_eq!((drawn.width(), drawn.height()), (256, 160));
    }

    #[test]
    fn a_macos_icon_is_drawn_from_its_largest_image() {
        let context = glib::MainContext::new();

        let thumbnail = context.block_on(render(&layered_fixture("modern.icns"), 128));
        let detailed = context.block_on(load_detailed(&layered_fixture("modern.icns"), 2048));

        let thumbnail = thumbnail.expect("the thumbnail is drawn");
        assert_eq!((thumbnail.width(), thumbnail.height()), (128, 128));
        let detailed = detailed.expect("the full image is drawn");
        assert_eq!((detailed.width, detailed.height), (256, 256));
        assert!(!detailed.is_vector);
    }

    #[test]
    fn an_old_macos_icon_without_a_modern_image_gives_no_picture() {
        let drawn =
            glib::MainContext::new().block_on(render(&layered_fixture("broken-legacy.icns"), 256));

        assert!(drawn.is_none());
    }

    #[test]
    fn texture_and_render_files_are_drawn() {
        let context = glib::MainContext::new();
        for name in [
            "texture-uncompressed.tga",
            "texture-rle.tga",
            "texture-gray.tga",
            "texture-dxt5.dds",
            "texture.qoi",
            "texture.exr",
        ] {
            let drawn = context.block_on(render(&layered_fixture(name), 8));

            let drawn = drawn.expect(name);
            assert_eq!((drawn.width(), drawn.height()), (8, 4), "{name}");
        }
    }

    #[test]
    fn an_old_style_macos_icon_is_decoded_and_drawn() {
        let drawn =
            glib::MainContext::new().block_on(render(&layered_fixture("legacy-rgb.icns"), 64));

        let drawn = drawn.expect("the icon is drawn");
        assert_eq!((drawn.width(), drawn.height()), (64, 64));
    }

    #[test]
    fn a_compressed_svg_is_drawn_as_a_vector() {
        let context = glib::MainContext::new();

        let thumbnail = context.block_on(render(&layered_fixture("plain.svgz"), 24));
        let detailed = context.block_on(load_detailed(&layered_fixture("plain.svgz"), 480));

        let thumbnail = thumbnail.expect("the thumbnail is drawn");
        assert_eq!((thumbnail.width(), thumbnail.height()), (24, 12));
        let detailed = detailed.expect("the full image is drawn");
        assert!(detailed.is_vector);
        assert_eq!((detailed.width, detailed.height), (48, 24));
    }

    #[test]
    fn a_krita_image_without_a_merged_image_is_drawn_from_its_preview() {
        let drawn =
            glib::MainContext::new().block_on(render(&layered_fixture("lightweight.kra"), 256));

        let drawn = drawn.expect("the preview is drawn");
        assert_eq!((drawn.width(), drawn.height()), (8, 4));
    }

    #[test]
    fn a_photoshop_image_that_cannot_be_read_gives_no_picture() {
        let zipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures/no-preview.psd");

        let drawn = glib::MainContext::new().block_on(render(&zipped, 256));

        assert!(drawn.is_none());
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

    struct Linked {
        workspace: std::path::PathBuf,
        original: pigoune_core::OriginalFile,
        thumbnail_file: std::path::PathBuf,
        outside: std::path::PathBuf,
    }

    fn library_whose_original_is_a_link(label: &str) -> Linked {
        let id = asset_id(1);
        let workspace =
            std::env::temp_dir().join(format!("pigoune-{label}-{}", std::process::id()));
        let folder = workspace.join(format!("files/01/{id}"));
        fs::create_dir_all(&folder).expect("original folder");
        let outside = workspace.join("outside.png");
        let pixels = glib::Bytes::from_owned(vec![90_u8; 3 * 4 * 4]);
        let texture = gdk::MemoryTexture::new(3, 4, gdk::MemoryFormat::R8g8b8a8, &pixels, 12);
        fs::write(&outside, texture.save_to_png_bytes()).expect("outside image");
        std::os::unix::fs::symlink(&outside, folder.join("logo.png")).expect("link");
        let original = pigoune_core::OriginalFile::new(
            &workspace,
            id,
            std::path::Path::new(&format!("files/01/{id}/logo.png")),
        )
        .expect("a conforming record");
        let thumbnail_file = workspace.join(format!("cache/thumbnails/256/01/{id}.png"));
        Linked {
            workspace,
            original,
            thumbnail_file,
            outside,
        }
    }

    #[test]
    fn a_stored_thumbnail_is_shown_without_looking_at_the_original() {
        let linked = library_whose_original_is_a_link("stored-first");
        let pixels = glib::Bytes::from_owned(vec![200_u8; 3 * 4 * 4]);
        let texture = gdk::MemoryTexture::new(3, 4, gdk::MemoryFormat::R8g8b8a8, &pixels, 12);
        pigoune_core::write_thumbnail_file(&linked.thumbnail_file, &texture.save_to_png_bytes())
            .expect("stored");

        let shown = glib::MainContext::new()
            .block_on(thumbnail_of(Some(&linked.original), &linked.thumbnail_file));

        fs::remove_dir_all(&linked.workspace).expect("workspace removed");
        let shown = shown.expect("the stored thumbnail is shown");
        assert_eq!((shown.width(), shown.height()), (3, 4));
    }

    #[test]
    fn without_a_stored_thumbnail_a_linked_original_is_never_decoded() {
        let linked = library_whose_original_is_a_link("linked-original");

        let shown = glib::MainContext::new()
            .block_on(thumbnail_of(Some(&linked.original), &linked.thumbnail_file));

        let stored = linked.thumbnail_file.exists();
        let outside_kept = linked.outside.is_file();
        fs::remove_dir_all(&linked.workspace).expect("workspace removed");
        assert!(shown.is_none());
        assert!(!stored);
        assert!(outside_kept);
    }

    #[test]
    fn a_record_without_an_original_of_its_own_shows_nothing() {
        let linked = library_whose_original_is_a_link("no-original");
        let pixels = glib::Bytes::from_owned(vec![200_u8; 3 * 4 * 4]);
        let texture = gdk::MemoryTexture::new(3, 4, gdk::MemoryFormat::R8g8b8a8, &pixels, 12);
        pigoune_core::write_thumbnail_file(&linked.thumbnail_file, &texture.save_to_png_bytes())
            .expect("stored");

        let shown = glib::MainContext::new().block_on(thumbnail_of(None, &linked.thumbnail_file));

        fs::remove_dir_all(&linked.workspace).expect("workspace removed");
        assert!(shown.is_none());
    }
}
