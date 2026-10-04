use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use super::{Dimensions, InspectError};

const HEADER_LENGTH: usize = 6;
const ENTRY_LENGTH: usize = 16;
const FULL_SIDE: u32 = 256;
const SINGLE_IMAGE_OFFSET: usize = HEADER_LENGTH + ENTRY_LENGTH;
const PLANES: u16 = 1;
const BITS_PER_PIXEL: u16 = 32;

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

#[must_use]
pub fn single_size_icon(bytes: &[u8], size: Dimensions) -> Option<Vec<u8>> {
    let header = bytes.get(..HEADER_LENGTH)?;
    let count = usize::from(u16::from_le_bytes([header[4], header[5]]));
    let entries = bytes.get(HEADER_LENGTH..HEADER_LENGTH + count * ENTRY_LENGTH)?;
    let (entry, image) = entries
        .as_chunks::<ENTRY_LENGTH>()
        .0
        .iter()
        .filter(|entry| Dimensions::new(side(entry[0]), side(entry[1])) == Some(size))
        .filter_map(|entry| Some((entry, image_of(bytes, entry)?)))
        .max_by_key(|(entry, image)| (u16::from_le_bytes([entry[6], entry[7]]), image.len()))?;
    let mut icon = Vec::with_capacity(SINGLE_IMAGE_OFFSET + image.len());
    icon.extend([0, 0, 1, 0, 1, 0]);
    icon.extend(&entry[..12]);
    icon.extend(u32::try_from(SINGLE_IMAGE_OFFSET).ok()?.to_le_bytes());
    icon.extend(image);
    Some(icon)
}

#[must_use]
pub fn icon_from_pngs(images: &[(Dimensions, &[u8])]) -> Option<Vec<u8>> {
    let count = u16::try_from(images.len()).ok()?;
    let mut icon = vec![0, 0, 1, 0];
    icon.extend(count.to_le_bytes());
    let mut offset = HEADER_LENGTH + ENTRY_LENGTH * images.len();
    for (size, png) in images {
        icon.extend([
            stored_side(size.width())?,
            stored_side(size.height())?,
            0,
            0,
        ]);
        icon.extend(PLANES.to_le_bytes());
        icon.extend(BITS_PER_PIXEL.to_le_bytes());
        icon.extend(u32::try_from(png.len()).ok()?.to_le_bytes());
        icon.extend(u32::try_from(offset).ok()?.to_le_bytes());
        offset += png.len();
    }
    for (_, png) in images {
        icon.extend(*png);
    }
    Some(icon)
}

fn stored_side(side: u32) -> Option<u8> {
    if side == FULL_SIDE {
        Some(0)
    } else {
        u8::try_from(side).ok().filter(|stored| *stored > 0)
    }
}

fn image_of<'a>(bytes: &'a [u8], entry: &[u8; ENTRY_LENGTH]) -> Option<&'a [u8]> {
    let length = usize::try_from(u32::from_le_bytes(entry[8..12].try_into().ok()?)).ok()?;
    let offset = usize::try_from(u32::from_le_bytes(entry[12..16].try_into().ok()?)).ok()?;
    bytes.get(offset..offset.checked_add(length)?)
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
    use super::{icon_from_pngs, read_sizes, single_size_icon};
    use crate::media::Dimensions;

    struct Image {
        side: u8,
        bit_count: u16,
        data: Vec<u8>,
    }

    fn icon_with(images: &[Image]) -> Vec<u8> {
        let count = u16::try_from(images.len()).expect("few entries");
        let mut bytes = vec![0, 0, 1, 0];
        bytes.extend(count.to_le_bytes());
        let mut offset = 6 + 16 * images.len();
        for image in images {
            bytes.extend([image.side, image.side, 0, 0, 1, 0]);
            bytes.extend(image.bit_count.to_le_bytes());
            bytes.extend(
                u32::try_from(image.data.len())
                    .expect("small")
                    .to_le_bytes(),
            );
            bytes.extend(u32::try_from(offset).expect("small").to_le_bytes());
            offset += image.data.len();
        }
        for image in images {
            bytes.extend(&image.data);
        }
        bytes
    }

    fn image(side: u8, bit_count: u16, data: &[u8]) -> Image {
        Image {
            side,
            bit_count,
            data: data.to_vec(),
        }
    }

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

    #[test]
    fn the_chosen_size_becomes_an_icon_of_its_own() {
        let bytes = icon_with(&[image(16, 32, b"small"), image(32, 32, b"medium")]);

        let icon = single_size_icon(&bytes, dimensions(32, 32)).expect("size exists");

        assert_eq!(&icon[..6], [0, 0, 1, 0, 1, 0]);
        assert_eq!(&icon[6..8], [32, 32]);
        assert_eq!(&icon[14..18], 6_u32.to_le_bytes());
        assert_eq!(&icon[18..22], 22_u32.to_le_bytes());
        assert_eq!(&icon[22..], b"medium");
        assert_eq!(
            read_sizes(&mut icon.as_slice()).expect("readable"),
            [dimensions(32, 32)]
        );
    }

    #[test]
    fn the_richest_copy_of_a_size_is_chosen() {
        let bytes = icon_with(&[
            image(16, 4, b"poor"),
            image(16, 32, b"rich"),
            image(16, 8, b"middle"),
        ]);

        let icon = single_size_icon(&bytes, dimensions(16, 16)).expect("size exists");

        assert_eq!(&icon[22..], b"rich");
    }

    #[test]
    fn a_full_side_size_is_found() {
        let bytes = icon_with(&[image(0, 32, b"large")]);

        let icon = single_size_icon(&bytes, dimensions(256, 256)).expect("size exists");

        assert_eq!(&icon[22..], b"large");
    }

    #[test]
    fn a_missing_size_or_a_truncated_image_gives_nothing() {
        let bytes = icon_with(&[image(16, 32, b"small")]);

        assert_eq!(single_size_icon(&bytes, dimensions(48, 48)), None);
        assert_eq!(
            single_size_icon(&bytes[..bytes.len() - 2], dimensions(16, 16)),
            None
        );
        assert_eq!(single_size_icon(&bytes[..10], dimensions(16, 16)), None);
    }

    #[test]
    fn an_icon_is_assembled_from_several_pictures() {
        let small = b"small picture".as_slice();
        let large = b"large picture".as_slice();
        let icon = icon_from_pngs(&[(dimensions(16, 16), small), (dimensions(256, 256), large)])
            .expect("icon assembled");

        assert_eq!(
            read_sizes(&mut icon.as_slice()).expect("readable"),
            [dimensions(16, 16), dimensions(256, 256)]
        );
        let single = single_size_icon(&icon, dimensions(256, 256)).expect("size found");
        assert!(single.ends_with(large));
    }

    #[test]
    fn an_icon_side_cannot_exceed_256_pixels() {
        assert_eq!(icon_from_pngs(&[(dimensions(300, 300), b"picture")]), None);
    }
}
