use std::{error::Error, fmt, str::FromStr};

pub const ICON_CONTAINER_MAX_REPRESENTATIONS: usize = 1_024;
pub const ICON_CONTAINER_MAX_DIMENSION: u32 = 4_096;
pub const ICON_CONTAINER_MAX_DECODED_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerCodec {
    Png,
    Dib,
    Jpeg2000,
    IcnsRgb,
    IcnsArgb,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseContainerCodecError;

impl fmt::Display for ParseContainerCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown canonical container codec")
    }
}

impl Error for ParseContainerCodecError {}

impl ContainerCodec {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Dib => "dib",
            Self::Jpeg2000 => "jpeg2000",
            Self::IcnsRgb => "icns-rgb",
            Self::IcnsArgb => "icns-argb",
        }
    }
}

impl fmt::Display for ContainerCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ContainerCodec {
    type Err = ParseContainerCodecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "png" => Ok(Self::Png),
            "dib" => Ok(Self::Dib),
            "jpeg2000" => Ok(Self::Jpeg2000),
            "icns-rgb" => Ok(Self::IcnsRgb),
            "icns-argb" => Ok(Self::IcnsArgb),
            _ => Err(ParseContainerCodecError),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerRepresentation {
    ordinal: u16,
    width: u32,
    height: u32,
    bit_depth: Option<u16>,
    codec: ContainerCodec,
    encoded_size: u64,
    scale: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerMetadataError {
    Empty,
    TooManyRepresentations,
    InvalidRepresentation,
    InvalidOrdinal,
    MissingPrimary,
}

impl fmt::Display for ContainerMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid container metadata: {self:?}")
    }
}

impl Error for ContainerMetadataError {}

impl ContainerRepresentation {
    pub fn new(
        ordinal: u16,
        width: u32,
        height: u32,
        bit_depth: Option<u16>,
        codec: ContainerCodec,
        encoded_size: u64,
        scale: Option<u8>,
    ) -> Result<Self, ContainerMetadataError> {
        let decoded_bytes = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|area| area.checked_mul(4));
        if width == 0
            || height == 0
            || width > ICON_CONTAINER_MAX_DIMENSION
            || height > ICON_CONTAINER_MAX_DIMENSION
            || decoded_bytes.is_none_or(|bytes| bytes > ICON_CONTAINER_MAX_DECODED_BYTES)
            || encoded_size == 0
            || bit_depth == Some(0)
            || scale == Some(0)
        {
            return Err(ContainerMetadataError::InvalidRepresentation);
        }
        Ok(Self {
            ordinal,
            width,
            height,
            bit_depth,
            codec,
            encoded_size,
            scale,
        })
    }

    /// Physical element ordinal in the complete icon container.
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }
    pub const fn width(&self) -> u32 {
        self.width
    }
    pub const fn height(&self) -> u32 {
        self.height
    }
    pub const fn bit_depth(&self) -> Option<u16> {
        self.bit_depth
    }
    pub const fn codec(&self) -> ContainerCodec {
        self.codec
    }
    /// Total encoded payload bytes required by this logical representation.
    /// Legacy ICNS RGB includes its separate required mask payload.
    pub const fn encoded_size(&self) -> u64 {
        self.encoded_size
    }
    pub const fn scale(&self) -> Option<u8> {
        self.scale
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerMetadata {
    representations: Vec<ContainerRepresentation>,
    primary_ordinal: u16,
}

impl ContainerMetadata {
    pub fn new(
        representations: Vec<ContainerRepresentation>,
        primary_ordinal: u16,
    ) -> Result<Self, ContainerMetadataError> {
        if representations.is_empty() {
            return Err(ContainerMetadataError::Empty);
        }
        if representations.len() > ICON_CONTAINER_MAX_REPRESENTATIONS {
            return Err(ContainerMetadataError::TooManyRepresentations);
        }
        for pair in representations.windows(2) {
            if pair[0].ordinal >= pair[1].ordinal {
                return Err(ContainerMetadataError::InvalidOrdinal);
            }
        }
        if !representations
            .iter()
            .any(|item| item.ordinal == primary_ordinal)
        {
            return Err(ContainerMetadataError::MissingPrimary);
        }
        Ok(Self {
            representations,
            primary_ordinal,
        })
    }

    pub fn representations(&self) -> &[ContainerRepresentation] {
        &self.representations
    }
    pub const fn primary_ordinal(&self) -> u16 {
        self.primary_ordinal
    }
    pub fn primary(&self) -> &ContainerRepresentation {
        self.representations
            .iter()
            .find(|item| item.ordinal == self.primary_ordinal)
            .expect("primary ordinal checked by constructor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_codec_codes_round_trip() {
        for codec in [
            ContainerCodec::Png,
            ContainerCodec::Dib,
            ContainerCodec::Jpeg2000,
            ContainerCodec::IcnsRgb,
            ContainerCodec::IcnsArgb,
        ] {
            assert_eq!(codec.as_str().parse::<ContainerCodec>(), Ok(codec));
            assert_eq!(codec.to_string(), codec.as_str());
        }
        assert!("PNG".parse::<ContainerCodec>().is_err());
        assert!("image/png".parse::<ContainerCodec>().is_err());
        assert!("future".parse::<ContainerCodec>().is_err());
    }

    #[test]
    fn validates_inventory_and_budget() {
        let item =
            ContainerRepresentation::new(0, 256, 256, Some(32), ContainerCodec::Dib, 12, None)
                .unwrap();
        assert_eq!(
            ContainerMetadata::new(vec![item.clone()], 0)
                .unwrap()
                .primary(),
            &item
        );
        assert_eq!(
            ContainerMetadata::new(vec![], 0),
            Err(ContainerMetadataError::Empty)
        );
        assert_eq!(
            ContainerMetadata::new(vec![item.clone()], 1),
            Err(ContainerMetadataError::MissingPrimary)
        );
        let second =
            ContainerRepresentation::new(2, 16, 16, None, ContainerCodec::Png, 12, Some(2))
                .unwrap();
        let third = ContainerRepresentation::new(5, 32, 32, None, ContainerCodec::Png, 12, Some(1))
            .unwrap();
        let sparse =
            ContainerMetadata::new(vec![item.clone(), second.clone(), third.clone()], 2).unwrap();
        assert_eq!(sparse.primary(), &second);
        assert_eq!(
            ContainerMetadata::new(vec![second.clone(), second.clone()], 2),
            Err(ContainerMetadataError::InvalidOrdinal)
        );
        assert_eq!(
            ContainerMetadata::new(vec![second.clone(), item.clone()], 2),
            Err(ContainerMetadataError::InvalidOrdinal)
        );
        assert_eq!(
            ContainerMetadata::new(vec![item, second, third], 1),
            Err(ContainerMetadataError::MissingPrimary)
        );
        assert!(
            ContainerRepresentation::new(0, 4097, 1, None, ContainerCodec::Png, 12, None).is_err()
        );
        assert!(
            ContainerRepresentation::new(0, 4096, 4096, None, ContainerCodec::Png, 12, None)
                .is_ok()
        );
        assert_eq!(4096_u64 * 4096 * 4, ICON_CONTAINER_MAX_DECODED_BYTES);
        assert!(
            ContainerRepresentation::new(0, 0, 1, None, ContainerCodec::Png, 12, None).is_err()
        );
    }
}
