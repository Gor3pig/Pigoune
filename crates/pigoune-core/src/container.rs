use std::{error::Error, fmt};

pub const ICO_MAX_ENTRIES: usize = 1_024;
pub const ICO_MAX_DIMENSION: u32 = 4_096;
pub const ICO_MAX_DECODED_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerCodec {
    Png,
    Dib,
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
            || width > ICO_MAX_DIMENSION
            || height > ICO_MAX_DIMENSION
            || decoded_bytes.is_none_or(|bytes| bytes > ICO_MAX_DECODED_BYTES)
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
        if representations.len() > ICO_MAX_ENTRIES {
            return Err(ContainerMetadataError::TooManyRepresentations);
        }
        for (index, item) in representations.iter().enumerate() {
            if usize::from(item.ordinal) != index {
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
        &self.representations[usize::from(self.primary_ordinal)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let wrong = ContainerRepresentation::new(2, 16, 16, None, ContainerCodec::Png, 12, Some(2))
            .unwrap();
        assert_eq!(
            ContainerMetadata::new(vec![wrong], 2),
            Err(ContainerMetadataError::InvalidOrdinal)
        );
        assert!(
            ContainerRepresentation::new(0, 4097, 1, None, ContainerCodec::Png, 12, None).is_err()
        );
        assert!(
            ContainerRepresentation::new(0, 4096, 4096, None, ContainerCodec::Png, 12, None)
                .is_ok()
        );
        assert_eq!(4096_u64 * 4096 * 4, ICO_MAX_DECODED_BYTES);
        assert!(
            ContainerRepresentation::new(0, 0, 1, None, ContainerCodec::Png, 12, None).is_err()
        );
    }
}
