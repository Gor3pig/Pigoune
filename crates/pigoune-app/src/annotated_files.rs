use std::fs;
use std::io::Write;
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
    if render.file.is_file() {
        return Some(render.file.clone());
    }
    let document = AnnotationDocument::from_json(&render.document).ok()?;
    let texture = load_source(source).await?;
    let flat = render_flat(&texture, &document, widget)?;
    let png = image_conversion::png_bytes(&flat).await.ok()?;
    write_atomically(&render.file, &png).ok()?;
    Some(render.file.clone())
}

pub fn png_size(file: &Path) -> Option<(u32, u32)> {
    let mut header = [0_u8; 24];
    std::io::Read::read_exact(&mut fs::File::open(file).ok()?, &mut header).ok()?;
    let width = u32::from_be_bytes(header[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(header[20..24].try_into().ok()?);
    (&header[1..4] == b"PNG" && width > 0 && height > 0).then_some((width, height))
}

pub fn render_flat(
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
    annotation_export::final_texture(&backdrop, document, &widget.pango_context())
}

fn write_atomically(destination: &Path, content: &[u8]) -> std::io::Result<()> {
    let folder = destination
        .parent()
        .ok_or_else(|| std::io::Error::other("a rendering always has a folder"))?;
    fs::create_dir_all(folder)?;
    let partial = destination.with_extension("partial");
    let mut file = fs::File::create(&partial)?;
    file.write_all(content)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&partial, destination).inspect_err(|_| {
        let _ = fs::remove_file(&partial);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rendering_is_written_whole_with_its_folders_and_leaves_no_unfinished_file() {
        let folder =
            std::env::temp_dir().join(format!("pigoune-render-test-{}", std::process::id()));
        let file = folder.join("nested/one.png");
        write_atomically(&file, b"png").expect("written");
        assert_eq!(fs::read(&file).expect("read"), b"png");
        assert!(!file.with_extension("partial").exists());
        write_atomically(&file, b"newer").expect("replaced");
        assert_eq!(fs::read(&file).expect("read"), b"newer");
        let _ = fs::remove_dir_all(folder);
    }
}
