use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const PNG_MIME_TYPE: &str = "image/png";
const ORIGINAL_FILE_PREFIX: &str = "application/octet-stream;name=";
const SUGGESTED_NAME_MIME_TYPE: &str = "application/x-moz-file-promise-dest-filename";
const CHUNK_BYTES: usize = 1 << 16;
const LARGEST_IMAGE_BYTES: usize = 256 << 20;
const PNG_EXTENSION: &str = "png";

pub struct DroppedImage {
    pub bytes: Vec<u8>,
    pub name: Option<String>,
    pub extension: String,
}

pub enum Dropped {
    Files(Vec<PathBuf>),
    Image(DroppedImage),
    Nothing,
}

pub fn formats() -> gdk::ContentFormats {
    gdk::ContentFormats::builder()
        .add_type(gdk::FileList::static_type())
        .add_mime_type("text/uri-list")
        .add_mime_type(PNG_MIME_TYPE)
        .add_mime_type("text/html")
        .add_mime_type("text/plain")
        .add_mime_type("text/x-moz-url")
        .build()
}

pub fn may_hold_an_image(available: &gdk::ContentFormats) -> bool {
    available.contains_type(gdk::FileList::static_type())
        || available.contain_mime_type(PNG_MIME_TYPE)
        || original_file_mime_type(available).is_some()
}

pub async fn read(drop: &gdk::Drop) -> Dropped {
    let available = drop.formats();
    if available.contains_type(gdk::FileList::static_type()) {
        let paths = local_files(drop).await;
        if !paths.is_empty() {
            return Dropped::Files(paths);
        }
    }
    if let Some(image) = original_file(drop, &available).await {
        return Dropped::Image(image);
    }
    if let Some(image) = png_copy(drop, &available).await {
        return Dropped::Image(image);
    }
    Dropped::Nothing
}

fn original_file_mime_type(available: &gdk::ContentFormats) -> Option<String> {
    available
        .mime_types()
        .iter()
        .map(ToString::to_string)
        .find(|mime| mime.starts_with(ORIGINAL_FILE_PREFIX))
}

async fn original_file(drop: &gdk::Drop, available: &gdk::ContentFormats) -> Option<DroppedImage> {
    let mime = original_file_mime_type(available)?;
    let bytes = read_all(drop, &mime)
        .await
        .filter(|bytes| !bytes.is_empty())?;
    let (name, extension) = split_file_name(&file_name_of(&mime)?);
    Some(DroppedImage {
        bytes,
        name,
        extension,
    })
}

async fn png_copy(drop: &gdk::Drop, available: &gdk::ContentFormats) -> Option<DroppedImage> {
    if !available.contain_mime_type(PNG_MIME_TYPE) {
        return None;
    }
    let bytes = read_all(drop, PNG_MIME_TYPE).await?;
    if !bytes.starts_with(PNG_SIGNATURE) {
        return None;
    }
    let name = if available.contain_mime_type(SUGGESTED_NAME_MIME_TYPE) {
        read_all(drop, SUGGESTED_NAME_MIME_TYPE)
            .await
            .and_then(|bytes| suggested_name(&bytes))
    } else {
        None
    };
    Some(DroppedImage {
        bytes,
        name,
        extension: PNG_EXTENSION.to_owned(),
    })
}

async fn local_files(drop: &gdk::Drop) -> Vec<PathBuf> {
    let Ok(value) = drop
        .read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT)
        .await
    else {
        return Vec::new();
    };
    let Ok(files) = value.get::<gdk::FileList>() else {
        return Vec::new();
    };
    files.files().iter().filter_map(gio::File::path).collect()
}

async fn read_all(drop: &gdk::Drop, mime_type: &str) -> Option<Vec<u8>> {
    let (stream, _) = drop
        .read_future(&[mime_type], glib::Priority::DEFAULT)
        .await
        .ok()?;
    let mut bytes = Vec::new();
    loop {
        let chunk = stream
            .read_bytes_future(CHUNK_BYTES, glib::Priority::DEFAULT)
            .await
            .ok()?;
        if chunk.is_empty() {
            return Some(bytes);
        }
        bytes.extend_from_slice(&chunk);
        if bytes.len() > LARGEST_IMAGE_BYTES {
            return None;
        }
    }
}

fn file_name_of(mime: &str) -> Option<String> {
    let name = mime
        .strip_prefix(ORIGINAL_FILE_PREFIX)?
        .trim()
        .trim_matches('"')
        .trim();
    (!name.is_empty()).then(|| name.to_owned())
}

fn split_file_name(file_name: &str) -> (Option<String>, String) {
    let path = Path::new(file_name);
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().trim().to_owned())
        .filter(|stem| !stem.is_empty() && !stem.starts_with('.'));
    let extension = path.extension().map_or_else(String::new, |extension| {
        extension.to_string_lossy().into_owned()
    });
    (name, extension)
}

fn suggested_name(utf16_little_endian: &[u8]) -> Option<String> {
    let units: Vec<u16> = utf16_little_endian
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    let full = String::from_utf16_lossy(&units);
    split_file_name(full.trim_matches('\0').trim()).0
}

#[cfg(test)]
mod tests {
    use super::{file_name_of, split_file_name, suggested_name};

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn the_extension_of_the_suggested_name_is_dropped() {
        assert_eq!(
            suggested_name(&utf16("amazon-devices.webp")).as_deref(),
            Some("amazon-devices")
        );
        assert_eq!(
            suggested_name(&utf16("été 2024.jpg")).as_deref(),
            Some("été 2024")
        );
    }

    #[test]
    fn a_name_without_extension_is_kept_and_padding_is_ignored() {
        let mut bytes = utf16("logo");
        bytes.extend_from_slice(&[0, 0]);
        assert_eq!(suggested_name(&bytes).as_deref(), Some("logo"));
    }

    #[test]
    fn an_empty_name_gives_nothing() {
        assert_eq!(suggested_name(&[]), None);
        assert_eq!(suggested_name(&utf16("   ")), None);
        assert_eq!(suggested_name(&utf16(".jpg")), None);
    }

    #[test]
    fn the_original_file_name_is_read_from_the_type() {
        assert_eq!(
            file_name_of("application/octet-stream;name=\"un-noyau-de-pomme.webp\"").as_deref(),
            Some("un-noyau-de-pomme.webp")
        );
        assert_eq!(file_name_of("application/octet-stream;name=\"\""), None);
        assert_eq!(file_name_of("text/plain"), None);
    }

    #[test]
    fn a_file_name_is_split_into_name_and_extension() {
        assert_eq!(
            split_file_name("photo.final.JPG"),
            (Some("photo.final".to_owned()), "JPG".to_owned())
        );
        assert_eq!(
            split_file_name("logo"),
            (Some("logo".to_owned()), String::new())
        );
        assert_eq!(split_file_name(".jpg"), (None, String::new()));
    }
}
