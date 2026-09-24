use std::{error::Error, fmt, str::FromStr};

/// Stable, MIME-independent format of validated image content.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Svg,
    Ico,
    Icns,
    WebP,
    Avif,
    Gif,
    Bmp,
    Tiff,
    Xpm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseImageFormatError;

impl fmt::Display for ParseImageFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown canonical image format")
    }
}

impl Error for ParseImageFormatError {}

impl ImageFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Svg => "svg",
            Self::Ico => "ico",
            Self::Icns => "icns",
            Self::WebP => "webp",
            Self::Avif => "avif",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Xpm => "xpm",
        }
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ImageFormat {
    type Err = ParseImageFormatError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "png" => Ok(Self::Png),
            "jpeg" => Ok(Self::Jpeg),
            "svg" => Ok(Self::Svg),
            "ico" => Ok(Self::Ico),
            "icns" => Ok(Self::Icns),
            "webp" => Ok(Self::WebP),
            "avif" => Ok(Self::Avif),
            "gif" => Ok(Self::Gif),
            "bmp" => Ok(Self::Bmp),
            "tiff" => Ok(Self::Tiff),
            "xpm" => Ok(Self::Xpm),
            _ => Err(ParseImageFormatError),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidImageDimensions {
    pub width: u32,
    pub height: u32,
}

impl fmt::Display for InvalidImageDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid image dimensions: {} × {}",
            self.width, self.height
        )
    }
}

impl Error for InvalidImageDimensions {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageMetadata {
    format: ImageFormat,
    width: u32,
    height: u32,
    animated: bool,
}

impl ImageMetadata {
    pub fn new(
        format: ImageFormat,
        width: u32,
        height: u32,
        animated: bool,
    ) -> Result<Self, InvalidImageDimensions> {
        if width == 0 || height == 0 {
            return Err(InvalidImageDimensions { width, height });
        }
        Ok(Self {
            format,
            width,
            height,
            animated,
        })
    }

    pub const fn format(self) -> ImageFormat {
        self.format
    }
    pub const fn width(self) -> u32 {
        self.width
    }
    pub const fn height(self) -> u32 {
        self.height
    }
    pub const fn animated(self) -> bool {
        self.animated
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageFormat, ImageMetadata};

    #[test]
    fn canonical_codes_round_trip_and_reject_other_spellings() {
        for format in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Svg,
            ImageFormat::Ico,
            ImageFormat::Icns,
            ImageFormat::WebP,
            ImageFormat::Avif,
            ImageFormat::Gif,
            ImageFormat::Bmp,
            ImageFormat::Tiff,
            ImageFormat::Xpm,
        ] {
            assert_eq!(format.as_str().parse::<ImageFormat>(), Ok(format));
            assert_eq!(format.to_string(), format.as_str());
        }
        assert_eq!(ImageFormat::Icns.as_str(), "icns");
        assert!("PNG".parse::<ImageFormat>().is_err());
        assert!("image/png".parse::<ImageFormat>().is_err());
        assert!("future".parse::<ImageFormat>().is_err());
    }

    #[test]
    fn metadata_requires_positive_dimensions() {
        let metadata = ImageMetadata::new(ImageFormat::Png, 3, 2, false).unwrap();
        assert_eq!(
            (
                metadata.format(),
                metadata.width(),
                metadata.height(),
                metadata.animated()
            ),
            (ImageFormat::Png, 3, 2, false)
        );
        assert!(ImageMetadata::new(ImageFormat::Png, 0, 2, false).is_err());
        assert!(ImageMetadata::new(ImageFormat::Png, 3, 0, false).is_err());
    }
}
