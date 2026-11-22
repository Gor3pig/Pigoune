use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::{AssetFormat, Dimensions, InspectError, MediaInfo};

const HEADER_LENGTH: usize = 18;
const FOOTER_SIGNATURE: &[u8] = b"TRUEVISION-XFILE.\0";
const FOOTER_LENGTH: u64 = 26;
const FOOTER_BYTES: usize = 26;
const EXTENSIONS: [&str; 4] = ["tga", "icb", "vda", "vst"];
const COLOR_MAPPED: u8 = 1;
const TRUE_COLOR: u8 = 2;
const GRAY: u8 = 3;
const RUN_LENGTH_FLAG: u8 = 8;
const RESERVED_DESCRIPTOR_BITS: u8 = 0xC0;
const MOST_ALPHA_BITS: u8 = 8;

struct Header {
    id_length: u64,
    color_map_type: u8,
    image_type: u8,
    map_first: u16,
    map_length: u16,
    map_entry_bits: u8,
    width: u16,
    height: u16,
    depth: u8,
    descriptor: u8,
}

impl Header {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.get(..HEADER_LENGTH)?;
        Some(Self {
            id_length: u64::from(bytes[0]),
            color_map_type: bytes[1],
            image_type: bytes[2],
            map_first: u16::from_le_bytes([bytes[3], bytes[4]]),
            map_length: u16::from_le_bytes([bytes[5], bytes[6]]),
            map_entry_bits: bytes[7],
            width: u16::from_le_bytes([bytes[12], bytes[13]]),
            height: u16::from_le_bytes([bytes[14], bytes[15]]),
            depth: bytes[16],
            descriptor: bytes[17],
        })
    }

    fn base_type(&self) -> u8 {
        self.image_type & !RUN_LENGTH_FLAG
    }

    fn is_run_length_encoded(&self) -> bool {
        self.image_type & RUN_LENGTH_FLAG != 0
    }

    fn is_plausible(&self) -> bool {
        let known_type = matches!(self.image_type, 1..=3 | 9..=11);
        let sides = self.width > 0 && self.height > 0;
        let descriptor = self.descriptor & RESERVED_DESCRIPTOR_BITS == 0
            && self.descriptor & 0x0F <= MOST_ALPHA_BITS;
        known_type
            && sides
            && descriptor
            && self.has_matching_color_map()
            && self.has_matching_depth()
    }

    fn has_matching_color_map(&self) -> bool {
        if self.base_type() == COLOR_MAPPED {
            self.color_map_type == 1
                && self.map_length > 0
                && matches!(self.map_entry_bits, 15 | 16 | 24 | 32)
        } else {
            self.color_map_type == 0
                && self.map_first == 0
                && self.map_length == 0
                && self.map_entry_bits == 0
        }
    }

    fn has_matching_depth(&self) -> bool {
        match self.base_type() {
            TRUE_COLOR => matches!(self.depth, 15 | 16 | 24 | 32),
            COLOR_MAPPED | GRAY => matches!(self.depth, 8 | 16),
            _ => false,
        }
    }

    fn smallest_file(&self) -> u64 {
        let map_bytes = u64::from(self.map_length) * u64::from(self.map_entry_bits).div_ceil(8);
        let before_pixels = HEADER_LENGTH as u64 + self.id_length + map_bytes;
        if self.is_run_length_encoded() {
            return before_pixels + 2;
        }
        let pixels = u64::from(self.width) * u64::from(self.height);
        before_pixels + pixels * u64::from(self.depth).div_ceil(8)
    }
}

#[must_use]
pub fn is_tga_file(path: &Path, header: &[u8]) -> bool {
    let Some(header) = Header::parse(header) else {
        return false;
    };
    if !header.is_plausible() {
        return false;
    }
    let Ok(file) = File::open(path) else {
        return false;
    };
    let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
        return false;
    };
    has_footer(file, length) || (has_tga_extension(path) && length >= header.smallest_file())
}

pub fn inspect(path: &Path) -> Result<MediaInfo, InspectError> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(HEADER_LENGTH as u64).read_to_end(&mut bytes))
        .map_err(|_| InspectError::Unreadable)?;
    let header = Header::parse(&bytes).ok_or(InspectError::Unreadable)?;
    Ok(MediaInfo {
        format: AssetFormat::Tga,
        dimensions: Some(
            Dimensions::new(u32::from(header.width), u32::from(header.height))
                .ok_or(InspectError::Unreadable)?,
        ),
        is_animated: false,
        embedded_sizes: Vec::new(),
    })
}

fn has_footer(mut file: File, length: u64) -> bool {
    if length < FOOTER_LENGTH + HEADER_LENGTH as u64 {
        return false;
    }
    let mut footer = [0_u8; FOOTER_BYTES];
    file.seek(SeekFrom::Start(length - FOOTER_LENGTH))
        .and_then(|_| file.read_exact(&mut footer))
        .is_ok()
        && footer.ends_with(FOOTER_SIGNATURE)
}

fn has_tga_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{Header, inspect, is_tga_file};
    use crate::media::{AssetFormat, Dimensions};

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn header_of(path: &Path) -> Vec<u8> {
        std::fs::read(path).expect("the fixture exists")
    }

    fn recognized(path: &Path) -> bool {
        is_tga_file(path, &header_of(path))
    }

    struct Folder(PathBuf);

    impl Folder {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("pigoune-tga-{label}-{}", std::process::id()));
            std::fs::create_dir_all(&path).expect("folder is created");
            Self(path)
        }

        fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, bytes).expect("file is written");
            path
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn recognizes_files_written_by_another_program() {
        for name in [
            "texture-uncompressed.tga",
            "texture-rle.tga",
            "texture-gray.tga",
        ] {
            let path = fixture(name);

            assert!(recognized(&path), "{name}");
            let info = inspect(&path).expect(name);
            assert_eq!(info.format, AssetFormat::Tga, "{name}");
            assert_eq!(info.dimensions, Dimensions::new(16, 8), "{name}");
        }
    }

    #[test]
    fn a_file_without_the_extension_or_the_footer_is_not_recognized() {
        let folder = Folder::new("extension");
        let bytes = header_of(&fixture("texture-uncompressed.tga"));
        let renamed = folder.write("picture.bin", &bytes);

        assert!(!is_tga_file(&renamed, &bytes));
    }

    #[test]
    fn the_footer_of_version_two_files_is_enough_without_the_extension() {
        let folder = Folder::new("footer");
        let mut bytes = header_of(&fixture("texture-uncompressed.tga"));
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(b"TRUEVISION-XFILE.\0");
        let path = folder.write("picture.bin", &bytes);

        assert!(is_tga_file(&path, &bytes));
    }

    #[test]
    fn a_file_that_is_too_short_for_its_pixels_is_not_recognized() {
        let folder = Folder::new("short");
        let bytes = header_of(&fixture("texture-uncompressed.tga"));
        let short = folder.write("short.tga", &bytes[..100]);

        assert!(!is_tga_file(&short, &bytes[..100]));
    }

    #[test]
    fn other_files_with_a_tga_name_are_not_recognized() {
        let folder = Folder::new("others");
        for name in [
            "red-dot.png",
            "blue-photo.jpg",
            "still.gif",
            "navy-tile.bmp",
            "gradient.kra",
            "modern.icns",
            "rle-gradient.psd",
            "plain.svgz",
            "teal-column.tiff",
        ] {
            let bytes = header_of(&fixture(name));
            let path = folder.write("fake.tga", &bytes);

            assert!(!is_tga_file(&path, &bytes), "{name}");
        }
    }

    #[test]
    fn random_bytes_are_never_recognized() {
        let folder = Folder::new("random");
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        for _ in 0..2000 {
            let mut bytes = Vec::new();
            for _ in 0..64 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                bytes.push(state.to_le_bytes()[0]);
            }
            let path = folder.write("noise.tga", &bytes);

            assert!(!is_tga_file(&path, &bytes), "{bytes:?}");
        }
    }

    #[test]
    fn the_header_of_text_is_not_plausible() {
        assert!(
            Header::parse(b"hello world, this is plain text")
                .is_some_and(|header| !header.is_plausible())
        );
        assert!(Header::parse(b"short").is_none());
    }
}
