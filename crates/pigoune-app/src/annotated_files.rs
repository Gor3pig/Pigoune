use std::path::{Path, PathBuf};

use gtk::gdk;
use gtk::prelude::*;
use pigoune_core::AnnotationRender;
use pigoune_core::annotations::AnnotationDocument;

use crate::annotation_export;
use crate::annotation_render::{Backdrop, PixelCache};
use crate::image_conversion;
use crate::thumbnails;

pub const VECTOR_PIXELS: u32 = 2048;

pub async fn load_source(file: &Path) -> Option<gdk::Texture> {
    thumbnails::load_detailed(file, VECTOR_PIXELS)
        .await
        .map(|image| image.texture)
}

pub async fn ensure(
    render: &AnnotationRender,
    source: &Path,
    widget: &impl IsA<gtk::Widget>,
) -> Option<PathBuf> {
    match pigoune_core::cache_file(&render.file) {
        pigoune_core::CacheFile::Plain => return Some(render.file.clone()),
        pigoune_core::CacheFile::Unsafe => return None,
        pigoune_core::CacheFile::Missing => {}
    }
    let document = AnnotationDocument::from_json(&render.document).ok()?;
    let texture = load_source(source).await?;
    let flat = render_flat_yielding(&texture, &document, widget).await?;
    let png = image_conversion::png_bytes(&flat).await.ok()?;
    pigoune_core::write_render_file(&render.file, &png).ok()?;
    Some(render.file.clone())
}

pub fn cropped_size(document: &str) -> Option<(u32, u32)> {
    let crop = AnnotationDocument::from_json(document).ok()?.crop?;
    Some((crop.width, crop.height))
}

pub fn png_size(file: &Path) -> Option<(u32, u32)> {
    let mut header = [0_u8; 24];
    std::io::Read::read_exact(&mut pigoune_core::open_plain_file(file).ok()?, &mut header).ok()?;
    let width = u32::from_be_bytes(header[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(header[20..24].try_into().ok()?);
    (&header[1..4] == b"PNG" && width > 0 && height > 0).then_some((width, height))
}

pub async fn render_flat_yielding(
    texture: &gdk::Texture,
    document: &AnnotationDocument,
    widget: &impl IsA<gtk::Widget>,
) -> Option<gdk::Texture> {
    let renderer = widget.native()?.renderer()?;
    let cache = PixelCache::default();
    let backdrop = Backdrop {
        texture,
        renderer: &renderer,
        cache: &cache,
    };
    annotation_export::final_texture_yielding(&backdrop, document, &widget.pango_context()).await
}

pub async fn reduced(
    render: &AnnotationRender,
    source: &Path,
    widget: &impl IsA<gtk::Widget>,
    longest_side: i32,
) -> Option<gdk::Texture> {
    let document = AnnotationDocument::from_json(&render.document).ok()?;
    let texture = load_source(source).await?;
    let renderer = widget.native()?.renderer()?;
    let cache = PixelCache::default();
    let backdrop = Backdrop {
        texture: &texture,
        renderer: &renderer,
        cache: &cache,
    };
    annotation_export::reduced_texture(&backdrop, &document, &widget.pango_context(), longest_side)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn a_rendering_is_written_whole_with_its_folders_and_leaves_no_unfinished_file() {
        let folder =
            std::env::temp_dir().join(format!("pigoune-render-test-{}", std::process::id()));
        fs::create_dir_all(&folder).expect("workspace");
        let file = folder.join("cache/nested/one.png");
        pigoune_core::write_render_file(&file, b"png").expect("written");
        assert_eq!(fs::read(&file).expect("read"), b"png");
        assert_eq!(
            fs::read_dir(file.parent().expect("folder"))
                .expect("folder")
                .count(),
            1
        );
        pigoune_core::write_render_file(&file, b"newer").expect("replaced");
        assert_eq!(fs::read(&file).expect("read"), b"newer");
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn a_crop_gives_the_size_of_the_flattened_image_and_no_crop_gives_none() {
        let cropped =
            r#"{"version":1,"crop":{"x":10,"y":20,"width":300,"height":120},"objects":[]}"#;
        assert_eq!(cropped_size(cropped), Some((300, 120)));
        assert_eq!(cropped_size(r#"{"version":1,"objects":[]}"#), None);
        assert_eq!(cropped_size("not a document"), None);
    }

    #[test]
    fn the_size_of_a_png_is_read_from_its_header_and_nothing_else_is_a_size() {
        let folder = std::env::temp_dir().join(format!("pigoune-size-test-{}", std::process::id()));
        fs::create_dir_all(&folder).expect("folder");
        let mut header = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
        header.extend_from_slice(b"IHDR");
        header.extend_from_slice(&640_u32.to_be_bytes());
        header.extend_from_slice(&480_u32.to_be_bytes());
        let png = folder.join("a.png");
        fs::write(&png, &header).expect("written");
        assert_eq!(png_size(&png), Some((640, 480)));
        let text = folder.join("b.png");
        fs::write(&text, b"not an image at all, but long enough to be read").expect("written");
        assert_eq!(png_size(&text), None);
        assert_eq!(png_size(&folder.join("missing.png")), None);
        let linked = folder.join("linked.png");
        std::os::unix::fs::symlink(&png, &linked).expect("link");
        assert_eq!(png_size(&linked), None);
        let _ = fs::remove_dir_all(folder);
    }
}
