use std::fs::File;
use std::io::Read;
use std::path::Path;

use roxmltree::Document;

use super::{Archive, AssetFormat, Dimensions, InspectError, LARGEST_SIDE, MediaInfo, format};

const MERGED_IMAGE: &str = "mergedimage.png";
const KRITA_PREVIEW: &str = "preview.png";
const OPENRASTER_THUMBNAIL: &str = "Thumbnails/thumbnail.png";
const KRITA_DOCUMENT: &str = "maindoc.xml";
const OPENRASTER_DOCUMENT: &str = "stack.xml";
const MOST_IMAGE_BYTES: u64 = 256 * 1024 * 1024;
const MOST_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;
const HEADER_LENGTH: u64 = 4096;

#[must_use]
pub fn layered_format_of(path: &Path) -> Option<AssetFormat> {
    let mut header = Vec::new();
    File::open(path)
        .ok()?
        .take(HEADER_LENGTH)
        .read_to_end(&mut header)
        .ok()?;
    format::detect_binary(&header)
        .filter(|format| matches!(format, AssetFormat::Kra | AssetFormat::Ora))
}

#[must_use]
pub fn is_layered_file(path: &Path) -> bool {
    layered_format_of(path).is_some()
}

#[must_use]
pub fn layered_preview_png(path: &Path) -> Option<Vec<u8>> {
    let format = layered_format_of(path)?;
    let mut archive = Archive::open(path).ok()?;
    let fallback = match format {
        AssetFormat::Kra => KRITA_PREVIEW,
        _ => OPENRASTER_THUMBNAIL,
    };
    [MERGED_IMAGE, fallback]
        .into_iter()
        .find_map(|name| archive.read_named(name, MOST_IMAGE_BYTES).ok())
}

pub fn inspect(path: &Path, format: AssetFormat) -> Result<MediaInfo, InspectError> {
    let mut archive = Archive::open(path).map_err(|_| InspectError::Unreadable)?;
    let dimensions = document_dimensions(&mut archive, format)
        .or_else(|| merged_image_dimensions(&mut archive))
        .ok_or(InspectError::Unreadable)?;
    Ok(MediaInfo {
        format,
        dimensions: Some(dimensions),
        is_animated: false,
        embedded_sizes: Vec::new(),
    })
}

fn document_dimensions(archive: &mut Archive<File>, format: AssetFormat) -> Option<Dimensions> {
    let (document, element, width, height) = if format == AssetFormat::Kra {
        (KRITA_DOCUMENT, "IMAGE", "width", "height")
    } else {
        (OPENRASTER_DOCUMENT, "image", "w", "h")
    };
    let bytes = archive.read_named(document, MOST_DOCUMENT_BYTES).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let document = Document::parse(text).ok()?;
    let node = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == element)?;
    let side = |name: &str| node.attribute(name)?.trim().parse::<f64>().ok();
    checked_dimensions(side(width)?, side(height)?)
}

fn merged_image_dimensions(archive: &mut Archive<File>) -> Option<Dimensions> {
    let bytes = archive.read_named(MERGED_IMAGE, MOST_IMAGE_BYTES).ok()?;
    let size = imagesize::blob_size(&bytes).ok()?;
    checked_dimensions(
        f64::from(u32::try_from(size.width).ok()?),
        f64::from(u32::try_from(size.height).ok()?),
    )
}

fn checked_dimensions(width: f64, height: f64) -> Option<Dimensions> {
    let sides = [width, height];
    if sides
        .iter()
        .any(|side| !side.is_finite() || *side < 1.0 || *side > f64::from(LARGEST_SIDE))
    {
        return None;
    }
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Dimensions::new(width.round() as u32, height.round() as u32)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{inspect, is_layered_file, layered_format_of, layered_preview_png};
    use crate::media::{AssetFormat, Dimensions, InspectError};

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn size(width: u32, height: u32) -> Option<Dimensions> {
        Dimensions::new(width, height)
    }

    #[test]
    fn tells_krita_and_openraster_files_by_their_mimetype() {
        assert_eq!(
            layered_format_of(&fixture("gradient.kra")),
            Some(AssetFormat::Kra)
        );
        assert_eq!(
            layered_format_of(&fixture("gradient.ora")),
            Some(AssetFormat::Ora)
        );
        assert_eq!(layered_format_of(&fixture("not-an-image.zip")), None);
        assert_eq!(
            layered_format_of(&fixture("python-archive.zip")),
            Some(AssetFormat::Ora)
        );
        assert!(!is_layered_file(&fixture("red-dot.png")));
        assert!(!is_layered_file(&fixture("missing.kra")));
    }

    #[test]
    fn reads_the_size_from_the_document() {
        for (name, format) in [
            ("gradient.kra", AssetFormat::Kra),
            ("gradient.ora", AssetFormat::Ora),
            ("lightweight.kra", AssetFormat::Kra),
            ("old-without-merged.ora", AssetFormat::Ora),
        ] {
            let info = inspect(&fixture(name), format).expect(name);

            assert_eq!(info.format, format, "{name}");
            assert_eq!(info.dimensions, size(16, 8), "{name}");
            assert!(!info.is_animated, "{name}");
        }
    }

    #[test]
    fn the_merged_image_is_the_preview_and_the_small_one_is_the_fallback() {
        let merged = layered_preview_png(&fixture("gradient.ora")).expect("merged image");
        let fallback = layered_preview_png(&fixture("old-without-merged.ora")).expect("thumbnail");
        let krita_fallback = layered_preview_png(&fixture("lightweight.kra")).expect("preview");

        assert_eq!(
            imagesize::blob_size(&merged)
                .map(|s| (s.width, s.height))
                .ok(),
            Some((16, 8))
        );
        assert_eq!(
            imagesize::blob_size(&fallback)
                .map(|s| (s.width, s.height))
                .ok(),
            Some((8, 4))
        );
        assert_eq!(
            imagesize::blob_size(&krita_fallback)
                .map(|s| (s.width, s.height))
                .ok(),
            Some((8, 4))
        );
        assert_eq!(layered_preview_png(&fixture("red-dot.png")), None);
    }

    #[test]
    fn a_damaged_archive_is_unreadable() {
        let bytes = std::fs::read(fixture("gradient.kra")).expect("fixture");
        let folder = std::env::temp_dir().join(format!("pigoune-layered-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("folder");
        let cut = folder.join("cut.kra");
        std::fs::write(&cut, &bytes[..bytes.len() / 2]).expect("file");

        let outcome = inspect(&cut, AssetFormat::Kra);
        std::fs::remove_dir_all(&folder).expect("folder removed");

        assert_eq!(outcome, Err(InspectError::Unreadable));
    }
}
