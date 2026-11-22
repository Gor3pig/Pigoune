use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use miniz_oxide::inflate::TINFLStatus;
use miniz_oxide::inflate::core::{DecompressorOxide, decompress, inflate_flags};

use crate::raster::RgbaImage;

const SIGNATURE: &[u8; 4] = b"8BPS";
const RESOURCE_SIGNATURE: &[u8; 4] = b"8BIM";
const VERSION: u16 = 1;
const HEADER_LENGTH: usize = 26;
const MOST_CHANNELS: u16 = 56;
const LARGEST_SIDE: u32 = 30_000;
const MOST_PIXELS: usize = 64_000_000;
const LARGEST_COMPRESSED_DATA: usize = 512 * 1024 * 1024;
const PALETTE_LENGTH: u64 = 768;
const PALETTE_SIDE: usize = 256;
const VERSION_INFO_RESOURCE: u16 = 1057;
const VERSION_INFO_PREFIX: usize = 5;
const RESOURCE_HEADER_LENGTH: u64 = 12;
const TAGGED_BLOCK_HEADER_LENGTH: u64 = 12;
const LARGE_SIGNATURE: &[u8; 4] = b"8B64";
const MERGED_TRANSPARENCY_KEYS: [[u8; 4]; 3] = [*b"Mtrn", *b"Mt16", *b"Mt32"];
const OPAQUE: u8 = 255;
const RGBA_CHANNELS: usize = 4;
const BITS_PER_BYTE: usize = 8;
const COMPRESSION_RAW: u16 = 0;
const COMPRESSION_RLE: u16 = 1;
const COMPRESSION_ZIP: u16 = 2;
const COMPRESSION_ZIP_PREDICTED: u16 = 3;
const REPEAT_MARKER: i8 = -128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PsdError {
    #[error("the file is not a readable PSD image")]
    Unreadable,
    #[error("the image is too large to be read")]
    TooLarge,
    #[error("this kind of PSD image cannot be read")]
    Unsupported,
    #[error("the file was saved without a merged image")]
    NoMergedImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    Bitmap,
    Gray,
    Indexed,
    Rgb,
    Cmyk,
}

impl ColorMode {
    fn from_code(code: u16) -> Result<Self, PsdError> {
        match code {
            0 => Ok(Self::Bitmap),
            1 | 8 => Ok(Self::Gray),
            2 => Ok(Self::Indexed),
            3 => Ok(Self::Rgb),
            4 => Ok(Self::Cmyk),
            _ => Err(PsdError::Unsupported),
        }
    }

    fn colorant_channels(self) -> usize {
        match self {
            Self::Bitmap | Self::Gray | Self::Indexed => 1,
            Self::Rgb => 3,
            Self::Cmyk => 4,
        }
    }
}

struct Header {
    channels: usize,
    width: usize,
    height: usize,
    depth: usize,
    mode: ColorMode,
}

impl Header {
    fn row_bytes(&self) -> usize {
        (self.width * self.depth).div_ceil(BITS_PER_BYTE)
    }

    fn plane_bytes(&self) -> usize {
        self.row_bytes() * self.height
    }

    fn pixels(&self) -> usize {
        self.width * self.height
    }
}

struct Sections {
    palette: Vec<u8>,
    transparency_in_merged_image: bool,
    has_real_merged_image: bool,
}

struct Stream<R> {
    reader: R,
    length: u64,
}

impl<R: Read + Seek> Stream<R> {
    fn new(mut reader: R) -> Result<Self, PsdError> {
        let length = reader
            .seek(SeekFrom::End(0))
            .map_err(|_| PsdError::Unreadable)?;
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|_| PsdError::Unreadable)?;
        Ok(Self { reader, length })
    }

    fn position(&mut self) -> Result<u64, PsdError> {
        self.reader
            .stream_position()
            .map_err(|_| PsdError::Unreadable)
    }

    fn remaining(&mut self) -> Result<u64, PsdError> {
        Ok(self.length.saturating_sub(self.position()?))
    }

    fn seek_to(&mut self, position: u64) -> Result<(), PsdError> {
        if position > self.length {
            return Err(PsdError::Unreadable);
        }
        self.reader
            .seek(SeekFrom::Start(position))
            .map_err(|_| PsdError::Unreadable)?;
        Ok(())
    }

    fn skip(&mut self, count: u64) -> Result<(), PsdError> {
        let target = self
            .position()?
            .checked_add(count)
            .ok_or(PsdError::Unreadable)?;
        self.seek_to(target)
    }

    fn fill(&mut self, buffer: &mut Vec<u8>, count: usize) -> Result<(), PsdError> {
        let wanted = u64::try_from(count).map_err(|_| PsdError::Unreadable)?;
        if wanted > self.remaining()? {
            return Err(PsdError::Unreadable);
        }
        buffer.clear();
        buffer.resize(count, 0);
        self.reader
            .read_exact(buffer)
            .map_err(|_| PsdError::Unreadable)
    }

    fn take(&mut self, count: usize) -> Result<Vec<u8>, PsdError> {
        let mut buffer = Vec::new();
        self.fill(&mut buffer, count)?;
        Ok(buffer)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PsdError> {
        let mut bytes = [0_u8; N];
        self.reader
            .read_exact(&mut bytes)
            .map_err(|_| PsdError::Unreadable)?;
        Ok(bytes)
    }

    fn u16(&mut self) -> Result<u16, PsdError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn i16(&mut self) -> Result<i16, PsdError> {
        Ok(i16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, PsdError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn section_length(&mut self) -> Result<u64, PsdError> {
        let length = u64::from(self.u32()?);
        if length > self.remaining()? {
            return Err(PsdError::Unreadable);
        }
        Ok(length)
    }
}

#[must_use]
pub fn is_photoshop_file(path: &Path) -> bool {
    let mut signature = [0_u8; SIGNATURE.len()];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut signature))
        .is_ok()
        && signature == *SIGNATURE
}

#[must_use]
pub fn photoshop_file_is_intact(path: &Path) -> bool {
    !matches!(read_merged_image_file(path), Err(PsdError::Unreadable))
}

#[must_use]
pub fn photoshop_problem(path: &Path) -> Option<PsdError> {
    let file = File::open(path).ok()?;
    let mut stream = Stream::new(BufReader::new(file)).ok()?;
    let outcome = read_header(&mut stream)
        .and_then(|header| read_sections(&mut stream, &header).map(|sections| (header, sections)));
    match outcome {
        Err(problem) => Some(problem),
        Ok((_, sections)) if !sections.has_real_merged_image => Some(PsdError::NoMergedImage),
        Ok(_) => None,
    }
}

pub fn read_merged_image_file(path: &Path) -> Result<RgbaImage, PsdError> {
    let file = File::open(path).map_err(|_| PsdError::Unreadable)?;
    read_merged_image_from(BufReader::new(file))
}

pub fn read_merged_image(bytes: &[u8]) -> Result<RgbaImage, PsdError> {
    read_merged_image_from(Cursor::new(bytes))
}

pub fn read_merged_image_from<R: Read + Seek>(reader: R) -> Result<RgbaImage, PsdError> {
    let mut stream = Stream::new(reader)?;
    let header = read_header(&mut stream)?;
    let sections = read_sections(&mut stream, &header)?;
    if !sections.has_real_merged_image {
        return Err(PsdError::NoMergedImage);
    }
    let planes = read_planes(&mut stream, &header, sections.transparency_in_merged_image)?;
    compose(&header, &sections.palette, &planes)
}

fn read_header<R: Read + Seek>(stream: &mut Stream<R>) -> Result<Header, PsdError> {
    if stream.length < HEADER_LENGTH as u64 {
        return Err(PsdError::Unreadable);
    }
    if stream.array::<4>()? != *SIGNATURE {
        return Err(PsdError::Unreadable);
    }
    if stream.u16()? != VERSION {
        return Err(PsdError::Unsupported);
    }
    stream.skip(6)?;
    let channels = stream.u16()?;
    let height = stream.u32()?;
    let width = stream.u32()?;
    let depth = stream.u16()?;
    let mode = ColorMode::from_code(stream.u16()?)?;
    if channels == 0 || channels > MOST_CHANNELS || width == 0 || height == 0 {
        return Err(PsdError::Unreadable);
    }
    if width > LARGEST_SIDE || height > LARGEST_SIDE {
        return Err(PsdError::TooLarge);
    }
    let header = Header {
        channels: usize::from(channels),
        width: usize::try_from(width).map_err(|_| PsdError::TooLarge)?,
        height: usize::try_from(height).map_err(|_| PsdError::TooLarge)?,
        depth: usize::from(depth),
        mode,
    };
    check_supported(&header)?;
    Ok(header)
}

fn check_supported(header: &Header) -> Result<(), PsdError> {
    if header.pixels() > MOST_PIXELS {
        return Err(PsdError::TooLarge);
    }
    let depth_is_known = match header.mode {
        ColorMode::Bitmap => header.depth == 1,
        ColorMode::Indexed => header.depth == 8,
        _ => matches!(header.depth, 8 | 16),
    };
    if !depth_is_known {
        return Err(PsdError::Unsupported);
    }
    if header.channels < header.mode.colorant_channels() {
        return Err(PsdError::Unreadable);
    }
    Ok(())
}

fn read_sections<R: Read + Seek>(
    stream: &mut Stream<R>,
    header: &Header,
) -> Result<Sections, PsdError> {
    let palette = read_color_mode_data(stream, header)?;
    let has_real_merged_image = read_image_resources(stream)?;
    let transparency_in_merged_image = saves_transparency(stream)?;
    Ok(Sections {
        palette,
        transparency_in_merged_image,
        has_real_merged_image,
    })
}

fn read_color_mode_data<R: Read + Seek>(
    stream: &mut Stream<R>,
    header: &Header,
) -> Result<Vec<u8>, PsdError> {
    let length = stream.section_length()?;
    if header.mode != ColorMode::Indexed {
        stream.skip(length)?;
        return Ok(Vec::new());
    }
    if length != PALETTE_LENGTH {
        return Err(PsdError::Unreadable);
    }
    stream.take(usize::try_from(length).map_err(|_| PsdError::Unreadable)?)
}

fn read_image_resources<R: Read + Seek>(stream: &mut Stream<R>) -> Result<bool, PsdError> {
    let length = stream.section_length()?;
    let end = stream.position()? + length;
    let mut has_real_merged_image = true;
    while stream.position()? + RESOURCE_HEADER_LENGTH <= end {
        if stream.array::<4>()? != *RESOURCE_SIGNATURE {
            break;
        }
        let identifier = stream.u16()?;
        let name_length = u64::from(stream.array::<1>()?[0]);
        let data_at = stream.position()? + name_length + (1 + name_length) % 2 + 4;
        if data_at > end {
            break;
        }
        stream.seek_to(data_at - 4)?;
        let data_length = u64::from(stream.u32()?);
        let next = data_at + data_length + data_length % 2;
        if next > end {
            break;
        }
        if identifier == VERSION_INFO_RESOURCE && data_length >= VERSION_INFO_PREFIX as u64 {
            let prefix = stream.take(VERSION_INFO_PREFIX)?;
            has_real_merged_image = prefix[VERSION_INFO_PREFIX - 1] != 0;
        }
        stream.seek_to(next)?;
    }
    stream.seek_to(end)?;
    Ok(has_real_merged_image)
}

fn saves_transparency<R: Read + Seek>(stream: &mut Stream<R>) -> Result<bool, PsdError> {
    let length = stream.section_length()?;
    let end = stream.position()? + length;
    let mut transparent = false;
    if length >= 4 {
        let info_length = u64::from(stream.u32()?);
        let info_end = stream.position()? + info_length;
        if info_end <= end {
            if info_length >= 2 {
                transparent = stream.i16()? < 0;
            }
            stream.seek_to(info_end)?;
            transparent = transparent || has_merged_transparency_block(stream, end);
        }
    }
    stream.seek_to(end)?;
    Ok(transparent)
}

fn has_merged_transparency_block<R: Read + Seek>(stream: &mut Stream<R>, end: u64) -> bool {
    let scan = |stream: &mut Stream<R>| -> Result<bool, PsdError> {
        let global_length = u64::from(stream.u32()?);
        let blocks_start = stream.position()? + global_length;
        stream.seek_to(blocks_start)?;
        while stream.position()? + TAGGED_BLOCK_HEADER_LENGTH <= end {
            let signature = stream.array::<4>()?;
            if signature != *RESOURCE_SIGNATURE && signature != *LARGE_SIGNATURE {
                break;
            }
            let key = stream.array::<4>()?;
            if MERGED_TRANSPARENCY_KEYS.contains(&key) {
                return Ok(true);
            }
            let length = u64::from(stream.u32()?);
            let next = stream.position()? + length.next_multiple_of(4);
            if next > end {
                break;
            }
            stream.seek_to(next)?;
        }
        Ok(false)
    };
    scan(stream).unwrap_or(false)
}

fn read_planes<R: Read + Seek>(
    stream: &mut Stream<R>,
    header: &Header,
    transparent: bool,
) -> Result<Vec<Vec<u8>>, PsdError> {
    let colorants = header.mode.colorant_channels();
    let wanted = colorants + usize::from(transparent && header.channels > colorants);
    match stream.u16()? {
        COMPRESSION_RAW => raw_planes(stream, header, wanted),
        COMPRESSION_RLE => run_length_planes(stream, header, wanted),
        code @ (COMPRESSION_ZIP | COMPRESSION_ZIP_PREDICTED) => {
            zipped_planes(stream, header, wanted, code == COMPRESSION_ZIP_PREDICTED)
        }
        _ => Err(PsdError::Unsupported),
    }
}

fn raw_planes<R: Read + Seek>(
    stream: &mut Stream<R>,
    header: &Header,
    wanted: usize,
) -> Result<Vec<Vec<u8>>, PsdError> {
    let mut row = Vec::new();
    let mut planes = Vec::with_capacity(wanted);
    for _ in 0..wanted {
        let mut plane = Vec::with_capacity(header.pixels());
        for _ in 0..header.height {
            stream.fill(&mut row, header.row_bytes())?;
            push_row(&mut plane, &row, header);
        }
        planes.push(plane);
    }
    Ok(planes)
}

fn run_length_planes<R: Read + Seek>(
    stream: &mut Stream<R>,
    header: &Header,
    wanted: usize,
) -> Result<Vec<Vec<u8>>, PsdError> {
    let counts = stream.take(header.channels * header.height * 2)?;
    let (counts, _) = counts.as_chunks::<2>();
    let mut packed = Vec::new();
    let mut planes = Vec::with_capacity(wanted);
    for channel in 0..wanted {
        let mut plane = Vec::with_capacity(header.pixels());
        for row in 0..header.height {
            let count = usize::from(u16::from_be_bytes(counts[channel * header.height + row]));
            stream.fill(&mut packed, count)?;
            push_row(
                &mut plane,
                &unpack_row(&packed, header.row_bytes())?,
                header,
            );
        }
        planes.push(plane);
    }
    Ok(planes)
}

fn zipped_planes<R: Read + Seek>(
    stream: &mut Stream<R>,
    header: &Header,
    wanted: usize,
    predicted: bool,
) -> Result<Vec<Vec<u8>>, PsdError> {
    if header.depth == 1 {
        return Err(PsdError::Unsupported);
    }
    let expected = header.channels * header.plane_bytes();
    let packed = usize::try_from(stream.remaining()?).map_err(|_| PsdError::TooLarge)?;
    if expected > LARGEST_COMPRESSED_DATA || packed > LARGEST_COMPRESSED_DATA {
        return Err(PsdError::TooLarge);
    }
    let compressed = stream.take(packed)?;
    let mut data = inflate_streams(&compressed, expected)?;
    if predicted {
        undo_prediction(&mut data, header);
    }
    let mut planes = Vec::with_capacity(wanted);
    for channel in 0..wanted {
        let mut plane = Vec::with_capacity(header.pixels());
        let start = channel * header.plane_bytes();
        for row in data[start..start + header.plane_bytes()].chunks_exact(header.row_bytes()) {
            push_row(&mut plane, row, header);
        }
        planes.push(plane);
    }
    Ok(planes)
}

fn inflate_streams(compressed: &[u8], expected: usize) -> Result<Vec<u8>, PsdError> {
    let flags = inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
        | inflate_flags::TINFL_FLAG_COMPUTE_ADLER32
        | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    let mut output = vec![0_u8; expected];
    let mut consumed = 0;
    let mut written = 0;
    while written < expected {
        let mut decompressor = Box::<DecompressorOxide>::default();
        let (status, used, produced) = decompress(
            &mut decompressor,
            &compressed[consumed..],
            &mut output,
            written,
            flags,
        );
        if status != TINFLStatus::Done || used + produced == 0 {
            return Err(PsdError::Unreadable);
        }
        consumed += used;
        written += produced;
    }
    Ok(output)
}

fn undo_prediction(data: &mut [u8], header: &Header) {
    for row in data.chunks_exact_mut(header.row_bytes()) {
        if header.depth == 16 {
            let mut previous = 0_u16;
            for word in row.as_chunks_mut::<2>().0 {
                previous = previous.wrapping_add(u16::from_be_bytes(*word));
                *word = previous.to_be_bytes();
            }
        } else {
            let mut previous = 0_u8;
            for byte in row {
                previous = previous.wrapping_add(*byte);
                *byte = previous;
            }
        }
    }
}

fn unpack_row(packed: &[u8], row_bytes: usize) -> Result<Vec<u8>, PsdError> {
    let mut row = Vec::with_capacity(row_bytes);
    let mut rest = packed;
    while row.len() < row_bytes {
        let (&marker, after) = rest.split_first().ok_or(PsdError::Unreadable)?;
        let marker = i8::from_be_bytes([marker]);
        rest = after;
        if marker == REPEAT_MARKER {
            continue;
        }
        let count = usize::from(marker.unsigned_abs()) + 1;
        if marker >= 0 {
            row.extend_from_slice(rest.get(..count).ok_or(PsdError::Unreadable)?);
            rest = &rest[count..];
        } else {
            let (&value, after) = rest.split_first().ok_or(PsdError::Unreadable)?;
            row.extend(std::iter::repeat_n(value, count));
            rest = after;
        }
    }
    (row.len() == row_bytes)
        .then_some(row)
        .ok_or(PsdError::Unreadable)
}

fn push_row(plane: &mut Vec<u8>, row: &[u8], header: &Header) {
    match header.depth {
        16 => plane.extend(row.as_chunks::<2>().0.iter().map(|pair| pair[0])),
        1 => {
            for x in 0..header.width {
                let black = (row[x / BITS_PER_BYTE] >> (BITS_PER_BYTE - 1 - x % BITS_PER_BYTE)) & 1;
                plane.push(if black == 1 { 0 } else { OPAQUE });
            }
        }
        _ => plane.extend_from_slice(row),
    }
}

fn without_white_matte(color: u8, opacity: u8) -> u8 {
    if opacity == OPAQUE || opacity == 0 {
        return color;
    }
    let opacity = i32::from(opacity);
    let straight = ((i32::from(color) + opacity - 255) * 255 + opacity / 2) / opacity;
    u8::try_from(straight.clamp(0, 255)).unwrap_or(OPAQUE)
}

fn compose(header: &Header, palette: &[u8], planes: &[Vec<u8>]) -> Result<RgbaImage, PsdError> {
    let colorants = header.mode.colorant_channels();
    let alpha = planes.get(colorants);
    let mut pixels = Vec::with_capacity(header.pixels() * RGBA_CHANNELS);
    for index in 0..header.pixels() {
        let sample = |channel: usize| planes[channel][index];
        let [red, green, blue] = match header.mode {
            ColorMode::Bitmap | ColorMode::Gray => [sample(0); 3],
            ColorMode::Indexed => {
                let entry = usize::from(sample(0));
                [
                    palette[entry],
                    palette[PALETTE_SIDE + entry],
                    palette[2 * PALETTE_SIDE + entry],
                ]
            }
            ColorMode::Rgb => [sample(0), sample(1), sample(2)],
            ColorMode::Cmyk => {
                let key = u16::from(sample(3));
                [0, 1, 2].map(|channel| {
                    u8::try_from(u16::from(sample(channel)) * key / u16::from(OPAQUE))
                        .unwrap_or(OPAQUE)
                })
            }
        };
        let opacity = alpha.map_or(OPAQUE, |plane| plane[index]);
        pixels.extend([
            without_white_matte(red, opacity),
            without_white_matte(green, opacity),
            without_white_matte(blue, opacity),
            opacity,
        ]);
    }
    RgbaImage::new(
        u32::try_from(header.width).map_err(|_| PsdError::TooLarge)?,
        u32::try_from(header.height).map_err(|_| PsdError::TooLarge)?,
        pixels,
    )
    .ok_or(PsdError::Unreadable)
}
#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io::{Cursor, Read, Seek, SeekFrom};
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use super::{
        PsdError, is_photoshop_file, photoshop_file_is_intact, photoshop_problem,
        read_merged_image, read_merged_image_from,
    };

    const RGB: u16 = 3;
    const GRAY: u16 = 1;
    const INDEXED: u16 = 2;
    const CMYK: u16 = 4;
    const BITMAP: u16 = 0;
    const RAW: u16 = 0;
    const RLE: u16 = 1;
    const ZIP: u16 = 2;
    const ZIP_PREDICTED: u16 = 3;

    struct Parts<'a> {
        mode: u16,
        depth: u16,
        channels: u16,
        width: u32,
        height: u32,
        color_data: &'a [u8],
        resources: &'a [u8],
        layers: &'a [u8],
        compression: u16,
        image_data: &'a [u8],
    }

    fn section(content: &[u8]) -> Vec<u8> {
        let mut bytes = u32::try_from(content.len()).unwrap().to_be_bytes().to_vec();
        bytes.extend_from_slice(content);
        bytes
    }

    fn file(parts: &Parts) -> Vec<u8> {
        let mut bytes = b"8BPS".to_vec();
        bytes.extend_from_slice(&1_u16.to_be_bytes());
        bytes.extend_from_slice(&[0; 6]);
        bytes.extend_from_slice(&parts.channels.to_be_bytes());
        bytes.extend_from_slice(&parts.height.to_be_bytes());
        bytes.extend_from_slice(&parts.width.to_be_bytes());
        bytes.extend_from_slice(&parts.depth.to_be_bytes());
        bytes.extend_from_slice(&parts.mode.to_be_bytes());
        bytes.extend_from_slice(&section(parts.color_data));
        bytes.extend_from_slice(&section(parts.resources));
        bytes.extend_from_slice(&section(parts.layers));
        bytes.extend_from_slice(&parts.compression.to_be_bytes());
        bytes.extend_from_slice(parts.image_data);
        bytes
    }

    fn layer_info(count: i16) -> Vec<u8> {
        let mut bytes = 2_u32.to_be_bytes().to_vec();
        bytes.extend_from_slice(&count.to_be_bytes());
        bytes
    }

    fn version_info(has_real_merged_image: bool) -> Vec<u8> {
        let mut data = 1_u32.to_be_bytes().to_vec();
        data.push(u8::from(has_real_merged_image));
        data.extend_from_slice(&[0; 8]);
        data.extend_from_slice(&1_u32.to_be_bytes());
        data.push(0);
        let mut bytes = b"8BIM".to_vec();
        bytes.extend_from_slice(&1057_u16.to_be_bytes());
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&17_u32.to_be_bytes());
        bytes.extend_from_slice(&data);
        bytes
    }

    fn raw(mode: u16, depth: u16, channels: u16, width: u32, height: u32, data: &[u8]) -> Vec<u8> {
        file(&Parts {
            mode,
            depth,
            channels,
            width,
            height,
            color_data: &[],
            resources: &[],
            layers: &[],
            compression: RAW,
            image_data: data,
        })
    }

    fn with_layers(channels: u16, layers: &[u8], data: &[u8]) -> Vec<u8> {
        file(&Parts {
            mode: RGB,
            depth: 8,
            channels,
            width: 1,
            height: 1,
            color_data: &[],
            resources: &[],
            layers,
            compression: RAW,
            image_data: data,
        })
    }

    fn zipped(compression: u16, depth: u16, width: u32, height: u32, planes: &[&[u8]]) -> Vec<u8> {
        let mut data = Vec::new();
        for plane in planes {
            data.extend_from_slice(plane);
        }
        if compression == ZIP_PREDICTED {
            data = predicted(&data, depth, usize::try_from(width).unwrap());
        }
        file(&Parts {
            mode: RGB,
            depth,
            channels: u16::try_from(planes.len()).unwrap(),
            width,
            height,
            color_data: &[],
            resources: &[],
            layers: &[],
            compression,
            image_data: &miniz_oxide::deflate::compress_to_vec_zlib(&data, 6),
        })
    }

    fn predicted(data: &[u8], depth: u16, width: usize) -> Vec<u8> {
        let row_bytes = width * usize::from(depth / 8);
        let mut out = Vec::new();
        for row in data.chunks_exact(row_bytes) {
            if depth == 16 {
                let mut previous = 0_u16;
                for word in row.as_chunks::<2>().0 {
                    let value = u16::from_be_bytes(*word);
                    out.extend_from_slice(&value.wrapping_sub(previous).to_be_bytes());
                    previous = value;
                }
            } else {
                let mut previous = 0_u8;
                for &byte in row {
                    out.push(byte.wrapping_sub(previous));
                    previous = byte;
                }
            }
        }
        out
    }

    fn pixels(bytes: &[u8]) -> Vec<u8> {
        read_merged_image(bytes)
            .expect("the image is read")
            .into_pixels()
    }

    #[test]
    fn reads_raw_rgb_planes() {
        let bytes = raw(RGB, 8, 3, 2, 1, &[255, 0, 0, 0, 0, 255]);

        assert_eq!(pixels(&bytes), [255, 0, 0, 255, 0, 0, 255, 255]);
    }

    #[test]
    fn a_negative_layer_count_makes_the_first_extra_channel_the_transparency() {
        let bytes = with_layers(4, &layer_info(-1), &[255, 255, 255, 0]);

        assert_eq!(pixels(&bytes), [255, 255, 255, 0]);
    }

    #[test]
    fn a_saving_merged_transparency_block_also_makes_the_extra_channel_transparency() {
        for key in [b"Mtrn", b"Mt16", b"Mt32"] {
            let mut layers = layer_info(2);
            layers.extend_from_slice(&0_u32.to_be_bytes());
            layers.extend_from_slice(b"8BIMlsct");
            layers.extend_from_slice(&4_u32.to_be_bytes());
            layers.extend_from_slice(&[0; 4]);
            layers.extend_from_slice(b"8BIM");
            layers.extend_from_slice(key);
            layers.extend_from_slice(&1_u32.to_be_bytes());
            layers.extend_from_slice(&[0; 4]);
            let bytes = with_layers(4, &layers, &[255, 255, 255, 0]);

            assert_eq!(pixels(&bytes), [255, 255, 255, 0], "{key:?}");
        }
    }

    #[test]
    fn other_tagged_blocks_do_not_make_an_extra_channel_transparency() {
        let mut layers = layer_info(2);
        layers.extend_from_slice(&0_u32.to_be_bytes());
        layers.extend_from_slice(b"8BIMlsct");
        layers.extend_from_slice(&4_u32.to_be_bytes());
        layers.extend_from_slice(&[0; 4]);
        let bytes = with_layers(4, &layers, &[10, 20, 30, 40]);

        assert_eq!(pixels(&bytes), [10, 20, 30, 255]);
    }

    #[test]
    fn colors_stored_over_white_are_restored_when_there_is_transparency() {
        let half_red = with_layers(4, &layer_info(-1), &[255, 127, 127, 128]);
        let quarter_blue = with_layers(4, &layer_info(-1), &[191, 191, 255, 64]);
        let opaque = with_layers(4, &layer_info(-1), &[10, 20, 30, 255]);

        assert_eq!(pixels(&half_red), [255, 0, 0, 128]);
        assert_eq!(pixels(&quarter_blue), [0, 0, 255, 64]);
        assert_eq!(pixels(&opaque), [10, 20, 30, 255]);
    }

    #[test]
    fn an_extra_channel_is_not_transparency_without_a_negative_layer_count() {
        let positive = with_layers(4, &layer_info(2), &[10, 20, 30, 40]);
        let none = with_layers(4, &[], &[10, 20, 30, 40]);
        let flat = with_layers(4, &layer_info(0), &[10, 20, 30, 40]);

        assert_eq!(pixels(&positive), [10, 20, 30, 255]);
        assert_eq!(pixels(&none), [10, 20, 30, 255]);
        assert_eq!(pixels(&flat), [10, 20, 30, 255]);
    }

    #[test]
    fn a_file_saved_without_a_merged_image_is_reported_before_its_pixels_are_read() {
        let bytes = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 3,
            width: 1,
            height: 1,
            color_data: &[],
            resources: &version_info(false),
            layers: &layer_info(3),
            compression: RAW,
            image_data: &[],
        });

        assert_eq!(read_merged_image(&bytes), Err(PsdError::NoMergedImage));
    }

    #[test]
    fn a_file_with_a_real_merged_image_is_read() {
        let bytes = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 3,
            width: 1,
            height: 1,
            color_data: &[],
            resources: &version_info(true),
            layers: &layer_info(3),
            compression: RAW,
            image_data: &[1, 2, 3],
        });

        assert_eq!(pixels(&bytes), [1, 2, 3, 255]);
    }

    #[test]
    fn a_damaged_resource_block_is_ignored_instead_of_failing_the_file() {
        let mut resources = b"8BIM".to_vec();
        resources.extend_from_slice(&1057_u16.to_be_bytes());
        resources.extend_from_slice(&[0, 0]);
        resources.extend_from_slice(&u32::MAX.to_be_bytes());
        let bytes = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 3,
            width: 1,
            height: 1,
            color_data: &[],
            resources: &resources,
            layers: &[],
            compression: RAW,
            image_data: &[7, 8, 9],
        });

        assert_eq!(pixels(&bytes), [7, 8, 9, 255]);
    }

    #[test]
    fn reads_run_length_rows_with_literals_and_repeats() {
        let rows: [&[u8]; 3] = [&[3, 1, 2, 3, 4], &[0xFD, 9], &[0xFD, 7]];
        let mut data = Vec::new();
        for row in &rows {
            data.extend_from_slice(&u16::try_from(row.len()).unwrap().to_be_bytes());
        }
        for row in &rows {
            data.extend_from_slice(row);
        }
        let bytes = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 3,
            width: 4,
            height: 1,
            color_data: &[],
            resources: &[],
            layers: &[],
            compression: RLE,
            image_data: &data,
        });

        assert_eq!(
            pixels(&bytes),
            [1, 9, 7, 255, 2, 9, 7, 255, 3, 9, 7, 255, 4, 9, 7, 255]
        );
    }

    #[test]
    fn reads_zip_compressed_planes_with_and_without_prediction() {
        let red: &[u8] = &[10, 20, 30, 40, 50, 60];
        let green: &[u8] = &[1, 1, 2, 3, 5, 8];
        let blue: &[u8] = &[255, 0, 255, 0, 255, 0];
        let expected = [
            10, 1, 255, 255, 20, 1, 0, 255, 30, 2, 255, 255, 40, 3, 0, 255, 50, 5, 255, 255, 60, 8,
            0, 255,
        ];

        let plain = zipped(ZIP, 8, 3, 2, &[red, green, blue]);
        let delta = zipped(ZIP_PREDICTED, 8, 3, 2, &[red, green, blue]);

        assert_eq!(pixels(&plain), expected);
        assert_eq!(pixels(&delta), expected);
    }

    #[test]
    fn reads_sixteen_bit_zip_compressed_planes_with_prediction() {
        let red: &[u8] = &[0x10, 0xFF, 0x20, 0x00, 0x30, 0x01];
        let green: &[u8] = &[0x40, 0x00, 0x41, 0x00, 0x42, 0x00];
        let blue: &[u8] = &[0xFF, 0xFF, 0x00, 0x00, 0x80, 0x00];

        let delta = zipped(ZIP_PREDICTED, 16, 3, 1, &[red, green, blue]);

        assert_eq!(
            pixels(&delta),
            [
                0x10, 0x40, 0xFF, 255, 0x20, 0x41, 0x00, 255, 0x30, 0x42, 0x80, 255
            ]
        );
    }

    #[test]
    fn reads_zip_data_written_as_one_stream_per_channel() {
        let channels: [&[u8]; 3] = [
            &[10, 20, 30, 40, 50, 60],
            &[1, 2, 3, 4, 5, 6],
            &[9, 8, 7, 6, 5, 4],
        ];
        let mut image_data = Vec::new();
        for channel in channels {
            image_data.extend_from_slice(&miniz_oxide::deflate::compress_to_vec_zlib(channel, 6));
        }
        let bytes = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 3,
            width: 3,
            height: 2,
            color_data: &[],
            resources: &[],
            layers: &[],
            compression: ZIP,
            image_data: &image_data,
        });

        assert_eq!(
            pixels(&bytes),
            [
                10, 1, 9, 255, 20, 2, 8, 255, 30, 3, 7, 255, 40, 4, 6, 255, 50, 5, 5, 255, 60, 6,
                4, 255
            ]
        );
    }

    #[test]
    fn a_zip_stream_that_is_too_short_or_damaged_is_unreadable() {
        let short = zipped(ZIP, 8, 2, 1, &[&[1, 2], &[3, 4]]);
        let mut truncated = zipped(ZIP, 8, 2, 1, &[&[1, 2], &[3, 4], &[5, 6]]);
        truncated.truncate(truncated.len() - 3);
        let mut checksum = zipped(ZIP, 8, 2, 1, &[&[1, 2], &[3, 4], &[5, 6]]);
        let last = checksum.len() - 1;
        checksum[last] ^= 0xFF;

        assert_eq!(read_merged_image(&short), Err(PsdError::Unreadable));
        assert_eq!(read_merged_image(&truncated), Err(PsdError::Unreadable));
        assert_eq!(read_merged_image(&checksum), Err(PsdError::Unreadable));
    }

    #[test]
    fn a_zip_stream_larger_than_the_image_cannot_swell_the_output() {
        let big = vec![7_u8; 100_000];
        let bytes = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 3,
            width: 2,
            height: 1,
            color_data: &[],
            resources: &[],
            layers: &[],
            compression: ZIP,
            image_data: &miniz_oxide::deflate::compress_to_vec_zlib(&big, 6),
        });

        assert_eq!(read_merged_image(&bytes), Err(PsdError::Unreadable));
    }

    #[test]
    fn keeps_the_high_byte_of_sixteen_bit_samples() {
        let bytes = raw(GRAY, 16, 1, 1, 1, &[0x80, 0x01]);

        assert_eq!(pixels(&bytes), [0x80, 0x80, 0x80, 255]);
    }

    #[test]
    fn reads_cmyk_stored_inverted() {
        let white = raw(CMYK, 8, 4, 1, 1, &[255, 255, 255, 255]);
        let cyan = raw(CMYK, 8, 4, 1, 1, &[0, 255, 255, 255]);
        let black = raw(CMYK, 8, 4, 1, 1, &[255, 255, 255, 0]);

        assert_eq!(pixels(&white), [255, 255, 255, 255]);
        assert_eq!(pixels(&cyan), [0, 255, 255, 255]);
        assert_eq!(pixels(&black), [0, 0, 0, 255]);
    }

    #[test]
    fn reads_indexed_colors_from_the_palette() {
        let mut palette = vec![0_u8; 768];
        palette[1] = 200;
        palette[256 + 1] = 100;
        palette[512 + 1] = 50;
        let bytes = file(&Parts {
            mode: INDEXED,
            depth: 8,
            channels: 1,
            width: 1,
            height: 1,
            color_data: &palette,
            resources: &[],
            layers: &[],
            compression: RAW,
            image_data: &[1],
        });

        assert_eq!(pixels(&bytes), [200, 100, 50, 255]);
    }

    #[test]
    fn rejects_an_indexed_image_without_a_palette() {
        let bytes = raw(INDEXED, 8, 1, 1, 1, &[0]);

        assert_eq!(read_merged_image(&bytes), Err(PsdError::Unreadable));
    }

    #[test]
    fn reads_one_bit_images_with_black_for_set_bits() {
        let bytes = raw(BITMAP, 1, 1, 8, 1, &[0b1000_0000]);

        let pixels = pixels(&bytes);

        assert_eq!(&pixels[..4], [0, 0, 0, 255]);
        assert_eq!(&pixels[4..8], [255, 255, 255, 255]);
    }

    #[test]
    fn refuses_what_it_cannot_read_without_failing() {
        let thirty_two_bit = raw(RGB, 32, 3, 1, 1, &[0; 12]);
        let lab = raw(9, 8, 3, 1, 1, &[0; 3]);
        let unknown_compression = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 3,
            width: 1,
            height: 1,
            color_data: &[],
            resources: &[],
            layers: &[],
            compression: 9,
            image_data: &[0; 3],
        });

        assert_eq!(
            read_merged_image(&thirty_two_bit),
            Err(PsdError::Unsupported)
        );
        assert_eq!(read_merged_image(&lab), Err(PsdError::Unsupported));
        assert_eq!(
            read_merged_image(&unknown_compression),
            Err(PsdError::Unsupported)
        );
    }

    #[test]
    fn refuses_a_large_document_before_reading_its_pixels() {
        let bytes = raw(RGB, 8, 3, 20_000, 20_000, &[]);

        assert_eq!(read_merged_image(&bytes), Err(PsdError::TooLarge));
    }

    #[test]
    fn refuses_other_files() {
        assert_eq!(read_merged_image(b"GIF89a"), Err(PsdError::Unreadable));
        assert_eq!(read_merged_image(&[]), Err(PsdError::Unreadable));
    }

    struct Counting<R> {
        inner: R,
        read: Rc<Cell<usize>>,
    }

    impl<R: Read> Read for Counting<R> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let count = self.inner.read(buffer)?;
            self.read.set(self.read.get() + count);
            Ok(count)
        }
    }

    impl<R: Seek> Seek for Counting<R> {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(position)
        }
    }

    #[test]
    fn the_layers_are_skipped_instead_of_read() {
        let mut layers = layer_info(-5);
        layers.resize(2_000_000, 0xAB);
        let bytes = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 4,
            width: 1,
            height: 1,
            color_data: &[],
            resources: &version_info(true),
            layers: &layers,
            compression: RAW,
            image_data: &[1, 2, 3, 255],
        });
        let total = bytes.len();
        let read = Rc::new(Cell::new(0));
        let reader = Counting {
            inner: Cursor::new(bytes),
            read: Rc::clone(&read),
        };

        let image = read_merged_image_from(reader).expect("the image is read");

        assert_eq!(image.into_pixels(), [1, 2, 3, 255]);
        assert!(
            read.get() < 1000,
            "{} of {total} bytes were read",
            read.get()
        );
    }

    #[test]
    fn never_panics_on_truncated_or_damaged_files() {
        let valid = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 4,
            width: 3,
            height: 2,
            color_data: &[],
            resources: &version_info(true),
            layers: &layer_info(-1),
            compression: RAW,
            image_data: &(0..24).collect::<Vec<u8>>(),
        });
        let zipped_valid = zipped(ZIP_PREDICTED, 8, 3, 2, &[&[1; 6], &[2; 6], &[3; 6]]);
        for bytes in [&valid, &zipped_valid] {
            for length in 0..bytes.len() {
                let _ = read_merged_image(&bytes[..length]);
            }
            for position in 0..bytes.len() {
                for value in [0, 1, 127, 128, 255] {
                    let mut damaged = bytes.clone();
                    damaged[position] = value;
                    let _ = read_merged_image(&damaged);
                }
            }
        }
    }

    #[test]
    fn rejects_a_run_that_overflows_its_row() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0, 2, 0, 2, 0, 2]);
        data.extend_from_slice(&[0xFE, 1, 0xFE, 1, 0xFE, 1]);
        let bytes = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 3,
            width: 1,
            height: 1,
            color_data: &[],
            resources: &[],
            layers: &[],
            compression: RLE,
            image_data: &data,
        });

        assert_eq!(read_merged_image(&bytes), Err(PsdError::Unreadable));
    }

    struct Folder(PathBuf);

    impl Folder {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("pigoune-psd-{label}-{}", std::process::id()));
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
    fn tells_photoshop_files_apart_by_their_signature() {
        let folder = Folder::new("signature");
        let psd = folder.write("a.psd", &raw(RGB, 8, 3, 1, 1, &[1, 2, 3]));
        let other = folder.write("b.psd", b"GIF89a");

        assert!(is_photoshop_file(&psd));
        assert!(!is_photoshop_file(&other));
        assert!(!is_photoshop_file(&folder.0.join("missing.psd")));
    }

    #[test]
    fn a_file_that_cannot_be_drawn_is_still_intact_but_a_damaged_one_is_not() {
        let folder = Folder::new("intact");
        let fine = folder.write("a.psd", &raw(RGB, 8, 3, 1, 1, &[1, 2, 3]));
        let deep = folder.write("b.psd", &raw(RGB, 32, 3, 1, 1, &[0; 12]));
        let without = folder.write("c.psd", &with_resources(&version_info(false)));
        let damaged = folder.write("d.psd", &raw(RGB, 8, 3, 4, 4, &[1, 2, 3]));

        assert!(photoshop_file_is_intact(&fine));
        assert!(photoshop_file_is_intact(&deep));
        assert!(photoshop_file_is_intact(&without));
        assert!(!photoshop_file_is_intact(&damaged));
    }

    fn with_resources(resources: &[u8]) -> Vec<u8> {
        file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 3,
            width: 1,
            height: 1,
            color_data: &[],
            resources,
            layers: &layer_info(1),
            compression: RAW,
            image_data: &[],
        })
    }

    #[test]
    fn names_the_problem_that_prevents_a_preview() {
        let folder = Folder::new("problem");
        let fine = folder.write("a.psd", &raw(RGB, 8, 3, 1, 1, &[1, 2, 3]));
        let deep = folder.write("b.psd", &raw(RGB, 32, 3, 1, 1, &[0; 12]));
        let large = folder.write("c.psd", &raw(RGB, 8, 3, 20_000, 20_000, &[]));
        let without = folder.write("d.psd", &with_resources(&version_info(false)));

        assert_eq!(photoshop_problem(&fine), None);
        assert_eq!(photoshop_problem(&deep), Some(PsdError::Unsupported));
        assert_eq!(photoshop_problem(&large), Some(PsdError::TooLarge));
        assert_eq!(photoshop_problem(&without), Some(PsdError::NoMergedImage));
        assert_eq!(photoshop_problem(Path::new("/nonexistent/none.psd")), None);
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn pixel_at(image: &crate::raster::RgbaImage, x: usize, y: usize) -> [u8; 4] {
        let start = (y * usize::try_from(image.width()).unwrap() + x) * 4;
        [
            image.pixels()[start],
            image.pixels()[start + 1],
            image.pixels()[start + 2],
            image.pixels()[start + 3],
        ]
    }

    fn assert_near(actual: [u8; 4], expected: [u8; 4], name: &str) {
        for channel in 0..4 {
            assert!(
                actual[channel].abs_diff(expected[channel]) <= 1,
                "{name}: {actual:?} instead of {expected:?}"
            );
        }
    }

    #[test]
    fn reads_the_files_checked_against_other_readers() {
        let cases: [(&str, [[u8; 4]; 4]); 6] = [
            (
                "zip-merged.psd",
                [
                    [0, 0, 255, 255],
                    [255, 0, 0, 255],
                    [136, 145, 119, 255],
                    [255, 255, 0, 255],
                ],
            ),
            (
                "zip-predicted-16bit.psd",
                [
                    [0, 0, 255, 255],
                    [255, 0, 0, 255],
                    [136, 146, 119, 255],
                    [255, 255, 0, 255],
                ],
            ),
            (
                "selection-channel.psd",
                [
                    [0, 0, 255, 255],
                    [255, 0, 0, 255],
                    [136, 145, 119, 255],
                    [255, 255, 0, 255],
                ],
            ),
            (
                "cmyk-rle.psd",
                [
                    [255, 255, 0, 255],
                    [0, 255, 255, 255],
                    [119, 110, 136, 255],
                    [0, 0, 255, 255],
                ],
            ),
            (
                "layered-transparent.psd",
                [
                    [255, 255, 255, 0],
                    [255, 0, 0, 255],
                    [137, 144, 118, 136],
                    [255, 255, 0, 255],
                ],
            ),
            (
                "merged-transparency-block.psd",
                [
                    [255, 255, 255, 0],
                    [255, 0, 0, 255],
                    [137, 144, 118, 136],
                    [255, 255, 0, 255],
                ],
            ),
        ];
        for (name, expected) in cases {
            let image = super::read_merged_image_file(&fixture(name)).expect(name);

            assert_eq!((image.width(), image.height()), (16, 8), "{name}");
            for (point, color) in [(0, 0), (15, 0), (8, 4), (15, 7)].into_iter().zip(expected) {
                assert_near(pixel_at(&image, point.0, point.1), color, name);
            }
        }
    }

    #[test]
    fn a_real_file_saved_without_a_merged_image_is_named() {
        let path = fixture("no-preview.psd");

        assert_eq!(
            super::read_merged_image_file(&path),
            Err(PsdError::NoMergedImage)
        );
        assert_eq!(photoshop_problem(&path), Some(PsdError::NoMergedImage));
        assert!(photoshop_file_is_intact(&path));
    }

    #[test]
    fn the_files_of_another_program_are_read() {
        for name in ["rle-gradient.psd", "zip-layers-gradient.psd"] {
            let image = super::read_merged_image_file(&fixture(name)).expect(name);

            assert_eq!((image.width(), image.height()), (16, 8), "{name}");
            assert_near(pixel_at(&image, 0, 0), [255, 0, 0, 255], name);
        }
    }
}
