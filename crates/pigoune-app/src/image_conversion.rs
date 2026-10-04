use std::path::Path;

use glycin::config::MimeType;
use gtk::gdk;
use gtk::prelude::*;

use crate::thumbnails;

const LARGEST_ICON_SIDE: u32 = 256;
const RGBA_CHANNELS: usize = 4;
const OPAQUE: u16 = 255;
const DEFAULT_QUALITY: u8 = 90;
const WHITE: [u8; 3] = [255, 255, 255];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFormat {
    Png,
    Jpeg,
    Webp,
    Avif,
    Ico,
}

impl TargetFormat {
    pub const ALL: [Self; 5] = [Self::Png, Self::Jpeg, Self::Webp, Self::Avif, Self::Ico];

    pub fn name(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Webp => "WebP",
            Self::Avif => "AVIF",
            Self::Ico => "ICO",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Avif => "avif",
            Self::Ico => "ico",
        }
    }

    pub fn has_quality(self) -> bool {
        matches!(self, Self::Jpeg | Self::Avif)
    }

    pub fn keeps_transparency(self) -> bool {
        self != Self::Jpeg
    }

    fn mime_type(self) -> MimeType {
        match self {
            Self::Png => MimeType::PNG,
            Self::Jpeg => MimeType::JPEG,
            Self::Webp => MimeType::WEBP,
            Self::Avif => MimeType::AVIF,
            Self::Ico => MimeType::ICO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversionSettings {
    pub format: TargetFormat,
    pub quality: u8,
    pub background: [u8; 3],
}

impl Default for ConversionSettings {
    fn default() -> Self {
        Self {
            format: TargetFormat::Png,
            quality: DEFAULT_QUALITY,
            background: WHITE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionError {
    Unreadable,
    TooLargeForIcon,
    EncodingFailed,
}

pub async fn convert(
    file: &Path,
    vector_pixels: u32,
    settings: ConversionSettings,
) -> Result<Vec<u8>, ConversionError> {
    let image = thumbnails::load_detailed(file, vector_pixels)
        .await
        .ok_or(ConversionError::Unreadable)?;
    let texture = image.texture;
    let (width, height) = (
        u32::try_from(texture.width()).map_err(|_| ConversionError::Unreadable)?,
        u32::try_from(texture.height()).map_err(|_| ConversionError::Unreadable)?,
    );
    if settings.format == TargetFormat::Ico && width.max(height) > LARGEST_ICON_SIDE {
        return Err(ConversionError::TooLargeForIcon);
    }
    let rgba = straight_rgba(&texture);
    let (memory_format, pixels) = if settings.format.keeps_transparency() {
        (glycin::MemoryFormat::R8g8b8a8, rgba)
    } else {
        (
            glycin::MemoryFormat::R8g8b8,
            flattened(&rgba, settings.background),
        )
    };
    let mut creator = glycin::Creator::new(settings.format.mime_type())
        .await
        .map_err(|_| ConversionError::EncodingFailed)?;
    creator
        .add_frame(width, height, memory_format, pixels)
        .map_err(|_| ConversionError::EncodingFailed)?;
    if settings.format.has_quality() {
        let _ = creator.set_encoding_quality(settings.quality);
    }
    let encoded = creator
        .create()
        .await
        .map_err(|_| ConversionError::EncodingFailed)?;
    Ok(encoded.data_full())
}

fn straight_rgba(texture: &gdk::Texture) -> Vec<u8> {
    let mut downloader = gdk::TextureDownloader::new(texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    let row_length = usize::try_from(texture.width()).unwrap_or(0) * RGBA_CHANNELS;
    bytes
        .chunks(stride)
        .flat_map(|row| &row[..row_length.min(row.len())])
        .copied()
        .collect()
}

fn flattened(rgba: &[u8], background: [u8; 3]) -> Vec<u8> {
    rgba.as_chunks::<RGBA_CHANNELS>()
        .0
        .iter()
        .flat_map(|[red, green, blue, alpha]| {
            [
                blend(*red, background[0], *alpha),
                blend(*green, background[1], *alpha),
                blend(*blue, background[2], *alpha),
            ]
        })
        .collect()
}

fn blend(color: u8, background: u8, alpha: u8) -> u8 {
    let alpha = u16::from(alpha);
    let mixed = u16::from(color) * alpha + u16::from(background) * (OPAQUE - alpha);
    u8::try_from((mixed + OPAQUE / 2) / OPAQUE).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use gtk::glib;

    use super::{ConversionError, ConversionSettings, TargetFormat, convert, flattened};

    const WHITE: [u8; 3] = [255, 255, 255];

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures")
            .join(name)
    }

    fn converted(name: &str, format: TargetFormat) -> Result<Vec<u8>, ConversionError> {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                context.block_on(convert(
                    &fixture(name),
                    512,
                    ConversionSettings {
                        format,
                        quality: 90,
                        background: WHITE,
                    },
                ))
            })
            .expect("context available")
    }

    fn starts_like(bytes: &[u8], format: TargetFormat) -> bool {
        match format {
            TargetFormat::Png => bytes.starts_with(b"\x89PNG"),
            TargetFormat::Jpeg => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
            TargetFormat::Webp => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
            TargetFormat::Avif => bytes.get(4..12) == Some(b"ftypavif"),
            TargetFormat::Ico => bytes.starts_with(&[0, 0, 1, 0]),
        }
    }

    #[test]
    fn an_image_converts_to_every_target_format() {
        for format in TargetFormat::ALL {
            let bytes = converted("red-dot.png", format).expect("conversion succeeds");
            assert!(starts_like(&bytes, format), "{}", format.name());
        }
    }

    #[test]
    fn a_vector_image_converts_to_pixels() {
        let bytes = converted("dark-circle.svg", TargetFormat::Png).expect("conversion succeeds");
        assert!(starts_like(&bytes, TargetFormat::Png));
    }

    #[test]
    fn an_icon_is_limited_to_256_pixels() {
        assert_eq!(
            converted("dark-circle.svg", TargetFormat::Ico),
            Err(ConversionError::TooLargeForIcon)
        );
    }

    #[test]
    fn an_unreadable_file_is_reported() {
        assert_eq!(
            converted("absent.png", TargetFormat::Png),
            Err(ConversionError::Unreadable)
        );
    }

    #[test]
    fn transparent_pixels_take_the_background_color() {
        let rgba = [0, 0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 128];
        assert_eq!(
            flattened(&rgba, WHITE),
            [255, 255, 255, 255, 0, 0, 127, 127, 127]
        );
    }
}
