use std::{
    error::Error,
    fmt,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
};

use gio::ReadInputStream;
use glycin::{Error as GlycinError, ErrorCtx, Loader, SandboxMechanism};
use pigoune_core::{
    ContainerCodec, ImageFormat, ImageMetadata, StagedObject, StagedValidator, icns as icns_core,
    ico,
};

use super::{ImportWarning, ValidatedImport, svg};

#[derive(Debug)]
pub enum ImportValidationError {
    StagingAccess(io::Error),
    UnsupportedFormat(Box<ErrorCtx>),
    Load(Box<ErrorCtx>),
    Frame(Box<ErrorCtx>),
    InvalidDimensions {
        width: u32,
        height: u32,
    },
    MimeNotAccepted(String),
    SvgAnalysis(svg::SvgAnalysisError),
    Ico(ico::IcoError),
    Icns(icns_core::IcnsError),
    LegacyDecode(io::Error),
    PrimaryLoad {
        codec: ContainerCodec,
        error: Box<ErrorCtx>,
    },
    PrimaryFrame {
        codec: ContainerCodec,
        error: Box<ErrorCtx>,
    },
    PrimaryMimeMismatch {
        codec: ContainerCodec,
        mime: String,
    },
    DecodedDimensionsMismatch {
        expected: (u32, u32),
        actual: (u32, u32),
    },
}

impl fmt::Display for ImportValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StagingAccess(error) => write!(f, "cannot read staged image: {error}"),
            Self::UnsupportedFormat(error) => write!(f, "unsupported image format: {error}"),
            Self::Load(error) => write!(f, "cannot load image: {error}"),
            Self::Frame(error) => write!(f, "cannot decode first frame: {error}"),
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid image dimensions: {width} × {height}")
            }
            Self::MimeNotAccepted(mime) => write!(f, "image MIME type is not accepted: {mime}"),
            Self::SvgAnalysis(error) => write!(f, "cannot analyze staged SVG: {error}"),
            Self::Ico(error) => write!(f, "invalid ICO container: {error}"),
            Self::Icns(error) => write!(f, "invalid ICNS container: {error}"),
            Self::LegacyDecode(error) => write!(f, "cannot decode ICNS legacy primary: {error}"),
            Self::PrimaryLoad { codec, error } => {
                write!(f, "cannot load {codec:?} primary: {error}")
            }
            Self::PrimaryFrame { codec, error } => {
                write!(f, "cannot decode {codec:?} primary: {error}")
            }
            Self::PrimaryMimeMismatch { codec, mime } => {
                write!(f, "{codec:?} primary has unexpected MIME type: {mime}")
            }
            Self::DecodedDimensionsMismatch { expected, actual } => write!(
                f,
                "icon primary decoded as {} × {}, expected {} × {}",
                actual.0, actual.1, expected.0, expected.1
            ),
        }
    }
}

impl Error for ImportValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::StagingAccess(error) => Some(error),
            Self::UnsupportedFormat(error) | Self::Load(error) | Self::Frame(error) => Some(error),
            Self::InvalidDimensions { .. } | Self::MimeNotAccepted(_) => None,
            Self::SvgAnalysis(error) => Some(error),
            Self::Ico(error) => Some(error),
            Self::Icns(error) => Some(error),
            Self::LegacyDecode(error) => Some(error),
            Self::PrimaryLoad { error, .. } | Self::PrimaryFrame { error, .. } => Some(error),
            Self::PrimaryMimeMismatch { .. } => None,
            Self::DecodedDimensionsMismatch { .. } => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct GlycinValidator;

impl StagedValidator for GlycinValidator {
    type Output = ValidatedImport;
    type Error = ImportValidationError;

    /// Call only from a background worker: this waits for Glycin and blocks its caller.
    fn validate(&self, staged: &StagedObject<'_>) -> Result<ValidatedImport, Self::Error> {
        let (validated, _) = validate_staged(staged)?;
        Ok(validated)
    }
}

fn validate_staged(
    staged: &StagedObject<'_>,
) -> Result<(ValidatedImport, SandboxMechanism), ImportValidationError> {
    let mut probe = staged
        .open_read()
        .map_err(ImportValidationError::StagingAccess)?;
    let mut magic = [0; 4];
    if probe
        .read(&mut magic)
        .map_err(ImportValidationError::StagingAccess)?
        == 4
        && &magic == b"icns"
    {
        return validate_icns(staged);
    }
    let file = staged
        .open_read()
        .map_err(ImportValidationError::StagingAccess)?;
    let loader = loader_from_reader(file);

    // Glycin's default async-io backend uses the same executor for its own blocking work.
    let (metadata, mechanism, container) = async_io::block_on(async move {
        let image = loader.load().await.map_err(|error| {
            if is_unsupported(&error) {
                ImportValidationError::UnsupportedFormat(Box::new(error))
            } else {
                ImportValidationError::Load(Box::new(error))
            }
        })?;
        let mime_type = image.mime_type().as_str().to_owned();
        let format = format_from_mime(&mime_type)
            .ok_or(ImportValidationError::MimeNotAccepted(mime_type))?;

        if format == ImageFormat::Ico {
            let mut staged_file = staged
                .open_read()
                .map_err(ImportValidationError::StagingAccess)?;
            let parsed = ico::parse_ico(&mut staged_file).map_err(ImportValidationError::Ico)?;
            let primary = parsed.metadata().primary();
            let expected = (primary.width(), primary.height());
            let codec = primary.codec();
            let chosen = parsed.primary();
            let payload = StagedSlice::new(staged_file, chosen.payload_offset, chosen.payload_size)
                .map_err(ImportValidationError::StagingAccess)?;
            let selected_loader = match codec {
                ContainerCodec::Png => loader_from_reader(payload),
                ContainerCodec::Dib => loader_from_reader(
                    SingleEntryIco::new(payload, chosen.directory_entry)
                        .map_err(ImportValidationError::StagingAccess)?,
                ),
                _ => unreachable!("ICO parser returns only PNG or DIB"),
            };
            drop(image);
            let selected = selected_loader.load().await.map_err(|error| {
                ImportValidationError::PrimaryLoad {
                    codec,
                    error: Box::new(error),
                }
            })?;
            let expected_format = match codec {
                ContainerCodec::Png => ImageFormat::Png,
                ContainerCodec::Dib => ImageFormat::Ico,
                _ => unreachable!("ICO parser returns only PNG or DIB"),
            };
            let mime_type = selected.mime_type();
            let selected_mime = mime_type.as_str();
            if format_from_mime(selected_mime) != Some(expected_format) {
                return Err(ImportValidationError::PrimaryMimeMismatch {
                    codec,
                    mime: selected_mime.to_owned(),
                });
            }
            let frame = selected.next_frame().await.map_err(|error| {
                ImportValidationError::PrimaryFrame {
                    codec,
                    error: Box::new(error),
                }
            })?;
            let actual = (selected.details().width(), selected.details().height());
            if actual != expected || (frame.width(), frame.height()) != expected {
                return Err(ImportValidationError::DecodedDimensionsMismatch { expected, actual });
            }
            let metadata = ImageMetadata::new(ImageFormat::Ico, expected.0, expected.1, false)
                .map_err(|_| ImportValidationError::InvalidDimensions {
                    width: expected.0,
                    height: expected.1,
                })?;
            return Ok((
                metadata,
                selected.active_sandbox_mechanism(),
                Some(parsed.metadata().clone()),
            ));
        }

        let frame = image
            .next_frame()
            .await
            .map_err(|error| ImportValidationError::Frame(Box::new(error)))?;
        let details = image.details();
        let (width, height) = (details.width(), details.height());
        if width == 0 || height == 0 || frame.width() == 0 || frame.height() == 0 {
            return Err(ImportValidationError::InvalidDimensions { width, height });
        }

        let metadata = ImageMetadata::new(format, width, height, frame.delay().is_some())
            .map_err(|_| ImportValidationError::InvalidDimensions { width, height })?;
        Ok((metadata, image.active_sandbox_mechanism(), None))
    })?;
    let warnings = if metadata.format() == ImageFormat::Svg {
        if svg::has_external_references(staged).map_err(ImportValidationError::SvgAnalysis)? {
            vec![ImportWarning::SvgExternalReferences]
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    Ok((
        ValidatedImport {
            metadata,
            warnings,
            container,
        },
        mechanism,
    ))
}

fn validate_icns(
    staged: &StagedObject<'_>,
) -> Result<(ValidatedImport, SandboxMechanism), ImportValidationError> {
    let mut file = staged
        .open_read()
        .map_err(ImportValidationError::StagingAccess)?;
    let parsed = icns_core::parse_icns(&mut file).map_err(ImportValidationError::Icns)?;
    let primary = parsed.metadata().primary();
    let expected = (primary.width(), primary.height());
    let chosen = parsed.primary();
    let codec = primary.codec();
    let mechanism = match codec {
        ContainerCodec::Png | ContainerCodec::Jpeg2000 => {
            let payload = StagedSlice::new(file, chosen.color.offset, chosen.color.size)
                .map_err(ImportValidationError::StagingAccess)?;
            async_io::block_on(async move {
                let selected = loader_from_reader(payload).load().await.map_err(|error| {
                    ImportValidationError::PrimaryLoad {
                        codec,
                        error: Box::new(error),
                    }
                })?;
                let mime = selected.mime_type();
                let expected_mime = match (codec, chosen.jpeg2000_kind) {
                    (ContainerCodec::Png, _) => "image/png",
                    (ContainerCodec::Jpeg2000, Some(icns_core::Jpeg2000Kind::Jp2)) => "image/jp2",
                    (ContainerCodec::Jpeg2000, Some(icns_core::Jpeg2000Kind::Codestream)) => {
                        "image/x-jp2-codestream"
                    }
                    _ => unreachable!("parser provides JPEG 2000 kind"),
                };
                if mime.as_str() != expected_mime {
                    return Err(ImportValidationError::PrimaryMimeMismatch {
                        codec,
                        mime: mime.as_str().to_owned(),
                    });
                }
                let frame = selected.next_frame().await.map_err(|error| {
                    ImportValidationError::PrimaryFrame {
                        codec,
                        error: Box::new(error),
                    }
                })?;
                let actual = (selected.details().width(), selected.details().height());
                if actual != expected || (frame.width(), frame.height()) != expected {
                    return Err(ImportValidationError::DecodedDimensionsMismatch {
                        expected,
                        actual,
                    });
                }
                Ok(selected.active_sandbox_mechanism())
            })?
        }
        ContainerCodec::IcnsRgb | ContainerCodec::IcnsArgb => {
            let mut color = StagedSlice::new(file, chosen.color.offset, chosen.color.size)
                .map_err(ImportValidationError::StagingAccess)?;
            let mut data = Vec::new();
            color
                .read_to_end(&mut data)
                .map_err(ImportValidationError::StagingAccess)?;
            if chosen.add_rle_prefix {
                let mut prefixed =
                    Vec::with_capacity(data.len().checked_add(4).ok_or_else(|| {
                        ImportValidationError::StagingAccess(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "RLE prefix size overflow",
                        ))
                    })?);
                prefixed.extend_from_slice(&[0; 4]);
                prefixed.extend_from_slice(&data);
                data = prefixed;
            }
            let color_element = icns::IconElement::new(icns::OSType(chosen.color.kind), data);
            let image = if let Some(mask) = chosen.mask {
                let mut mask_slice = StagedSlice::new(
                    staged
                        .open_read()
                        .map_err(ImportValidationError::StagingAccess)?,
                    mask.offset,
                    mask.size,
                )
                .map_err(ImportValidationError::StagingAccess)?;
                let mut mask_data = Vec::new();
                mask_slice
                    .read_to_end(&mut mask_data)
                    .map_err(ImportValidationError::StagingAccess)?;
                let mask_element = icns::IconElement::new(icns::OSType(mask.kind), mask_data);
                color_element
                    .decode_image_with_mask(&mask_element)
                    .map_err(ImportValidationError::LegacyDecode)?
            } else {
                color_element
                    .decode_image()
                    .map_err(ImportValidationError::LegacyDecode)?
            };
            let actual = (image.width(), image.height());
            if actual != expected {
                return Err(ImportValidationError::DecodedDimensionsMismatch { expected, actual });
            }
            SandboxMechanism::NotSandboxed
        }
        ContainerCodec::Dib => unreachable!("ICNS parser never returns DIB"),
    };
    let metadata =
        ImageMetadata::new(ImageFormat::Icns, expected.0, expected.1, false).map_err(|_| {
            ImportValidationError::InvalidDimensions {
                width: expected.0,
                height: expected.1,
            }
        })?;
    let warnings = if parsed.unknown_count() > 0 {
        vec![ImportWarning::IcnsUnknownElements {
            count: parsed.unknown_count(),
        }]
    } else {
        Vec::new()
    };
    Ok((
        ValidatedImport {
            metadata,
            warnings,
            container: Some(parsed.metadata().clone()),
        },
        mechanism,
    ))
}

struct StagedSlice {
    staged: File,
    offset: u64,
    length: u64,
    position: u64,
}

impl StagedSlice {
    fn new(staged: File, offset: u64, length: u64) -> io::Result<Self> {
        let end = offset
            .checked_add(length)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "staged slice overflow"))?;
        if end > staged.metadata()?.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "staged slice exceeds file",
            ));
        }
        Ok(Self {
            staged,
            offset,
            length,
            position: 0,
        })
    }
}

impl Read for StagedSlice {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let available = usize::try_from(self.length - self.position).unwrap_or(usize::MAX);
        let count = output.len().min(available);
        if count == 0 {
            return Ok(0);
        }
        let offset = self.offset.checked_add(self.position).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "staged slice offset overflow")
        })?;
        self.staged.seek(SeekFrom::Start(offset))?;
        let count = self.staged.read(&mut output[..count])?;
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for StagedSlice {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.position = logical_seek(self.position, self.length, from)?;
        Ok(self.position)
    }
}

struct SingleEntryIco {
    header: [u8; 22],
    payload: StagedSlice,
    length: u64,
    position: u64,
}

impl SingleEntryIco {
    fn new(payload: StagedSlice, directory_entry: [u8; 16]) -> io::Result<Self> {
        let length = payload
            .length
            .checked_add(22)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "ICO view size overflow"))?;
        let mut header = [0_u8; 22];
        header[..6].copy_from_slice(&[0, 0, 1, 0, 1, 0]);
        header[6..].copy_from_slice(&directory_entry);
        Ok(Self {
            header,
            payload,
            length,
            position: 0,
        })
    }
}

impl Read for SingleEntryIco {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() || self.position >= self.length {
            return Ok(0);
        }
        if self.position < 22 {
            let start = self.position as usize;
            let count = output.len().min(22 - start);
            output[..count].copy_from_slice(&self.header[start..start + count]);
            self.position += count as u64;
            return Ok(count);
        }
        self.payload.seek(SeekFrom::Start(self.position - 22))?;
        let count = self.payload.read(output)?;
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for SingleEntryIco {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.position = logical_seek(self.position, self.length, from)?;
        Ok(self.position)
    }
}

fn logical_seek(position: u64, length: u64, from: SeekFrom) -> io::Result<u64> {
    let target = match from {
        SeekFrom::Start(value) => i128::from(value),
        SeekFrom::Current(delta) => i128::from(position) + i128::from(delta),
        SeekFrom::End(delta) => i128::from(length) + i128::from(delta),
    };
    let target = u64::try_from(target)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "seek before stream start"))?;
    if target > length {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "seek past stream end",
        ));
    }
    Ok(target)
}

fn loader_from_reader(reader: impl Read + Seek + Send + 'static) -> Loader {
    let stream = ReadInputStream::new_seekable(reader);
    // SAFETY: Glycin takes ownership of this stream. No reference or clone is kept,
    // and it is never accessed again after transfer to the loader.
    unsafe { Loader::new_stream(stream) }
}

fn is_unsupported(error: &ErrorCtx) -> bool {
    error.unsupported_format().is_some()
        || matches!(error.error(), GlycinError::UnknownContentType(_))
}

fn format_from_mime(mime: &str) -> Option<ImageFormat> {
    match mime {
        "image/png" | "image/apng" => Some(ImageFormat::Png),
        "image/jpeg" => Some(ImageFormat::Jpeg),
        "image/svg+xml" | "image/svg+xml-compressed" => Some(ImageFormat::Svg),
        "image/vnd.microsoft.icon" | "image/x-win-bitmap" => Some(ImageFormat::Ico),
        "image/webp" => Some(ImageFormat::WebP),
        "image/avif" => Some(ImageFormat::Avif),
        "image/gif" => Some(ImageFormat::Gif),
        "image/bmp" => Some(ImageFormat::Bmp),
        "image/tiff" => Some(ImageFormat::Tiff),
        "image/x-xpixmap" => Some(ImageFormat::Xpm),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pigoune_core::{AssetId, AssetRecord, Library, ObjectHash, ObjectStore, OriginalFilename};
    use std::{ffi::OsStr, fs};
    use tempfile::tempdir;

    const PNG: &[u8] = include_bytes!("../../tests/fixtures/sample.png");
    const JPEG: &[u8] = include_bytes!("../../tests/fixtures/sample.jpg");
    const WEBP: &[u8] = include_bytes!("../../tests/fixtures/sample.webp");
    const AVIF: &[u8] = include_bytes!("../../tests/fixtures/sample.avif");
    const BMP: &[u8] = include_bytes!("../../tests/fixtures/sample.bmp");
    const TIFF: &[u8] = include_bytes!("../../tests/fixtures/sample.tiff");
    const XPM: &[u8] = include_bytes!("../../tests/fixtures/sample.xpm");
    const ICO: &[u8] = include_bytes!("../../tests/fixtures/sample.ico");
    const GIF: &[u8] = include_bytes!("../../tests/fixtures/animated.gif");
    const APNG: &[u8] = include_bytes!("../../tests/fixtures/animated.apng");
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="3" height="2"><rect width="3" height="2" fill="#315b8f"/></svg>"##;
    const SVG_EXTERNAL: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="3" height="2"><rect width="3" height="2" fill="#315b8f"/><image href="missing.png" width="1" height="1"/></svg>"##;

    fn dib(width: u32, height: u32) -> Vec<u8> {
        let mut bytes =
            vec![0; (40 + width * height * 4 + (width.div_ceil(32) * 4) * height) as usize];
        bytes[..4].copy_from_slice(&40_u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&(width as i32).to_le_bytes());
        bytes[8..12].copy_from_slice(&((height * 2) as i32).to_le_bytes());
        bytes[12..14].copy_from_slice(&1_u16.to_le_bytes());
        bytes[14..16].copy_from_slice(&32_u16.to_le_bytes());
        for pixel in bytes[40..(40 + width * height * 4) as usize]
            .as_chunks_mut::<4>()
            .0
        {
            pixel.copy_from_slice(&[49, 91, 143, 255]);
        }
        bytes
    }

    fn multi_ico(items: &[(u8, u8, &[u8])]) -> Vec<u8> {
        let mut bytes = vec![0; 6 + 16 * items.len()];
        bytes[2..4].copy_from_slice(&1_u16.to_le_bytes());
        bytes[4..6].copy_from_slice(&(items.len() as u16).to_le_bytes());
        for (index, &(width, height, payload)) in items.iter().enumerate() {
            let position = 6 + 16 * index;
            bytes[position] = width;
            bytes[position + 1] = height;
            bytes[position + 4..position + 6].copy_from_slice(&1_u16.to_le_bytes());
            bytes[position + 6..position + 8].copy_from_slice(&32_u16.to_le_bytes());
            bytes[position + 8..position + 12]
                .copy_from_slice(&(payload.len() as u32).to_le_bytes());
            let offset = bytes.len() as u32;
            bytes[position + 12..position + 16].copy_from_slice(&offset.to_le_bytes());
            bytes.extend_from_slice(payload);
        }
        bytes
    }

    fn png_with_color_type(width: u32, height: u32, color_type: u8) -> Vec<u8> {
        fn crc32(bytes: &[u8]) -> u32 {
            let mut crc = !0_u32;
            for &byte in bytes {
                crc ^= u32::from(byte);
                for _ in 0..8 {
                    crc = if crc & 1 != 0 {
                        (crc >> 1) ^ 0xedb8_8320
                    } else {
                        crc >> 1
                    };
                }
            }
            !crc
        }
        fn chunk(kind: &[u8; 4], data: &[u8], output: &mut Vec<u8>) {
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let start = output.len();
            output.extend_from_slice(kind);
            output.extend_from_slice(data);
            output.extend_from_slice(&crc32(&output[start..]).to_be_bytes());
        }
        let mut raw = Vec::new();
        for _ in 0..height {
            raw.extend_from_slice(&[0]);
            for _ in 0..width {
                match color_type {
                    0 => raw.extend_from_slice(&[143]),
                    2 => raw.extend_from_slice(&[49, 91, 143]),
                    6 => raw.extend_from_slice(&[49, 91, 143, 255]),
                    _ => unreachable!("test PNG color type"),
                }
            }
        }
        let len = u16::try_from(raw.len()).unwrap();
        let mut zlib = vec![0x78, 0x01, 0x01];
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(&raw);
        let mut s1 = 1_u32;
        let mut s2 = 0_u32;
        for byte in raw {
            s1 = (s1 + u32::from(byte)) % 65_521;
            s2 = (s2 + s1) % 65_521;
        }
        zlib.extend_from_slice(&((s2 << 16) | s1).to_be_bytes());
        let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let mut ihdr = [0_u8; 13];
        ihdr[..4].copy_from_slice(&width.to_be_bytes());
        ihdr[4..8].copy_from_slice(&height.to_be_bytes());
        ihdr[8] = 8;
        ihdr[9] = color_type;
        chunk(b"IHDR", &ihdr, &mut png);
        chunk(b"IDAT", &zlib, &mut png);
        chunk(b"IEND", &[], &mut png);
        png
    }

    #[test]
    fn staged_slice_restricts_reads_and_seeks() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("staged");
        fs::write(&path, b"beforePAYLOADafter").unwrap();
        let mut slice = StagedSlice::new(File::open(&path).unwrap(), 6, 7).unwrap();
        let mut bytes = Vec::new();
        slice.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"PAYLOAD");
        assert_eq!(slice.read(&mut [0; 4]).unwrap(), 0);
        assert_eq!(slice.seek(SeekFrom::End(-3)).unwrap(), 4);
        let mut tail = [0; 3];
        slice.read_exact(&mut tail).unwrap();
        assert_eq!(&tail, b"OAD");
        assert_eq!(slice.seek(SeekFrom::Start(0)).unwrap(), 0);
        assert!(slice.seek(SeekFrom::Current(-1)).is_err());
        assert!(slice.seek(SeekFrom::Start(8)).is_err());
        assert!(slice.seek(SeekFrom::End(1)).is_err());
        assert!(StagedSlice::new(File::open(&path).unwrap(), u64::MAX, 1).is_err());
        assert!(StagedSlice::new(File::open(&path).unwrap(), 6, 13).is_err());
    }

    #[test]
    fn single_entry_ico_reads_only_its_bounded_payload() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("staged");
        fs::write(&path, b"beforePAYLOADafter").unwrap();
        let payload = StagedSlice::new(File::open(&path).unwrap(), 6, 7).unwrap();
        let mut ico = SingleEntryIco::new(payload, [0; 16]).unwrap();
        let mut bytes = Vec::new();
        ico.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes.len(), 29);
        assert_eq!(&bytes[..6], &[0, 0, 1, 0, 1, 0]);
        assert_eq!(&bytes[22..], b"PAYLOAD");
        assert!(ico.seek(SeekFrom::End(1)).is_err());
    }

    #[test]
    fn accepted_mime_aliases_have_explicit_domain_formats() {
        for (mime, format) in [
            ("image/png", ImageFormat::Png),
            ("image/apng", ImageFormat::Png),
            ("image/jpeg", ImageFormat::Jpeg),
            ("image/svg+xml", ImageFormat::Svg),
            ("image/svg+xml-compressed", ImageFormat::Svg),
            ("image/vnd.microsoft.icon", ImageFormat::Ico),
            ("image/x-win-bitmap", ImageFormat::Ico),
            ("image/webp", ImageFormat::WebP),
            ("image/avif", ImageFormat::Avif),
            ("image/gif", ImageFormat::Gif),
            ("image/bmp", ImageFormat::Bmp),
            ("image/tiff", ImageFormat::Tiff),
            ("image/x-xpixmap", ImageFormat::Xpm),
        ] {
            assert_eq!(format_from_mime(mime), Some(format));
        }
        assert_eq!(format_from_mime("image/icns"), None);
        assert_eq!(format_from_mime("image/unknown"), None);
    }

    #[test]
    fn staged_glycin_image_persists_with_asset() {
        let directory = tempdir().unwrap();
        let mut library = Library::create(&directory.path().join("library")).unwrap();
        let validated = library
            .object_store
            .stage_reader(PNG, Some(OsStr::new("original.jpg")))
            .unwrap()
            .validate_with(&GlycinValidator)
            .unwrap();
        let metadata = validated.validation().metadata;
        assert!(validated.validation().warnings.is_empty());
        let published = validated.publish().unwrap();
        let object = published.stored.object;
        let asset = AssetRecord {
            id: AssetId::new(),
            object_hash: object.hash,
            original_filename: OriginalFilename::from_bytes(b"original.jpg".to_vec()).unwrap(),
            display_name: "Original".into(),
            imported_at_utc_us: 1_700_000_000_000_000,
        };
        library
            .database
            .import_published_asset(&object, metadata, None, &asset)
            .unwrap();
        let stored = library.database.get_object(object.hash).unwrap().unwrap();
        assert_eq!(object.hash, ObjectHash::from_bytes(PNG));
        assert_eq!(
            fs::read(directory.path().join("library").join(&object.relative_path)).unwrap(),
            PNG
        );
        assert_eq!(stored.object.size, PNG.len() as u64);
        assert_eq!(stored.metadata.unwrap().format(), ImageFormat::Png);
        assert_eq!(
            (
                stored.metadata.unwrap().width(),
                stored.metadata.unwrap().height()
            ),
            (3, 2)
        );
        assert!(!stored.metadata.unwrap().animated());
        let read_asset = library.database.get_asset(asset.id).unwrap().unwrap();
        assert_eq!(read_asset.id, asset.id);
        assert_eq!(read_asset.original_filename.as_bytes(), b"original.jpg");
        assert_eq!(read_asset, asset);
    }

    fn validate_and_publish(
        bytes: &[u8],
        name: Option<&str>,
        format: ImageFormat,
        size: (u32, u32),
    ) {
        let library = tempdir().expect("temporary library");
        let store = ObjectStore::new(library.path()).expect("object store");
        let staged = store
            .stage_reader(bytes, name.map(OsStr::new))
            .expect("stage bytes");
        let validated = staged
            .validate_with(&GlycinValidator)
            .expect("decode staged image");
        assert_eq!(validated.validation().metadata.format(), format);
        assert!(validated.validation().warnings.is_empty());
        assert_eq!(
            (
                validated.validation().metadata.width(),
                validated.validation().metadata.height()
            ),
            size
        );
        let published = validated.publish().expect("publish image");
        assert_eq!(published.stored.object.hash, ObjectHash::from_bytes(bytes));
        assert_eq!(
            fs::read(library.path().join(published.stored.object.relative_path))
                .expect("published bytes"),
            bytes
        );
    }

    #[test]
    fn png_and_jpeg_decode_and_publish_exact_bytes() {
        validate_and_publish(PNG, Some("sample.png"), ImageFormat::Png, (3, 2));
        validate_and_publish(JPEG, Some("sample.jpg"), ImageFormat::Jpeg, (3, 2));
    }

    #[test]
    fn filename_does_not_determine_format() {
        validate_and_publish(PNG, Some("misnamed.jpg"), ImageFormat::Png, (3, 2));
        validate_and_publish(PNG, None, ImageFormat::Png, (3, 2));
        validate_and_publish(PNG, Some("sample.unknown"), ImageFormat::Png, (3, 2));
    }

    #[test]
    fn invalid_data_is_not_published_and_staging_is_removed() {
        for (bytes, name) in [
            (b"this is text".as_slice(), "fake.png"),
            (&PNG[..20], "truncated.png"),
        ] {
            let library = tempdir().expect("temporary library");
            let store = ObjectStore::new(library.path()).expect("object store");
            let staged = store
                .stage_reader(bytes, Some(OsStr::new(name)))
                .expect("stage bytes");
            assert!(staged.validate_with(&GlycinValidator).is_err());
            assert_eq!(
                fs::read_dir(library.path().join("objects/.tmp"))
                    .expect("staging directory")
                    .count(),
                0
            );
            assert!(
                !store
                    .contains(ObjectHash::from_bytes(bytes))
                    .expect("store lookup")
            );
        }
    }

    #[test]
    fn additional_local_loaders_decode() {
        validate_and_publish(WEBP, Some("sample.webp"), ImageFormat::WebP, (3, 2));
        validate_and_publish(AVIF, Some("sample.avif"), ImageFormat::Avif, (4, 4));
        validate_and_publish(BMP, Some("sample.bmp"), ImageFormat::Bmp, (3, 2));
        validate_and_publish(TIFF, Some("sample.tiff"), ImageFormat::Tiff, (3, 2));
        validate_and_publish(XPM, Some("sample.xpm"), ImageFormat::Xpm, (3, 2));
        validate_and_publish(ICO, Some("sample.ico"), ImageFormat::Ico, (16, 16));
        validate_and_publish(SVG, Some("sample.svg"), ImageFormat::Svg, (3, 2));
    }

    fn assert_selected_png_is_imported(color_type: u8) {
        let smaller = dib(2, 2);
        let png = png_with_color_type(3, 2, color_type);
        let bytes = multi_ico(&[(2, 2, &smaller), (3, 2, &png)]);
        let library = tempdir().unwrap();
        let store = ObjectStore::new(library.path()).unwrap();
        let validated = store
            .stage_reader(bytes.as_slice(), Some(OsStr::new("multi.ico")))
            .unwrap()
            .validate_with(&GlycinValidator)
            .unwrap();
        assert_eq!(validated.validation().metadata.format(), ImageFormat::Ico);
        assert!(!validated.validation().metadata.animated());
        assert_eq!(
            (
                validated.validation().metadata.width(),
                validated.validation().metadata.height()
            ),
            (3, 2)
        );
        assert_eq!(
            validated
                .validation()
                .container
                .as_ref()
                .unwrap()
                .primary_ordinal(),
            1
        );
        assert_eq!(
            validated
                .validation()
                .container
                .as_ref()
                .unwrap()
                .primary()
                .codec(),
            ContainerCodec::Png
        );
        let published = validated.publish().unwrap();
        assert_eq!(
            fs::read(library.path().join(published.stored.object.relative_path)).unwrap(),
            bytes
        );
    }

    #[test]
    fn selected_rgb_png_is_decoded_directly_even_when_second() {
        assert_selected_png_is_imported(2);
    }

    #[test]
    fn selected_rgba_png_is_decoded_directly_even_when_second() {
        assert_selected_png_is_imported(6);
    }

    #[test]
    fn selected_grayscale_png_is_decoded_directly_even_when_second() {
        assert_selected_png_is_imported(0);
    }

    #[test]
    fn selected_dib_is_decoded_even_when_second() {
        let larger = dib(16, 16);
        let bytes = multi_ico(&[(3, 2, PNG), (16, 16, &larger)]);
        let library = tempdir().unwrap();
        let store = ObjectStore::new(library.path()).unwrap();
        let validated = store
            .stage_reader(bytes.as_slice(), Some(OsStr::new("multi.ico")))
            .unwrap()
            .validate_with(&GlycinValidator)
            .unwrap();
        assert_eq!(
            (
                validated.validation().metadata.width(),
                validated.validation().metadata.height()
            ),
            (16, 16)
        );
        assert_eq!(
            validated
                .validation()
                .container
                .as_ref()
                .unwrap()
                .primary_ordinal(),
            1
        );
        let published = validated.publish().unwrap();
        assert_eq!(
            fs::read(library.path().join(published.stored.object.relative_path)).unwrap(),
            bytes
        );
    }

    #[test]
    fn invalid_secondary_entry_blocks_publication() {
        let mut broken = dib(2, 2);
        broken.truncate(broken.len() - 8);
        let bytes = multi_ico(&[(3, 2, PNG), (2, 2, &broken)]);
        let library = tempdir().unwrap();
        let store = ObjectStore::new(library.path()).unwrap();
        let staged = store
            .stage_reader(bytes.as_slice(), Some(OsStr::new("bad.ico")))
            .unwrap();
        assert!(matches!(
            staged.validate_with(&GlycinValidator),
            Err(ImportValidationError::Ico(_))
        ));
        assert_eq!(
            fs::read_dir(library.path().join("objects/.tmp"))
                .unwrap()
                .count(),
            0
        );
        assert!(!store.contains(ObjectHash::from_bytes(&bytes)).unwrap());
    }

    #[test]
    fn first_frame_reports_animation() {
        for (bytes, name, format) in [
            (GIF, "animated.gif", ImageFormat::Gif),
            (APNG, "animated.apng", ImageFormat::Png),
        ] {
            let library = tempdir().expect("temporary library");
            let store = ObjectStore::new(library.path()).expect("object store");
            let staged = store
                .stage_reader(bytes, Some(OsStr::new(name)))
                .expect("stage animation");
            let validated = staged
                .validate_with(&GlycinValidator)
                .expect("decode animation");
            assert_eq!(validated.validation().metadata.format(), format);
            assert!(validated.validation().metadata.animated());
            assert!(validated.validation().warnings.is_empty());
        }
    }

    #[test]
    fn decodable_svg_with_external_resource_warns_and_preserves_bytes() {
        let library = tempdir().unwrap();
        let store = ObjectStore::new(library.path()).unwrap();
        let staged = store
            .stage_reader(SVG_EXTERNAL, Some(OsStr::new("external.svg")))
            .unwrap();
        let validated = staged.validate_with(&GlycinValidator).unwrap();
        assert_eq!(validated.validation().metadata.format(), ImageFormat::Svg);
        assert_eq!(
            (
                validated.validation().metadata.width(),
                validated.validation().metadata.height()
            ),
            (3, 2)
        );
        assert_eq!(
            validated.validation().warnings,
            vec![ImportWarning::SvgExternalReferences]
        );
        let published = validated.publish().unwrap();
        assert_eq!(
            published.stored.object.hash,
            ObjectHash::from_bytes(SVG_EXTERNAL)
        );
        assert_eq!(
            fs::read(library.path().join(published.stored.object.relative_path)).unwrap(),
            SVG_EXTERNAL
        );
    }

    #[test]
    fn invalid_svg_is_rejected_without_publication() {
        let bytes = b"<svg xmlns='http://www.w3.org/2000/svg' width='3' height='2'><rect width='3' height='2' fill='#315b8f' style='@import'/></svg>";
        let library = tempdir().unwrap();
        let store = ObjectStore::new(library.path()).unwrap();
        let staged = store
            .stage_reader(bytes.as_slice(), Some(OsStr::new("invalid.svg")))
            .unwrap();
        assert!(svg::has_external_references(&staged).is_err());
        assert!(matches!(
            staged.validate_with(&GlycinValidator),
            Err(ImportValidationError::SvgAnalysis(_))
        ));
        assert_eq!(
            fs::read_dir(library.path().join("objects/.tmp"))
                .unwrap()
                .count(),
            0
        );
        assert!(!store.contains(ObjectHash::from_bytes(bytes)).unwrap());
    }

    #[test]
    fn installed_flatpak_uses_a_loader_sandbox() {
        if std::env::var_os("PIGOUNE_CHECK_GLYCIN_SANDBOX").is_none() {
            return;
        }
        let library = tempdir().expect("temporary library");
        let store = ObjectStore::new(library.path()).expect("object store");
        let staged = store.stage_reader(PNG, None).expect("stage png");
        let (_, mechanism) = validate_staged(&staged).expect("decode png");
        assert_eq!(mechanism, SandboxMechanism::FlatpakSpawn);
    }

    fn icns_file(elements: &[([u8; 4], &[u8])]) -> Vec<u8> {
        let mut bytes = b"icns\0\0\0\0".to_vec();
        for (kind, payload) in elements {
            bytes.extend_from_slice(kind);
            bytes.extend_from_slice(&(payload.len() as u32 + 8).to_be_bytes());
            bytes.extend_from_slice(payload);
        }
        let length = bytes.len() as u32;
        bytes[4..8].copy_from_slice(&length.to_be_bytes());
        bytes
    }

    fn legacy_rle(dimension: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in [12, 34, 56] {
            let mut remaining = dimension * dimension;
            while remaining > 0 {
                let run = remaining.min(130);
                bytes.extend_from_slice(&[(run + 125) as u8, value]);
                remaining -= run;
            }
        }
        bytes
    }

    #[test]
    fn icns_png_payload_is_decoded_and_original_container_published() {
        let png = include_bytes!("../../tests/fixtures/icns-16.png");
        let bytes = icns_file(&[(*b"icp4", png), (*b"zzzz", b"ignored")]);
        assert!(
            async_io::block_on(loader_from_reader(io::Cursor::new(bytes.clone())).load()).is_err()
        );
        let directory = tempdir().unwrap();
        let store = ObjectStore::new(directory.path()).unwrap();
        let validated = store
            .stage_reader(bytes.as_slice(), None)
            .unwrap()
            .validate_with(&GlycinValidator)
            .unwrap();
        assert_eq!(validated.validation().metadata.format(), ImageFormat::Icns);
        assert_eq!(
            validated.validation().warnings,
            vec![ImportWarning::IcnsUnknownElements { count: 1 }]
        );
        assert_eq!(
            validated
                .validation()
                .container
                .as_ref()
                .unwrap()
                .primary()
                .codec(),
            ContainerCodec::Png
        );
        let published = validated.publish().unwrap();
        assert_eq!(
            fs::read(directory.path().join(published.stored.object.relative_path)).unwrap(),
            bytes
        );
    }

    #[test]
    fn icns_jpeg2000_payloads_decode_directly() {
        for payload in [
            include_bytes!("../../tests/fixtures/icns-16.jp2").as_slice(),
            include_bytes!("../../tests/fixtures/icns-16.j2k").as_slice(),
        ] {
            let bytes = icns_file(&[(*b"icp4", payload)]);
            let directory = tempdir().unwrap();
            let store = ObjectStore::new(directory.path()).unwrap();
            let validated = store
                .stage_reader(bytes.as_slice(), None)
                .unwrap()
                .validate_with(&GlycinValidator)
                .unwrap();
            assert_eq!(validated.validation().metadata.format(), ImageFormat::Icns);
            assert_eq!(
                (
                    validated.validation().metadata.width(),
                    validated.validation().metadata.height()
                ),
                (16, 16)
            );
            assert_eq!(
                validated
                    .validation()
                    .container
                    .as_ref()
                    .unwrap()
                    .primary()
                    .codec(),
                ContainerCodec::Jpeg2000
            );
            let published = validated.publish().unwrap();
            assert_eq!(
                fs::read(directory.path().join(published.stored.object.relative_path)).unwrap(),
                bytes
            );
        }
    }

    #[test]
    fn icns_legacy_rgb_uses_validated_mask_alpha() {
        let color = legacy_rle(16);
        let mask = vec![128; 256];
        let direct = icns::IconElement::new(icns::OSType(*b"is32"), color.clone())
            .decode_image_with_mask(&icns::IconElement::new(
                icns::OSType(*b"s8mk"),
                mask.clone(),
            ))
            .unwrap();
        assert_eq!(&direct.data()[..4], &[12, 34, 56, 128]);
        let bytes = icns_file(&[(*b"is32", &color), (*b"s8mk", &mask)]);
        let directory = tempdir().unwrap();
        let store = ObjectStore::new(directory.path()).unwrap();
        let validated = store
            .stage_reader(bytes.as_slice(), None)
            .unwrap()
            .validate_with(&GlycinValidator)
            .unwrap();
        assert_eq!(validated.validation().metadata.format(), ImageFormat::Icns);
        assert_eq!(
            validated
                .validation()
                .container
                .as_ref()
                .unwrap()
                .primary()
                .codec(),
            ContainerCodec::IcnsRgb
        );
        let published = validated.publish().unwrap();
        assert_eq!(
            fs::read(directory.path().join(published.stored.object.relative_path)).unwrap(),
            bytes
        );
    }

    #[test]
    fn icns_all_legacy_rgb_sizes_decode_with_their_masks() {
        for (color_kind, mask_kind, dimension) in [
            (*b"is32", *b"s8mk", 16),
            (*b"il32", *b"l8mk", 32),
            (*b"ih32", *b"h8mk", 48),
            (*b"it32", *b"t8mk", 128),
        ] {
            let mut color = legacy_rle(dimension);
            if dimension == 128 {
                color.splice(..0, [0; 4]);
            }
            let mask = vec![128; (dimension * dimension) as usize];
            let direct = icns::IconElement::new(icns::OSType(color_kind), color.clone())
                .decode_image_with_mask(&icns::IconElement::new(
                    icns::OSType(mask_kind),
                    mask.clone(),
                ))
                .unwrap();
            assert_eq!((direct.width(), direct.height()), (dimension, dimension));
            assert_eq!(&direct.data()[..4], &[12, 34, 56, 128]);
            let bytes = icns_file(&[(color_kind, &color), (mask_kind, &mask)]);
            let directory = tempdir().unwrap();
            let store = ObjectStore::new(directory.path()).unwrap();
            let validated = store
                .stage_reader(bytes.as_slice(), None)
                .unwrap()
                .validate_with(&GlycinValidator)
                .unwrap();
            assert_eq!(
                (
                    validated.validation().metadata.width(),
                    validated.validation().metadata.height()
                ),
                (dimension, dimension)
            );
            assert_eq!(
                validated
                    .validation()
                    .container
                    .as_ref()
                    .unwrap()
                    .primary()
                    .codec(),
                ContainerCodec::IcnsRgb
            );
        }
    }

    #[test]
    fn icns_argb_primary_decodes_without_glycin() {
        let mut payload = b"ARGB".to_vec();
        for value in [128, 12, 34, 56] {
            let mut remaining = 16 * 16;
            while remaining > 0 {
                let run = remaining.min(130);
                payload.extend_from_slice(&[(run + 125) as u8, value]);
                remaining -= run;
            }
        }
        let bytes = icns_file(&[(*b"ic04", &payload)]);
        let directory = tempdir().unwrap();
        let store = ObjectStore::new(directory.path()).unwrap();
        let validated = store
            .stage_reader(bytes.as_slice(), None)
            .unwrap()
            .validate_with(&GlycinValidator)
            .unwrap();
        assert_eq!(validated.validation().metadata.format(), ImageFormat::Icns);
        assert_eq!(
            validated
                .validation()
                .container
                .as_ref()
                .unwrap()
                .primary()
                .codec(),
            ContainerCodec::IcnsArgb
        );
    }

    #[test]
    fn icns_unprefixed_zero_start_uses_exact_legacy_stream() {
        let mut color = vec![0, 0, 0, 0];
        let mut append = |pixels: u32, value: u8| {
            let mut remaining = pixels;
            while remaining > 0 {
                let run = remaining.min(130);
                if run >= 3 {
                    color.extend_from_slice(&[(run + 125) as u8, value]);
                } else {
                    color.push((run - 1) as u8);
                    color.extend(std::iter::repeat_n(value, run as usize));
                }
                remaining -= run;
            }
        };
        append(254, 12);
        append(256, 34);
        append(256, 56);
        let mask = vec![128; 256];
        let bytes = icns_file(&[(*b"is32", &color), (*b"s8mk", &mask)]);
        let directory = tempdir().unwrap();
        let store = ObjectStore::new(directory.path()).unwrap();
        let validated = store
            .stage_reader(bytes.as_slice(), None)
            .unwrap()
            .validate_with(&GlycinValidator)
            .unwrap();
        assert_eq!(validated.validation().metadata.format(), ImageFormat::Icns);
    }

    #[test]
    fn icns_primary_prefers_scale_one_at_equal_physical_size() {
        let png = include_bytes!("../../tests/fixtures/icns-32.png");
        let bytes = icns_file(&[(*b"ic11", png), (*b"icp5", png)]);
        let directory = tempdir().unwrap();
        let store = ObjectStore::new(directory.path()).unwrap();
        let validated = store
            .stage_reader(bytes.as_slice(), None)
            .unwrap()
            .validate_with(&GlycinValidator)
            .unwrap();
        assert_eq!(
            validated
                .validation()
                .container
                .as_ref()
                .unwrap()
                .primary_ordinal(),
            1
        );
    }
}
