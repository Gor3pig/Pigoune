use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use super::AssetFormat;

const ORIENTATION_TAG: u16 = 0x0112;
const LARGEST_DIRECTORY: u16 = 512;
const LARGEST_EXIF: u64 = 1 << 20;
const MOST_SEGMENTS: usize = 512;
const MOST_CHUNKS: usize = 256;
const EXIF_PREFIX: &[u8] = b"Exif\0\0";
const SIDEWAYS_FROM: u16 = 5;
const LAST_ORIENTATION: u16 = 8;

pub fn swaps_sides(path: &Path, format: AssetFormat) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    match format {
        AssetFormat::Jpeg => jpeg_orientation(&mut file).is_some_and(turns_sideways),
        AssetFormat::Tiff => tiff_orientation(&mut file).is_some_and(turns_sideways),
        AssetFormat::Webp => webp_orientation(&mut file).is_some_and(turns_sideways),
        AssetFormat::Png => png_orientation(&mut file).is_some_and(turns_sideways),
        _ => false,
    }
}

fn turns_sideways(orientation: u16) -> bool {
    (SIDEWAYS_FROM..=LAST_ORIENTATION).contains(&orientation)
}

fn read_exact<R: Read>(reader: &mut R, length: usize) -> Option<Vec<u8>> {
    let mut buffer = vec![0; length];
    reader.read_exact(&mut buffer).ok()?;
    Some(buffer)
}

fn big_u16(bytes: &[u8]) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(..2)?.try_into().ok()?))
}

fn big_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?))
}

fn little_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?))
}

pub fn tiff_orientation<R: Read + Seek>(reader: &mut R) -> Option<u16> {
    reader.seek(SeekFrom::Start(0)).ok()?;
    let header = read_exact(reader, 8)?;
    let little = match &header[..4] {
        [b'I', b'I', 42, 0] => true,
        [b'M', b'M', 0, 42] => false,
        _ => return None,
    };
    let word = |bytes: &[u8]| {
        if little {
            Some(u16::from_le_bytes(bytes.get(..2)?.try_into().ok()?))
        } else {
            big_u16(bytes)
        }
    };
    let offset = if little {
        little_u32(&header[4..])?
    } else {
        big_u32(&header[4..])?
    };
    reader.seek(SeekFrom::Start(u64::from(offset))).ok()?;
    let count = word(&read_exact(reader, 2)?)?.min(LARGEST_DIRECTORY);
    for _ in 0..count {
        let entry = read_exact(reader, 12)?;
        if word(&entry)? != ORIENTATION_TAG {
            continue;
        }
        let value = word(&entry[8..])?;
        return (1..=LAST_ORIENTATION).contains(&value).then_some(value);
    }
    None
}

fn exif_payload_orientation(payload: &[u8]) -> Option<u16> {
    let tiff = payload.strip_prefix(EXIF_PREFIX).unwrap_or(payload);
    tiff_orientation(&mut Cursor::new(tiff))
}

pub fn jpeg_orientation<R: Read + Seek>(reader: &mut R) -> Option<u16> {
    reader.seek(SeekFrom::Start(0)).ok()?;
    if read_exact(reader, 2)? != [0xFF, 0xD8] {
        return None;
    }
    for _ in 0..MOST_SEGMENTS {
        let mut marker = read_exact(reader, 2)?;
        while marker[0] != 0xFF {
            marker = read_exact(reader, 2)?;
        }
        while marker[1] == 0xFF {
            marker[1] = read_exact(reader, 1)?[0];
        }
        match marker[1] {
            0xDA | 0xD9 => return None,
            0x01 | 0xD0..=0xD7 => continue,
            _ => {}
        }
        let length = usize::from(big_u16(&read_exact(reader, 2)?)?).checked_sub(2)?;
        if marker[1] == 0xE1 {
            let payload = read_exact(reader, length)?;
            if payload.starts_with(EXIF_PREFIX) {
                return exif_payload_orientation(&payload);
            }
        } else {
            reader
                .seek(SeekFrom::Current(i64::try_from(length).ok()?))
                .ok()?;
        }
    }
    None
}

pub fn webp_orientation<R: Read + Seek>(reader: &mut R) -> Option<u16> {
    reader.seek(SeekFrom::Start(0)).ok()?;
    let header = read_exact(reader, 12)?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WEBP" {
        return None;
    }
    for _ in 0..MOST_CHUNKS {
        let chunk = read_exact(reader, 8)?;
        let size = u64::from(little_u32(&chunk[4..])?);
        if &chunk[..4] == b"EXIF" {
            if size > LARGEST_EXIF {
                return None;
            }
            let payload = read_exact(reader, usize::try_from(size).ok()?)?;
            return exif_payload_orientation(&payload);
        }
        let padded = size + (size & 1);
        reader
            .seek(SeekFrom::Current(i64::try_from(padded).ok()?))
            .ok()?;
    }
    None
}

pub fn png_orientation<R: Read + Seek>(reader: &mut R) -> Option<u16> {
    reader.seek(SeekFrom::Start(0)).ok()?;
    if read_exact(reader, 8)? != [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] {
        return None;
    }
    for _ in 0..MOST_CHUNKS {
        let chunk = read_exact(reader, 8)?;
        let size = u64::from(big_u32(&chunk)?);
        match &chunk[4..] {
            b"IDAT" | b"IEND" => return None,
            b"eXIf" => {
                if size > LARGEST_EXIF {
                    return None;
                }
                let payload = read_exact(reader, usize::try_from(size).ok()?)?;
                return exif_payload_orientation(&payload);
            }
            _ => {
                reader
                    .seek(SeekFrom::Current(i64::try_from(size + 4).ok()?))
                    .ok()?;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exif(orientation: u16, little: bool) -> Vec<u8> {
        let mut tiff = if little {
            vec![b'I', b'I', 42, 0, 8, 0, 0, 0]
        } else {
            vec![b'M', b'M', 0, 42, 0, 0, 0, 8]
        };
        let word = |value: u16| {
            if little {
                value.to_le_bytes()
            } else {
                value.to_be_bytes()
            }
        };
        tiff.extend_from_slice(&word(2));
        tiff.extend_from_slice(&word(0x010F));
        tiff.extend_from_slice(&word(3));
        tiff.extend_from_slice(&[0; 8]);
        tiff.extend_from_slice(&word(ORIENTATION_TAG));
        tiff.extend_from_slice(&word(3));
        tiff.extend_from_slice(&if little { [1, 0, 0, 0] } else { [0, 0, 0, 1] });
        tiff.extend_from_slice(&word(orientation));
        tiff.extend_from_slice(&[0, 0]);
        tiff
    }

    fn jpeg(orientation: u16, padding_before: usize) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8];
        let filler = vec![0_u8; padding_before];
        bytes.extend_from_slice(&[0xFF, 0xE0]);
        bytes.extend_from_slice(
            &u16::try_from(filler.len() + 2)
                .expect("small")
                .to_be_bytes(),
        );
        bytes.extend_from_slice(&filler);
        let mut payload = EXIF_PREFIX.to_vec();
        payload.extend(exif(orientation, false));
        bytes.extend_from_slice(&[0xFF, 0xE1]);
        bytes.extend_from_slice(
            &u16::try_from(payload.len() + 2)
                .expect("small")
                .to_be_bytes(),
        );
        bytes.extend_from_slice(&payload);
        bytes.extend_from_slice(&[0xFF, 0xDA, 0, 2]);
        bytes
    }

    fn orientation_of_jpeg(bytes: Vec<u8>) -> Option<u16> {
        jpeg_orientation(&mut Cursor::new(bytes))
    }

    #[test]
    fn every_jpeg_orientation_is_read_and_only_five_to_eight_turn_the_picture() {
        for orientation in 1..=8 {
            assert_eq!(orientation_of_jpeg(jpeg(orientation, 0)), Some(orientation));
            assert_eq!(turns_sideways(orientation), orientation >= 5);
        }
    }

    #[test]
    fn the_exif_segment_is_found_far_beyond_the_first_kilobytes() {
        assert_eq!(orientation_of_jpeg(jpeg(6, 60_000)), Some(6));
    }

    #[test]
    fn a_jpeg_without_exif_or_with_a_damaged_one_has_no_orientation() {
        assert_eq!(
            orientation_of_jpeg(vec![0xFF, 0xD8, 0xFF, 0xDA, 0, 2]),
            None
        );
        let mut broken = jpeg(6, 0);
        broken.truncate(broken.len() - 12);
        assert_eq!(orientation_of_jpeg(broken), None);
        assert_eq!(orientation_of_jpeg(b"not a jpeg at all".to_vec()), None);
        assert_eq!(orientation_of_jpeg(Vec::new()), None);
    }

    #[test]
    fn an_absurd_orientation_value_is_ignored() {
        for value in [0, 9, 300, u16::MAX] {
            assert_eq!(orientation_of_jpeg(jpeg(value, 0)), None);
        }
    }

    #[test]
    fn a_tiff_directory_is_read_in_either_byte_order() {
        for little in [true, false] {
            assert_eq!(
                tiff_orientation(&mut Cursor::new(exif(8, little))),
                Some(8),
                "little endian: {little}"
            );
        }
        assert_eq!(
            tiff_orientation(&mut Cursor::new(b"II*\0\xff\xff\xff\xff".to_vec())),
            None
        );
    }

    #[test]
    fn a_webp_exif_chunk_is_read_with_or_without_its_prefix() {
        for prefixed in [false, true] {
            let mut payload = if prefixed {
                EXIF_PREFIX.to_vec()
            } else {
                Vec::new()
            };
            payload.extend(exif(6, true));
            let mut chunks = b"VP8X".to_vec();
            chunks.extend_from_slice(&10_u32.to_le_bytes());
            chunks.extend_from_slice(&[0; 10]);
            chunks.extend_from_slice(b"EXIF");
            chunks.extend_from_slice(&u32::try_from(payload.len()).expect("small").to_le_bytes());
            chunks.extend_from_slice(&payload);
            let mut bytes = b"RIFF".to_vec();
            bytes.extend_from_slice(
                &u32::try_from(chunks.len() + 4)
                    .expect("small")
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(b"WEBP");
            bytes.extend_from_slice(&chunks);
            assert_eq!(
                webp_orientation(&mut Cursor::new(bytes)),
                Some(6),
                "prefixed: {prefixed}"
            );
        }
    }

    #[test]
    fn a_png_exif_chunk_before_the_pixels_is_read_and_one_after_is_ignored() {
        let signature = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        let chunk = |kind: &[u8; 4], data: &[u8]| {
            let mut bytes = u32::try_from(data.len())
                .expect("small")
                .to_be_bytes()
                .to_vec();
            bytes.extend_from_slice(kind);
            bytes.extend_from_slice(data);
            bytes.extend_from_slice(&[0; 4]);
            bytes
        };
        let mut before = signature.to_vec();
        before.extend(chunk(b"IHDR", &[0; 13]));
        before.extend(chunk(b"eXIf", &exif(8, false)));
        before.extend(chunk(b"IDAT", &[0; 4]));
        assert_eq!(png_orientation(&mut Cursor::new(before)), Some(8));
        let mut after = signature.to_vec();
        after.extend(chunk(b"IHDR", &[0; 13]));
        after.extend(chunk(b"IDAT", &[0; 4]));
        after.extend(chunk(b"eXIf", &exif(8, false)));
        assert_eq!(png_orientation(&mut Cursor::new(after)), None);
    }
}
