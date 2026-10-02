use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use super::{Dimensions, InspectError};

const HEADER_LENGTH: usize = 6;
const ENTRY_LENGTH: usize = 16;
const FULL_SIDE: u32 = 256;

pub fn embedded_sizes(path: &Path) -> Result<Vec<Dimensions>, InspectError> {
    let file = File::open(path).map_err(|_| InspectError::Unreadable)?;
    let sizes = read_sizes(&mut BufReader::new(file)).map_err(|_| InspectError::Unreadable)?;
    if sizes.is_empty() {
        return Err(InspectError::Unreadable);
    }
    Ok(sizes)
}

fn read_sizes(reader: &mut impl Read) -> io::Result<Vec<Dimensions>> {
    let mut header = [0; HEADER_LENGTH];
    reader.read_exact(&mut header)?;
    let count = u16::from_le_bytes([header[4], header[5]]);

    let mut sizes = BTreeSet::new();
    for _ in 0..count {
        let mut entry = [0; ENTRY_LENGTH];
        reader.read_exact(&mut entry)?;
        sizes.extend(Dimensions::new(side(entry[0]), side(entry[1])));
    }
    Ok(sizes.into_iter().collect())
}

fn side(stored: u8) -> u32 {
    if stored == 0 {
        FULL_SIDE
    } else {
        u32::from(stored)
    }
}

#[cfg(test)]
mod tests {
    use super::read_sizes;
    use crate::media::Dimensions;

    fn ico(sides: &[(u8, u8)]) -> Vec<u8> {
        let count = u16::try_from(sides.len()).expect("few entries");
        let mut bytes = vec![0, 0, 1, 0];
        bytes.extend(count.to_le_bytes());
        for &(width, height) in sides {
            bytes.extend([width, height]);
            bytes.extend([0; 14]);
        }
        bytes
    }

    fn dimensions(width: u32, height: u32) -> Dimensions {
        Dimensions::new(width, height).expect("valid dimensions")
    }

    #[test]
    fn sizes_are_listed_from_smallest_to_largest_without_repetition() {
        let bytes = ico(&[(48, 48), (16, 16), (32, 32), (16, 16)]);
        assert_eq!(
            read_sizes(&mut bytes.as_slice()).expect("readable"),
            [dimensions(16, 16), dimensions(32, 32), dimensions(48, 48)]
        );
    }

    #[test]
    fn a_zero_side_means_256_pixels() {
        let bytes = ico(&[(0, 0)]);
        assert_eq!(
            read_sizes(&mut bytes.as_slice()).expect("readable"),
            [dimensions(256, 256)]
        );
    }

    #[test]
    fn a_truncated_directory_is_unreadable() {
        let bytes = ico(&[(16, 16), (32, 32)]);
        assert!(read_sizes(&mut &bytes[..30]).is_err());
    }
}
