use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::{Dimensions, InspectError};

const SIZE_FIELD_OFFSET: usize = 14;
const DIMENSIONS_OFFSET: usize = 18;
const HEADER_LENGTH: u64 = 26;
const OS2_HEADER_SIZE: u32 = 12;

pub fn size(path: &Path) -> Result<Dimensions, InspectError> {
    let mut header = Vec::new();
    File::open(path)
        .and_then(|file| file.take(HEADER_LENGTH).read_to_end(&mut header))
        .map_err(|_| InspectError::Unreadable)?;
    read_size(&header).ok_or(InspectError::Unreadable)
}

fn read_size(header: &[u8]) -> Option<Dimensions> {
    let header_size = u32::from_le_bytes(field(header, SIZE_FIELD_OFFSET)?);
    if header_size == OS2_HEADER_SIZE {
        let width = u16::from_le_bytes(field(header, DIMENSIONS_OFFSET)?);
        let height = u16::from_le_bytes(field(header, DIMENSIONS_OFFSET + 2)?);
        return Dimensions::new(u32::from(width), u32::from(height));
    }
    let width = i32::from_le_bytes(field(header, DIMENSIONS_OFFSET)?);
    let height = i32::from_le_bytes(field(header, DIMENSIONS_OFFSET + 4)?);
    if width < 0 {
        return None;
    }
    Dimensions::new(width.unsigned_abs(), height.unsigned_abs())
}

fn field<const N: usize>(header: &[u8], offset: usize) -> Option<[u8; N]> {
    header.get(offset..offset + N)?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::read_size;
    use crate::media::Dimensions;

    fn modern(width: i32, height: i32) -> Vec<u8> {
        let mut bytes = b"BM".to_vec();
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&40_u32.to_le_bytes());
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes
    }

    fn os2(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = b"BM".to_vec();
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&12_u32.to_le_bytes());
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes
    }

    #[test]
    fn a_bottom_up_bitmap_has_the_stored_size() {
        assert_eq!(read_size(&modern(100, 60)), Dimensions::new(100, 60));
    }

    #[test]
    fn a_top_down_bitmap_has_a_positive_height() {
        assert_eq!(read_size(&modern(100, -60)), Dimensions::new(100, 60));
    }

    #[test]
    fn the_old_os2_header_uses_sixteen_bit_sizes() {
        assert_eq!(read_size(&os2(320, 200)), Dimensions::new(320, 200));
    }

    #[test]
    fn a_negative_width_or_a_short_header_is_refused() {
        assert_eq!(read_size(&modern(-5, 5)), None);
        assert_eq!(read_size(&modern(0, 5)), None);
        assert_eq!(read_size(b"BM"), None);
    }
}
