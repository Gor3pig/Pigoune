use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::{Dimensions, InspectError};

const SIGNATURE: &[u8; 4] = b"icns";
const HEADER_LENGTH: u64 = 8;
const HEADER_BYTES: usize = 8;
const MOST_ENTRIES: usize = 512;
const MOST_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_2000_SIGNATURES: [&[u8]; 2] = [b"\0\0\0\x0cjP  \r\n\x87\n", b"\xFF\x4F\xFF\x51"];
const SIGNATURE_LENGTH: usize = 12;
const KNOWN_SIDES: [([u8; 4], u32); 17] = [
    (*b"icp4", 16),
    (*b"icp5", 32),
    (*b"icp6", 64),
    (*b"ic07", 128),
    (*b"ic08", 256),
    (*b"ic09", 512),
    (*b"ic10", 1024),
    (*b"ic11", 32),
    (*b"ic12", 64),
    (*b"ic13", 256),
    (*b"ic14", 512),
    (*b"ic04", 16),
    (*b"ic05", 32),
    (*b"is32", 16),
    (*b"il32", 32),
    (*b"ih32", 48),
    (*b"it32", 128),
];

struct Entry {
    side: u32,
    data_offset: u64,
    data_length: u64,
    is_modern_image: bool,
}

#[must_use]
pub fn is_icns_file(path: &Path) -> bool {
    let mut signature = [0_u8; SIGNATURE.len()];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut signature))
        .is_ok()
        && signature == *SIGNATURE
}

pub fn embedded_sizes(path: &Path) -> Result<Vec<Dimensions>, InspectError> {
    let mut file = File::open(path).map_err(|_| InspectError::Unreadable)?;
    let entries = read_entries(&mut file)?;
    let mut sides: Vec<u32> = entries.iter().map(|entry| entry.side).collect();
    sides.sort_unstable();
    sides.dedup();
    if sides.is_empty() {
        return Err(InspectError::Unreadable);
    }
    Ok(sides
        .into_iter()
        .filter_map(|side| Dimensions::new(side, side))
        .collect())
}

#[must_use]
pub fn largest_image(path: &Path) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let entries = read_entries(&mut file).ok()?;
    let best = entries
        .iter()
        .filter(|entry| entry.is_modern_image && entry.data_length <= MOST_IMAGE_BYTES)
        .max_by_key(|entry| entry.side)?;
    file.seek(SeekFrom::Start(best.data_offset)).ok()?;
    let mut data = Vec::new();
    file.take(best.data_length).read_to_end(&mut data).ok()?;
    (u64::try_from(data.len()).ok()? == best.data_length).then_some(data)
}

fn read_entries<R: Read + Seek>(reader: &mut R) -> Result<Vec<Entry>, InspectError> {
    let length = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| InspectError::Unreadable)?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|_| InspectError::Unreadable)?;
    let mut header = [0_u8; HEADER_BYTES];
    reader
        .read_exact(&mut header)
        .map_err(|_| InspectError::Unreadable)?;
    if header[..4] != *SIGNATURE {
        return Err(InspectError::Unsupported);
    }
    let declared = u64::from(u32::from_be_bytes([
        header[4], header[5], header[6], header[7],
    ]));
    let end = declared.clamp(HEADER_LENGTH, length);
    let mut entries = Vec::new();
    let mut offset = HEADER_LENGTH;
    while offset + HEADER_LENGTH <= end && entries.len() < MOST_ENTRIES {
        reader
            .seek(SeekFrom::Start(offset))
            .map_err(|_| InspectError::Unreadable)?;
        let mut entry_header = [0_u8; HEADER_BYTES];
        reader
            .read_exact(&mut entry_header)
            .map_err(|_| InspectError::Unreadable)?;
        let kind = [
            entry_header[0],
            entry_header[1],
            entry_header[2],
            entry_header[3],
        ];
        let size = u64::from(u32::from_be_bytes([
            entry_header[4],
            entry_header[5],
            entry_header[6],
            entry_header[7],
        ]));
        if size < HEADER_LENGTH || offset + size > end {
            break;
        }
        if let Some(side) = KNOWN_SIDES
            .iter()
            .find(|known| known.0 == kind)
            .map(|known| known.1)
        {
            let data_offset = offset + HEADER_LENGTH;
            let data_length = size - HEADER_LENGTH;
            entries.push(Entry {
                side,
                data_offset,
                data_length,
                is_modern_image: starts_like_an_image(reader, data_length)?,
            });
        }
        offset += size;
    }
    Ok(entries)
}

fn starts_like_an_image<R: Read>(reader: &mut R, data_length: u64) -> Result<bool, InspectError> {
    let wanted = usize::try_from(data_length.min(SIGNATURE_LENGTH as u64))
        .map_err(|_| InspectError::Unreadable)?;
    let mut start = vec![0_u8; wanted];
    reader
        .read_exact(&mut start)
        .map_err(|_| InspectError::Unreadable)?;
    Ok(start.starts_with(PNG_SIGNATURE)
        || JPEG_2000_SIGNATURES
            .iter()
            .any(|signature| start.starts_with(signature)))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{embedded_sizes, is_icns_file, largest_image};
    use crate::media::InspectError;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn sides(path: &Path) -> Vec<u32> {
        embedded_sizes(path)
            .expect("the sizes are read")
            .into_iter()
            .map(crate::media::Dimensions::width)
            .collect()
    }

    #[test]
    fn lists_the_sizes_of_a_modern_icon_file() {
        assert_eq!(sides(&fixture("modern.icns")), [16, 32, 128, 256]);
    }

    #[test]
    fn an_old_icon_file_still_lists_its_sizes_but_has_no_image_to_draw() {
        assert_eq!(sides(&fixture("legacy-only.icns")), [16, 32]);
        assert_eq!(largest_image(&fixture("legacy-only.icns")), None);
    }

    #[test]
    fn the_largest_image_is_extracted() {
        let image = largest_image(&fixture("modern.icns")).expect("an image");

        let size = imagesize::blob_size(&image).expect("a png");
        assert_eq!((size.width, size.height), (256, 256));
    }

    #[test]
    fn other_files_are_not_icon_files() {
        assert!(is_icns_file(&fixture("modern.icns")));
        assert!(!is_icns_file(&fixture("red-dot.png")));
        assert!(!is_icns_file(&fixture("missing.icns")));
        assert_eq!(
            embedded_sizes(&fixture("red-dot.png")),
            Err(InspectError::Unsupported)
        );
    }

    #[test]
    fn never_panics_on_truncated_or_damaged_files() {
        let valid = std::fs::read(fixture("modern.icns")).expect("fixture");
        let folder = std::env::temp_dir().join(format!("pigoune-icns-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("folder");
        let path = folder.join("damaged.icns");
        for length in (0..valid.len()).step_by(7) {
            std::fs::write(&path, &valid[..length]).expect("file");
            let _ = embedded_sizes(&path);
            let _ = largest_image(&path);
        }
        for position in (0..300).step_by(3) {
            for value in [0, 1, 0x7f, 0xff] {
                let mut damaged = valid.clone();
                damaged[position] = value;
                std::fs::write(&path, &damaged).expect("file");
                let _ = embedded_sizes(&path);
                let _ = largest_image(&path);
            }
        }
        std::fs::remove_dir_all(&folder).expect("folder removed");
    }
}
