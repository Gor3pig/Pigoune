use std::fs::File;
use std::io::Read;
use std::path::Path;

use miniz_oxide::inflate::core::{DecompressorOxide, decompress, inflate_flags};

use super::archive::crc32;
use super::{AssetFormat, InspectError, MediaInfo, svg};

const GZIP_MAGIC: [u8; 3] = [0x1F, 0x8B, 8];
const GZIP_FIXED_LENGTH: usize = 10;
const GZIP_TRAILER_LENGTH: usize = 8;
const HEADER_CRC_FLAG: u8 = 2;
const EXTRA_FLAG: u8 = 4;
const NAME_FLAG: u8 = 8;
const COMMENT_FLAG: u8 = 16;
const LARGEST_FILE: u64 = 16 * 1024 * 1024;
const SNIFFED_BYTES: usize = 512;

#[must_use]
pub fn is_compressed_svg(header: &[u8]) -> bool {
    if !header.starts_with(&GZIP_MAGIC) {
        return false;
    }
    let Some(start) = deflate_start(header) else {
        return false;
    };
    let mut out = [0_u8; SNIFFED_BYTES];
    let flags = inflate_flags::TINFL_FLAG_HAS_MORE_INPUT
        | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    let (_, _, produced) = decompress(
        &mut DecompressorOxide::new(),
        &header[start..],
        &mut out,
        0,
        flags,
    );
    super::format::looks_like_markup(&out[..produced])
}

pub fn inspect(path: &Path) -> Result<MediaInfo, InspectError> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(LARGEST_FILE + 1).read_to_end(&mut bytes))
        .map_err(|_| InspectError::Unreadable)?;
    if bytes.len() as u64 > LARGEST_FILE {
        return Err(InspectError::Unreadable);
    }
    let svg = gunzip(&bytes, usize::try_from(LARGEST_FILE).unwrap_or(usize::MAX))
        .ok_or(InspectError::Unreadable)?;
    svg::inspect_bytes(&svg, AssetFormat::Svgz)
}

fn gunzip(bytes: &[u8], limit: usize) -> Option<Vec<u8>> {
    let start = deflate_start(bytes)?;
    let end = bytes.len().checked_sub(GZIP_TRAILER_LENGTH)?;
    let data =
        miniz_oxide::inflate::decompress_to_vec_with_limit(bytes.get(start..end)?, limit).ok()?;
    let trailer = bytes.get(end..)?;
    let crc = u32::from_le_bytes([trailer[0], trailer[1], trailer[2], trailer[3]]);
    let size = u32::from_le_bytes([trailer[4], trailer[5], trailer[6], trailer[7]]);
    let length_matches = u32::try_from(data.len() % (1 << 32)).ok()? == size;
    (crc32(&data) == crc && length_matches).then_some(data)
}

fn deflate_start(bytes: &[u8]) -> Option<usize> {
    if !bytes.starts_with(&GZIP_MAGIC) {
        return None;
    }
    let flags = *bytes.get(3)?;
    let mut position = GZIP_FIXED_LENGTH;
    if flags & EXTRA_FLAG != 0 {
        let length = usize::from(u16::from_le_bytes([
            *bytes.get(position)?,
            *bytes.get(position + 1)?,
        ]));
        position += 2 + length;
    }
    for flag in [NAME_FLAG, COMMENT_FLAG] {
        if flags & flag != 0 {
            let terminator = bytes.get(position..)?.iter().position(|byte| *byte == 0)?;
            position += terminator + 1;
        }
    }
    if flags & HEADER_CRC_FLAG != 0 {
        position += 2;
    }
    (position <= bytes.len()).then_some(position)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{gunzip, inspect, is_compressed_svg};
    use crate::media::{AssetFormat, Dimensions, InspectError};

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn bytes(name: &str) -> Vec<u8> {
        std::fs::read(fixture(name)).expect("the fixture exists")
    }

    #[test]
    fn recognizes_a_compressed_svg_and_nothing_else() {
        assert!(is_compressed_svg(&bytes("plain.svgz")));
        assert!(is_compressed_svg(&bytes("named.svgz")));
        assert!(!is_compressed_svg(&bytes("not-svg.gz")));
        assert!(!is_compressed_svg(&bytes("red-dot.png")));
        assert!(!is_compressed_svg(&[0x1F, 0x8B, 8]));
        assert!(!is_compressed_svg(&[]));
    }

    #[test]
    fn reads_the_size_of_a_compressed_svg() {
        for name in ["plain.svgz", "named.svgz"] {
            let info = inspect(&fixture(name)).expect(name);

            assert_eq!(info.format, AssetFormat::Svgz, "{name}");
            assert_eq!(info.dimensions, Dimensions::new(48, 24), "{name}");
        }
    }

    #[test]
    fn a_compressed_file_that_is_not_an_svg_is_refused() {
        assert_eq!(
            inspect(&fixture("not-svg.gz")),
            Err(InspectError::Unsupported)
        );
    }

    #[test]
    fn a_damaged_stream_is_refused_and_never_panics() {
        let valid = bytes("plain.svgz");
        let limit = 1 << 20;
        assert!(gunzip(&valid, limit).is_some());
        assert!(gunzip(&valid, 10).is_none());
        for length in 0..valid.len() {
            let _ = gunzip(&valid[..length], limit);
            let _ = is_compressed_svg(&valid[..length]);
        }
        for position in 0..valid.len() {
            for value in [0, 1, 0x7f, 0x80, 0xff] {
                let mut damaged = valid.clone();
                damaged[position] = value;
                let _ = gunzip(&damaged, limit);
                let _ = is_compressed_svg(&damaged);
            }
        }
    }
}
