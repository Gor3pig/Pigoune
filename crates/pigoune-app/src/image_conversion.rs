use std::path::Path;

use glycin::config::MimeType;
use gtk::prelude::*;
use gtk::{gdk, gio};
use pigoune_core::{Dimensions, RgbaImage, fitted_within};

use crate::export_size::CustomSize;
use crate::icon_sides::IconSides;
use crate::thumbnails;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConversionSettings {
    pub format: TargetFormat,
    pub quality: u8,
    pub background: [u8; 3],
    pub custom: CustomSize,
    pub icon_sides: IconSides,
}

impl Default for ConversionSettings {
    fn default() -> Self {
        Self {
            format: TargetFormat::Png,
            quality: DEFAULT_QUALITY,
            background: WHITE,
            custom: CustomSize::default(),
            icon_sides: IconSides::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionError {
    Unreadable,
    TooSmallForIcon,
    EncodingFailed,
}

pub struct Source<'a> {
    pub file: &'a Path,
    pub is_vector: bool,
    pub natural: (u32, u32),
}

pub struct Converted {
    pub bytes: Vec<u8>,
    pub name_suffix: Option<String>,
}

pub async fn convert(
    source: &Source<'_>,
    settings: ConversionSettings,
) -> Result<Converted, ConversionError> {
    if settings.format == TargetFormat::Ico {
        return Ok(Converted {
            bytes: icon(source, settings.icon_sides).await?,
            name_suffix: None,
        });
    }
    let target = settings.custom.target(source.natural);
    let image = sized_image(source, target).await?;
    let name_suffix = (target != source.natural).then(|| format!("{}x{}", target.0, target.1));
    Ok(Converted {
        bytes: encode(image, settings).await?,
        name_suffix,
    })
}

async fn icon(source: &Source<'_>, sides: IconSides) -> Result<Vec<u8>, ConversionError> {
    let original = if source.is_vector {
        None
    } else {
        Some(load(source.file, source.natural.0.max(source.natural.1)).await?)
    };
    let mut pictures = Vec::new();
    for side in sides.sides() {
        let image = match &original {
            Some(original) if original.width().max(original.height()) < side => continue,
            Some(original) => shrunk(original.clone(), side).await?,
            None => load(source.file, side).await?,
        };
        let png = encode_png(image.centered_on_square(side)).await?;
        pictures.push((
            Dimensions::new(side, side).ok_or(ConversionError::EncodingFailed)?,
            png,
        ));
    }
    if pictures.is_empty() {
        return Err(ConversionError::TooSmallForIcon);
    }
    let parts: Vec<(Dimensions, &[u8])> = pictures
        .iter()
        .map(|(size, png)| (*size, png.as_slice()))
        .collect();
    pigoune_core::icon_from_pngs(&parts).ok_or(ConversionError::EncodingFailed)
}

async fn sized_image(
    source: &Source<'_>,
    (width, height): (u32, u32),
) -> Result<RgbaImage, ConversionError> {
    let image = load(source.file, width.max(height)).await?;
    if (image.width(), image.height()) == (width, height) {
        return Ok(image);
    }
    resized(image, width, height).await
}

async fn resized(image: RgbaImage, width: u32, height: u32) -> Result<RgbaImage, ConversionError> {
    gio::spawn_blocking(move || image.resized(width, height))
        .await
        .map_err(|_| ConversionError::EncodingFailed)
}

async fn shrunk(image: RgbaImage, side: u32) -> Result<RgbaImage, ConversionError> {
    let (width, height) = fitted_within(image.width(), image.height(), side);
    if (width, height) == (image.width(), image.height()) {
        return Ok(image);
    }
    resized(image, width, height).await
}

async fn load(file: &Path, vector_side: u32) -> Result<RgbaImage, ConversionError> {
    let image = thumbnails::load_detailed(file, vector_side)
        .await
        .ok_or(ConversionError::Unreadable)?;
    let texture = image.texture;
    let width = u32::try_from(texture.width()).map_err(|_| ConversionError::Unreadable)?;
    let height = u32::try_from(texture.height()).map_err(|_| ConversionError::Unreadable)?;
    RgbaImage::new(width, height, straight_rgba(&texture)).ok_or(ConversionError::Unreadable)
}

async fn encode(
    image: RgbaImage,
    settings: ConversionSettings,
) -> Result<Vec<u8>, ConversionError> {
    let (width, height) = (image.width(), image.height());
    let (memory_format, pixels) = if settings.format.keeps_transparency() {
        (glycin::MemoryFormat::R8g8b8a8, image.into_pixels())
    } else {
        (
            glycin::MemoryFormat::R8g8b8,
            flattened(image.pixels(), settings.background),
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

async fn encode_png(image: RgbaImage) -> Result<Vec<u8>, ConversionError> {
    let settings = ConversionSettings {
        format: TargetFormat::Png,
        ..ConversionSettings::default()
    };
    encode(image, settings).await
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
    use pigoune_core::Dimensions;

    use super::{
        ConversionError, ConversionSettings, Converted, Source, TargetFormat, convert, flattened,
    };
    use crate::export_size::{CustomSize, SizeUnit};
    use crate::icon_sides::IconSides;

    const WHITE: [u8; 3] = [255, 255, 255];

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures")
            .join(name)
    }

    fn converted(
        name: &str,
        natural: (u32, u32),
        settings: ConversionSettings,
    ) -> Result<Converted, ConversionError> {
        let file = fixture(name);
        let source = Source {
            file: &file,
            is_vector: file.extension().is_some_and(|extension| extension == "svg"),
            natural,
        };
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| context.block_on(convert(&source, settings)))
            .expect("context available")
    }

    fn settings(format: TargetFormat) -> ConversionSettings {
        ConversionSettings {
            format,
            ..ConversionSettings::default()
        }
    }

    fn sized(format: TargetFormat, custom: CustomSize) -> ConversionSettings {
        ConversionSettings {
            custom,
            ..settings(format)
        }
    }

    fn pixels(width: f64, height: f64, linked: bool) -> CustomSize {
        CustomSize {
            unit: SizeUnit::Pixels,
            width,
            height,
            linked,
        }
    }

    fn png_size(bytes: &[u8]) -> (u32, u32) {
        let number = |range: std::ops::Range<usize>| {
            u32::from_be_bytes(bytes[range].try_into().expect("four bytes"))
        };
        (number(16..20), number(20..24))
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
            let converted =
                converted("dark-circle.svg", (24, 24), settings(format)).expect("converted");
            assert!(starts_like(&converted.bytes, format), "{}", format.name());
        }
    }

    #[test]
    fn the_original_size_keeps_the_plain_name() {
        let converted =
            converted("red-dot.png", (3, 2), settings(TargetFormat::Png)).expect("converted");
        assert_eq!(png_size(&converted.bytes), (3, 2));
        assert_eq!(converted.name_suffix, None);
    }

    #[test]
    fn a_vector_image_is_drawn_at_the_requested_size() {
        let converted = converted(
            "dark-circle.svg",
            (24, 24),
            sized(TargetFormat::Png, pixels(300.0, 300.0, true)),
        )
        .expect("converted");
        assert_eq!(png_size(&converted.bytes), (300, 300));
        assert_eq!(converted.name_suffix.as_deref(), Some("300x300"));
    }

    #[test]
    fn a_pixel_image_can_be_shrunk_enlarged_or_stretched() {
        let half = CustomSize::default().with_width(50.0, (8, 8));
        let shrunk =
            converted("navy-tile.bmp", (8, 8), sized(TargetFormat::Png, half)).expect("converted");
        assert_eq!(png_size(&shrunk.bytes), (4, 4));

        let double = CustomSize::default().with_width(200.0, (8, 8));
        let enlarged = converted("navy-tile.bmp", (8, 8), sized(TargetFormat::Png, double))
            .expect("converted");
        assert_eq!(png_size(&enlarged.bytes), (16, 16));

        let stretched = converted(
            "navy-tile.bmp",
            (8, 8),
            sized(TargetFormat::Png, pixels(12.0, 3.0, false)),
        )
        .expect("converted");
        assert_eq!(png_size(&stretched.bytes), (12, 3));
        assert_eq!(stretched.name_suffix.as_deref(), Some("12x3"));
    }

    #[test]
    fn an_icon_holds_every_chosen_size() {
        let converted =
            converted("dark-circle.svg", (24, 24), settings(TargetFormat::Ico)).expect("converted");
        let sizes: Vec<u32> = [16, 32, 48, 256]
            .into_iter()
            .filter(|side| {
                let size = Dimensions::new(*side, *side).expect("valid size");
                pigoune_core::single_size_icon(&converted.bytes, size).is_some()
            })
            .collect();
        assert_eq!(sizes, [16, 32, 48, 256]);
        assert_eq!(converted.name_suffix, None);
    }

    #[test]
    fn an_icon_skips_sizes_larger_than_a_pixel_image() {
        let settings = ConversionSettings {
            icon_sides: IconSides::default().with(64, false),
            ..settings(TargetFormat::Ico)
        };
        assert_eq!(
            converted("navy-tile.bmp", (8, 8), settings).err(),
            Some(ConversionError::TooSmallForIcon)
        );
    }

    #[test]
    fn an_unreadable_file_is_reported() {
        assert_eq!(
            converted("absent.png", (3, 2), settings(TargetFormat::Png)).err(),
            Some(ConversionError::Unreadable)
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
