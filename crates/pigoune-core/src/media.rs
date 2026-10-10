mod bmp;
mod color;
mod dimensions;
mod format;
mod frame_timing;
mod gif;
mod ico;
mod png;
mod rgb;
mod shape;
mod svg;
mod webp;

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

pub use color::{
    AssetColor, DominantColor, dominant_colors, dominant_from_text, dominant_text,
    families_from_text, families_text,
};
pub use dimensions::Dimensions;
pub use format::AssetFormat;
pub use ico::{icon_from_pngs, single_size_icon};
pub use rgb::Rgb;
pub use shape::{AssetShape, shapes_from_text, shapes_text};

const HEADER_LENGTH: u64 = 4096;
pub const LARGEST_SIDE: u32 = 65_535;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    pub format: AssetFormat,
    pub dimensions: Option<Dimensions>,
    pub is_animated: bool,
    pub embedded_sizes: Vec<Dimensions>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationTiming {
    pub frames: usize,
    pub duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectError {
    Unsupported,
    Unreadable,
}

pub fn inspect(path: &Path) -> Result<MediaInfo, InspectError> {
    let header = read_header(path)?;
    if let Some(format) = format::detect_binary(&header) {
        return inspect_binary(path, format);
    }
    if format::looks_like_markup(&header) {
        return svg::inspect(path);
    }
    Err(InspectError::Unsupported)
}

#[must_use]
pub fn animation_timing(path: &Path, format: AssetFormat) -> Option<AnimationTiming> {
    match format {
        AssetFormat::Gif => gif::timing(path),
        AssetFormat::Png => png::timing(path),
        AssetFormat::Webp => webp::timing(path),
        _ => None,
    }
}

fn read_header(path: &Path) -> Result<Vec<u8>, InspectError> {
    let file = File::open(path).map_err(|_| InspectError::Unreadable)?;
    let mut header = Vec::new();
    file.take(HEADER_LENGTH)
        .read_to_end(&mut header)
        .map_err(|_| InspectError::Unreadable)?;
    Ok(header)
}

fn inspect_binary(path: &Path, format: AssetFormat) -> Result<MediaInfo, InspectError> {
    if format == AssetFormat::Ico {
        let embedded_sizes = ico::embedded_sizes(path)?;
        return Ok(MediaInfo {
            format,
            dimensions: embedded_sizes.last().copied(),
            is_animated: false,
            embedded_sizes,
        });
    }

    Ok(MediaInfo {
        format,
        dimensions: Some(probe_dimensions(path, format)?),
        is_animated: is_animated(path, format)?,
        embedded_sizes: Vec::new(),
    })
}

fn probe_dimensions(path: &Path, format: AssetFormat) -> Result<Dimensions, InspectError> {
    let dimensions = if format == AssetFormat::Bmp {
        bmp::size(path)?
    } else {
        let size = imagesize::size(path).map_err(|_| InspectError::Unreadable)?;
        let width = u32::try_from(size.width).map_err(|_| InspectError::Unreadable)?;
        let height = u32::try_from(size.height).map_err(|_| InspectError::Unreadable)?;
        Dimensions::new(width, height).ok_or(InspectError::Unreadable)?
    };
    if dimensions.width().max(dimensions.height()) > LARGEST_SIDE {
        return Err(InspectError::Unreadable);
    }
    Ok(dimensions)
}

pub fn is_animated(path: &Path, format: AssetFormat) -> Result<bool, InspectError> {
    match format {
        AssetFormat::Gif => gif::is_animated(path),
        AssetFormat::Png => png::is_animated(path),
        AssetFormat::Webp => webp::is_animated(path),
        _ => Ok(false),
    }
}
