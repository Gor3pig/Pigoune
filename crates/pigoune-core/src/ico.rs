//! Structural ICO inventory. The caller supplies an internal staged reader.

use std::{
    error::Error,
    fmt,
    io::{self, Read, Seek, SeekFrom},
};

use crate::{
    ContainerCodec, ContainerMetadata, ContainerRepresentation, ICON_CONTAINER_MAX_DIMENSION,
    ICON_CONTAINER_MAX_REPRESENTATIONS,
};

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

#[derive(Debug)]
pub enum IcoError {
    Io(io::Error),
    Truncated(&'static str),
    InvalidHeader,
    InvalidCount,
    InvalidEntry(u16, &'static str),
    Overflow,
    LimitExceeded(u16),
    PayloadOverlap,
    InvalidPng(u16, &'static str),
    UnsupportedRepresentation(u16, &'static str),
    InvalidDib(u16, &'static str),
    DimensionMismatch(u16),
}

impl fmt::Display for IcoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "ICO I/O error: {error}"),
            Self::Truncated(part) => write!(f, "truncated ICO {part}"),
            Self::InvalidHeader => f.write_str("invalid ICO header (reserved or type)"),
            Self::InvalidCount => f.write_str("invalid ICO entry count"),
            Self::InvalidEntry(n, why) => write!(f, "invalid ICO entry {n}: {why}"),
            Self::Overflow => f.write_str("ICO offset or size overflow"),
            Self::LimitExceeded(n) => write!(f, "ICO representation {n} exceeds safety limits"),
            Self::PayloadOverlap => f.write_str("ICO payloads overlap partially"),
            Self::InvalidPng(n, why) => write!(f, "invalid ICO PNG representation {n}: {why}"),
            Self::UnsupportedRepresentation(n, why) => {
                write!(f, "unsupported ICO representation {n}: {why}")
            }
            Self::InvalidDib(n, why) => write!(f, "invalid ICO DIB representation {n}: {why}"),
            Self::DimensionMismatch(n) => write!(
                f,
                "ICO directory and payload dimensions disagree for entry {n}"
            ),
        }
    }
}

impl Error for IcoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct Entry {
    raw: [u8; 16],
    offset: u64,
    size: u64,
    end: u64,
}

#[derive(Clone, Debug)]
pub struct IcoContainer {
    metadata: ContainerMetadata,
    entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IcoPrimary {
    pub payload_offset: u64,
    pub payload_size: u64,
    pub directory_entry: [u8; 16],
}

impl IcoContainer {
    pub fn metadata(&self) -> &ContainerMetadata {
        &self.metadata
    }

    pub fn primary(&self) -> IcoPrimary {
        let entry = &self.entries[usize::from(self.metadata.primary_ordinal())];
        let mut directory_entry = entry.raw;
        directory_entry[8..12].copy_from_slice(&(entry.size as u32).to_le_bytes());
        directory_entry[12..16].copy_from_slice(&22_u32.to_le_bytes());
        IcoPrimary {
            payload_offset: entry.offset,
            payload_size: entry.size,
            directory_entry,
        }
    }
}

pub fn parse_ico<R: Read + Seek>(reader: &mut R) -> Result<IcoContainer, IcoError> {
    let file_len = reader.seek(SeekFrom::End(0)).map_err(IcoError::Io)?;
    if file_len < 6 {
        return Err(IcoError::Truncated("header"));
    }
    reader.seek(SeekFrom::Start(0)).map_err(IcoError::Io)?;
    let mut header = [0_u8; 6];
    read_exact(reader, &mut header, "header")?;
    if le16(&header[0..2]) != 0 || le16(&header[2..4]) != 1 {
        return Err(IcoError::InvalidHeader);
    }
    let count = usize::from(le16(&header[4..6]));
    if count == 0 || count > ICON_CONTAINER_MAX_REPRESENTATIONS {
        return Err(IcoError::InvalidCount);
    }
    let directory_end = count
        .checked_mul(16)
        .and_then(|n| n.checked_add(6))
        .ok_or(IcoError::Overflow)? as u64;
    if directory_end > file_len {
        return Err(IcoError::Truncated("directory"));
    }

    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let ordinal = index as u16;
        let mut raw = [0_u8; 16];
        read_exact(reader, &mut raw, "directory")?;
        let width = dimension_byte(raw[0]);
        let height = dimension_byte(raw[1]);
        check_dimensions(width, height, ordinal)?;
        let size = u64::from(le32(&raw[8..12]));
        let offset = u64::from(le32(&raw[12..16]));
        if size == 0 {
            return Err(IcoError::InvalidEntry(ordinal, "empty payload"));
        }
        if offset < directory_end {
            return Err(IcoError::InvalidEntry(
                ordinal,
                "payload overlaps directory",
            ));
        }
        let end = offset.checked_add(size).ok_or(IcoError::Overflow)?;
        if end > file_len {
            return Err(IcoError::Truncated("payload"));
        }
        entries.push(Entry {
            raw,
            offset,
            size,
            end,
        });
    }
    let mut intervals: Vec<_> = entries
        .iter()
        .map(|entry| (entry.offset, entry.end))
        .collect();
    intervals.sort_unstable();
    for pair in intervals.windows(2) {
        if pair[1].0 < pair[0].1 && pair[0] != pair[1] {
            return Err(IcoError::PayloadOverlap);
        }
    }

    let mut representations = Vec::with_capacity(count);
    for (index, entry) in entries.iter().enumerate() {
        let ordinal = index as u16;
        reader
            .seek(SeekFrom::Start(entry.offset))
            .map_err(IcoError::Io)?;
        let mut prefix = [0_u8; 8];
        if entry.size < 8 {
            return Err(IcoError::Truncated("payload"));
        }
        read_exact(reader, &mut prefix, "payload")?;
        let (codec, width, height, depth) = if prefix == PNG_SIGNATURE {
            let (width, height, depth) = parse_png(reader, entry.end, ordinal)?;
            (ContainerCodec::Png, width, height, Some(depth))
        } else {
            let (width, height, depth) = parse_dib(reader, entry, ordinal)?;
            (ContainerCodec::Dib, width, height, Some(depth))
        };
        check_dimensions(width, height, ordinal)?;
        if width != dimension_byte(entry.raw[0]) || height != dimension_byte(entry.raw[1]) {
            return Err(IcoError::DimensionMismatch(ordinal));
        }
        representations.push(
            ContainerRepresentation::new(ordinal, width, height, depth, codec, entry.size, None)
                .map_err(|_| IcoError::LimitExceeded(ordinal))?,
        );
    }
    let primary = choose_primary(&representations);
    let metadata =
        ContainerMetadata::new(representations, primary).map_err(|_| IcoError::Overflow)?;
    Ok(IcoContainer { metadata, entries })
}

fn choose_primary(items: &[ContainerRepresentation]) -> u16 {
    items
        .iter()
        .max_by_key(|item| {
            let area = item
                .width()
                .checked_mul(item.height())
                .expect("bounded dimensions");
            (
                area,
                item.width().max(item.height()),
                u8::from(item.codec() == ContainerCodec::Png),
                item.bit_depth().unwrap_or(0),
                std::cmp::Reverse(item.ordinal()),
            )
        })
        .expect("nonempty ICO directory")
        .ordinal()
}

pub(crate) fn parse_png<R: Read + Seek>(
    reader: &mut R,
    end: u64,
    ordinal: u16,
) -> Result<(u32, u32, u16), IcoError> {
    let mut first = true;
    let mut dimensions = None;
    let mut saw_idat = false;
    let mut idat_ended = false;
    let mut saw_plte = false;
    let mut color_type = 0_u8;
    loop {
        let position = reader.stream_position().map_err(IcoError::Io)?;
        if end.checked_sub(position).is_none_or(|left| left < 12) {
            return Err(IcoError::InvalidPng(ordinal, "missing or truncated IEND"));
        }
        let mut chunk = [0_u8; 8];
        read_exact(reader, &mut chunk, "PNG chunk")?;
        let length = u64::from(u32::from_be_bytes(
            chunk[0..4].try_into().expect("fixed slice"),
        ));
        let next = position
            .checked_add(12)
            .and_then(|p| p.checked_add(length))
            .ok_or(IcoError::Overflow)?;
        if next > end {
            return Err(IcoError::InvalidPng(ordinal, "chunk exceeds payload"));
        }
        let kind = &chunk[4..8];
        if !kind.iter().all(u8::is_ascii_alphabetic) {
            return Err(IcoError::InvalidPng(ordinal, "invalid chunk type"));
        }
        if first && (kind != b"IHDR" || length != 13) {
            return Err(IcoError::InvalidPng(ordinal, "missing IHDR"));
        }
        if !first && kind == b"IHDR" {
            return Err(IcoError::InvalidPng(ordinal, "duplicate IHDR"));
        }
        if kind != b"IDAT" && saw_idat {
            idat_ended = true;
        }
        if kind == b"IDAT" && (idat_ended || (color_type == 3 && !saw_plte)) {
            return Err(IcoError::InvalidPng(ordinal, "invalid IDAT order"));
        }
        if kind == b"PLTE" {
            if saw_plte
                || saw_idat
                || length == 0
                || length > 768
                || length % 3 != 0
                || color_type == 0
                || color_type == 4
            {
                return Err(IcoError::InvalidPng(ordinal, "invalid PLTE"));
            }
            saw_plte = true;
        }
        if kind != b"IHDR"
            && kind != b"PLTE"
            && kind != b"IDAT"
            && kind != b"IEND"
            && kind[0].is_ascii_uppercase()
        {
            return Err(IcoError::UnsupportedRepresentation(
                ordinal,
                "unknown critical PNG chunk",
            ));
        }
        first = false;
        let mut crc = crc32_update(!0, kind);
        let mut ihdr = [0_u8; 13];
        if kind == b"IHDR" {
            read_exact(reader, &mut ihdr, "PNG IHDR")?;
            crc = crc32_update(crc, &ihdr);
            let width = u32::from_be_bytes(ihdr[0..4].try_into().expect("fixed slice"));
            let height = u32::from_be_bytes(ihdr[4..8].try_into().expect("fixed slice"));
            check_dimensions(width, height, ordinal)?;
            let channels = match ihdr[9] {
                0 => 1,
                2 => 3,
                3 => 1,
                4 => 2,
                6 => 4,
                _ => return Err(IcoError::InvalidPng(ordinal, "invalid color type")),
            };
            let valid_depth = match ihdr[9] {
                0 => matches!(ihdr[8], 1 | 2 | 4 | 8 | 16),
                3 => matches!(ihdr[8], 1 | 2 | 4 | 8),
                _ => matches!(ihdr[8], 8 | 16),
            };
            if !valid_depth || ihdr[10] != 0 || ihdr[11] != 0 || ihdr[12] > 1 {
                return Err(IcoError::InvalidPng(ordinal, "invalid IHDR encoding"));
            }
            dimensions = Some((width, height, u16::from(ihdr[8]) * channels));
            color_type = ihdr[9];
        } else {
            let mut remaining = length;
            let mut buffer = [0_u8; 4096];
            while remaining > 0 {
                let count = remaining.min(buffer.len() as u64) as usize;
                read_exact(reader, &mut buffer[..count], "PNG chunk")?;
                crc = crc32_update(crc, &buffer[..count]);
                remaining -= count as u64;
            }
        }
        let mut expected = [0_u8; 4];
        read_exact(reader, &mut expected, "PNG CRC")?;
        if (!crc).to_be_bytes() != expected {
            return Err(IcoError::InvalidPng(ordinal, "CRC mismatch"));
        }
        if kind == b"IDAT" {
            saw_idat = true;
        }
        if kind == b"IEND" {
            if length != 0 || !saw_idat || next != end {
                return Err(IcoError::InvalidPng(ordinal, "invalid IEND"));
            }
            return dimensions.ok_or(IcoError::InvalidPng(ordinal, "missing IHDR"));
        }
    }
}

fn parse_dib<R: Read + Seek>(
    reader: &mut R,
    entry: &Entry,
    ordinal: u16,
) -> Result<(u32, u32, u16), IcoError> {
    let mut header = [0_u8; 40];
    reader
        .seek(SeekFrom::Start(entry.offset))
        .map_err(IcoError::Io)?;
    read_exact(reader, &mut header[..4], "DIB header")?;
    if le32(&header[..4]) != 40 {
        return Err(IcoError::UnsupportedRepresentation(
            ordinal,
            "DIB header variant",
        ));
    }
    if entry.size < 40 {
        return Err(IcoError::Truncated("DIB header"));
    }
    read_exact(reader, &mut header[4..], "DIB header")?;
    let width = i32::from_le_bytes(header[4..8].try_into().expect("fixed slice"));
    let combined_height = i32::from_le_bytes(header[8..12].try_into().expect("fixed slice"));
    if width <= 0 || combined_height <= 0 || combined_height % 2 != 0 || le16(&header[12..14]) != 1
    {
        return Err(IcoError::InvalidDib(
            ordinal,
            "invalid width, doubled height, or planes",
        ));
    }
    let width = width as u32;
    let height = (combined_height / 2) as u32;
    check_dimensions(width, height, ordinal)?;
    let depth = le16(&header[14..16]);
    if !matches!(depth, 1 | 4 | 8 | 24 | 32) {
        return Err(IcoError::UnsupportedRepresentation(
            ordinal,
            "DIB bit depth",
        ));
    }
    if le32(&header[16..20]) != 0 {
        return Err(IcoError::UnsupportedRepresentation(
            ordinal,
            "DIB compression",
        ));
    }
    let max_palette = if depth <= 8 { 1_u64 << depth } else { 0 };
    let colors_used = u64::from(le32(&header[32..36]));
    if colors_used > max_palette {
        return Err(IcoError::InvalidDib(ordinal, "palette size"));
    }
    let palette = if depth <= 8 {
        if colors_used == 0 {
            max_palette
        } else {
            colors_used
        }
    } else {
        0
    };
    let xor_bits = u64::from(width)
        .checked_mul(u64::from(depth))
        .ok_or(IcoError::Overflow)?;
    let xor_stride = xor_bits.checked_add(31).ok_or(IcoError::Overflow)? / 32 * 4;
    let and_stride = u64::from(width).checked_add(31).ok_or(IcoError::Overflow)? / 32 * 4;
    let required = palette
        .checked_mul(4)
        .and_then(|n| n.checked_add(40))
        .and_then(|n| {
            xor_stride
                .checked_mul(u64::from(height))
                .and_then(|xor| n.checked_add(xor))
        })
        .and_then(|n| {
            and_stride
                .checked_mul(u64::from(height))
                .and_then(|mask| n.checked_add(mask))
        })
        .ok_or(IcoError::Overflow)?;
    if required > entry.size {
        return Err(IcoError::Truncated("DIB XOR or AND bitmap"));
    }
    Ok((width, height, depth))
}

fn check_dimensions(width: u32, height: u32, ordinal: u16) -> Result<(), IcoError> {
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .ok_or(IcoError::Overflow)?;
    if width == 0
        || height == 0
        || width > ICON_CONTAINER_MAX_DIMENSION
        || height > ICON_CONTAINER_MAX_DIMENSION
        || bytes > crate::ICON_CONTAINER_MAX_DECODED_BYTES
    {
        return Err(IcoError::LimitExceeded(ordinal));
    }
    Ok(())
}

fn dimension_byte(value: u8) -> u32 {
    if value == 0 { 256 } else { u32::from(value) }
}
fn le16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}
fn le32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
fn read_exact(
    reader: &mut impl Read,
    bytes: &mut [u8],
    part: &'static str,
) -> Result<(), IcoError> {
    reader.read_exact(bytes).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            IcoError::Truncated(part)
        } else {
            IcoError::Io(error)
        }
    })
}
fn crc32_update(mut crc: u32, bytes: &[u8]) -> u32 {
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
    crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn chunk(kind: &[u8; 4], data: &[u8], output: &mut Vec<u8>) {
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(kind);
        output.extend_from_slice(data);
        let crc = crc32_update(crc32_update(!0, kind), data);
        output.extend_from_slice(&(!crc).to_be_bytes());
    }

    fn png(width: u32, height: u32, depth: u8) -> Vec<u8> {
        let mut bytes = PNG_SIGNATURE.to_vec();
        let mut ihdr = [0_u8; 13];
        ihdr[..4].copy_from_slice(&width.to_be_bytes());
        ihdr[4..8].copy_from_slice(&height.to_be_bytes());
        ihdr[8] = depth;
        ihdr[9] = 0;
        chunk(b"IHDR", &ihdr, &mut bytes);
        chunk(b"IDAT", &[0], &mut bytes);
        chunk(b"IEND", &[], &mut bytes);
        bytes
    }

    fn dib(width: u32, height: u32, depth: u16) -> Vec<u8> {
        let xor_stride = (width * u32::from(depth)).div_ceil(32) * 4;
        let and_stride = width.div_ceil(32) * 4;
        let palette = if depth <= 8 { 1_u32 << depth } else { 0 };
        let mut bytes = vec![0; (40 + palette * 4 + (xor_stride + and_stride) * height) as usize];
        bytes[..4].copy_from_slice(&40_u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&(width as i32).to_le_bytes());
        bytes[8..12].copy_from_slice(&((height * 2) as i32).to_le_bytes());
        bytes[12..14].copy_from_slice(&1_u16.to_le_bytes());
        bytes[14..16].copy_from_slice(&depth.to_le_bytes());
        bytes
    }

    fn ico(items: &[(u8, u8, Vec<u8>)]) -> Vec<u8> {
        let mut bytes = vec![0; 6 + 16 * items.len()];
        bytes[2..4].copy_from_slice(&1_u16.to_le_bytes());
        bytes[4..6].copy_from_slice(&(items.len() as u16).to_le_bytes());
        for (index, (width, height, payload)) in items.iter().enumerate() {
            let pos = 6 + 16 * index;
            bytes[pos] = *width;
            bytes[pos + 1] = *height;
            bytes[pos + 4..pos + 6].copy_from_slice(&1_u16.to_le_bytes());
            bytes[pos + 8..pos + 12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
            let offset = bytes.len() as u32;
            bytes[pos + 12..pos + 16].copy_from_slice(&offset.to_le_bytes());
            bytes.extend_from_slice(payload);
        }
        bytes
    }

    fn parse(bytes: &[u8]) -> Result<IcoContainer, IcoError> {
        parse_ico(&mut Cursor::new(bytes))
    }

    #[test]
    fn inventories_png_dib_and_256_directory_encoding() {
        let image = ico(&[(16, 16, png(16, 16, 8)), (0, 0, dib(256, 256, 32))]);
        let parsed = parse(&image).unwrap();
        assert_eq!(parsed.metadata().representations().len(), 2);
        assert_eq!(parsed.metadata().primary_ordinal(), 1);
        assert_eq!(parsed.metadata().primary().width(), 256);
        assert_eq!(parsed.metadata().primary().codec(), ContainerCodec::Dib);
        assert_eq!(parsed.metadata().primary().scale(), None);
    }

    #[test]
    fn inventories_each_supported_bi_rgb_depth() {
        for depth in [1, 4, 8, 24, 32] {
            let bytes = ico(&[(16, 16, dib(16, 16, depth))]);
            let parsed = parse(&bytes).unwrap();
            assert_eq!(parsed.metadata().primary().codec(), ContainerCodec::Dib);
            assert_eq!(parsed.metadata().primary().bit_depth(), Some(depth));
        }
    }

    #[test]
    fn primary_uses_each_tie_break_in_order() {
        let sizes = ico(&[
            (16, 16, png(16, 16, 8)),
            (32, 32, png(32, 32, 8)),
            (128, 128, png(128, 128, 8)),
        ]);
        assert_eq!(parse(&sizes).unwrap().metadata().primary_ordinal(), 2);
        let dimension = ico(&[(32, 16, png(32, 16, 8)), (16, 32, png(16, 32, 8))]);
        assert_eq!(parse(&dimension).unwrap().metadata().primary_ordinal(), 0);
        let codec = ico(&[(16, 16, dib(16, 16, 32)), (16, 16, png(16, 16, 8))]);
        assert_eq!(parse(&codec).unwrap().metadata().primary_ordinal(), 1);
        let depth = ico(&[(16, 16, dib(16, 16, 24)), (16, 16, dib(16, 16, 32))]);
        assert_eq!(parse(&depth).unwrap().metadata().primary_ordinal(), 1);
        let ordinal = ico(&[(16, 16, png(16, 16, 8)), (16, 16, png(16, 16, 8))]);
        assert_eq!(parse(&ordinal).unwrap().metadata().primary_ordinal(), 0);
    }

    #[test]
    fn exact_shared_interval_is_two_representations() {
        let mut bytes = ico(&[(16, 16, png(16, 16, 8)), (16, 16, png(16, 16, 8))]);
        let first = bytes[18..22].to_vec();
        bytes[34..38].copy_from_slice(&first);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.metadata().representations().len(), 2);
        assert_eq!(parsed.metadata().primary_ordinal(), 0);
    }

    #[test]
    fn accepts_exact_entry_limit_with_shared_payload() {
        let mut bytes = ico(&[(16, 16, dib(16, 16, 32))]);
        let entry = bytes[6..22].to_vec();
        let payload = bytes.split_off(22);
        bytes[4..6].copy_from_slice(&(ICON_CONTAINER_MAX_REPRESENTATIONS as u16).to_le_bytes());
        for _ in 1..ICON_CONTAINER_MAX_REPRESENTATIONS {
            bytes.extend_from_slice(&entry);
        }
        let offset = bytes.len() as u32;
        for index in 0..ICON_CONTAINER_MAX_REPRESENTATIONS {
            let position = 6 + 16 * index + 12;
            bytes[position..position + 4].copy_from_slice(&offset.to_le_bytes());
        }
        bytes.extend_from_slice(&payload);
        assert_eq!(
            parse(&bytes).unwrap().metadata().representations().len(),
            ICON_CONTAINER_MAX_REPRESENTATIONS
        );
    }

    #[test]
    fn rejects_directory_bounds_and_intervals() {
        let good = ico(&[(16, 16, dib(16, 16, 32)), (16, 16, dib(16, 16, 32))]);
        assert!(matches!(
            parse(&good[..5]),
            Err(IcoError::Truncated("header"))
        ));
        let mut count_zero = good.clone();
        count_zero[4..6].copy_from_slice(&0_u16.to_le_bytes());
        assert!(matches!(parse(&count_zero), Err(IcoError::InvalidCount)));
        let mut too_many = good.clone();
        too_many[4..6].copy_from_slice(&1025_u16.to_le_bytes());
        assert!(matches!(parse(&too_many), Err(IcoError::InvalidCount)));
        let mut cur = good.clone();
        cur[2..4].copy_from_slice(&2_u16.to_le_bytes());
        assert!(matches!(parse(&cur), Err(IcoError::InvalidHeader)));
        let mut reserved = good.clone();
        reserved[0] = 1;
        assert!(matches!(parse(&reserved), Err(IcoError::InvalidHeader)));
        let mut empty = good.clone();
        empty[14..18].copy_from_slice(&0_u32.to_le_bytes());
        assert!(matches!(parse(&empty), Err(IcoError::InvalidEntry(0, _))));
        let mut directory = good.clone();
        directory[4..6].copy_from_slice(&3_u16.to_le_bytes());
        assert!(matches!(
            parse(&directory[..40]),
            Err(IcoError::Truncated("directory"))
        ));
        let mut before = good.clone();
        before[18..22].copy_from_slice(&20_u32.to_le_bytes());
        assert!(matches!(parse(&before), Err(IcoError::InvalidEntry(0, _))));
        let mut outside = good.clone();
        outside[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            parse(&outside),
            Err(IcoError::Truncated("payload"))
        ));
        let mut overlap = good.clone();
        let first_offset = le32(&overlap[18..22]);
        overlap[34..38].copy_from_slice(&(first_offset + 4).to_le_bytes());
        assert!(matches!(parse(&overlap), Err(IcoError::PayloadOverlap)));
        assert!(matches!(
            parse(&good[..good.len() - 1]),
            Err(IcoError::Truncated("payload"))
        ));
    }

    #[test]
    fn rejects_payload_mismatch_unsupported_and_limits() {
        let mut mismatch = ico(&[(16, 16, png(32, 32, 8))]);
        assert!(matches!(
            parse(&mismatch),
            Err(IcoError::DimensionMismatch(0))
        ));
        mismatch[6] = 32;
        mismatch[7] = 32;
        assert!(parse(&mismatch).is_ok());
        let mut bad_crc = ico(&[(16, 16, png(16, 16, 8))]);
        let last = bad_crc.len() - 1;
        bad_crc[last] ^= 1;
        assert!(matches!(
            parse(&bad_crc),
            Err(IcoError::InvalidPng(0, "CRC mismatch"))
        ));
        let mut oversized = ico(&[(0, 0, dib(16, 16, 32))]);
        let offset = le32(&oversized[18..22]) as usize;
        oversized[offset + 4..offset + 8].copy_from_slice(&4097_i32.to_le_bytes());
        assert!(matches!(parse(&oversized), Err(IcoError::LimitExceeded(0))));
        let mut unsupported = ico(&[(16, 16, dib(16, 16, 32))]);
        let offset = le32(&unsupported[18..22]) as usize;
        unsupported[offset..offset + 4].copy_from_slice(&124_u32.to_le_bytes());
        assert!(matches!(
            parse(&unsupported),
            Err(IcoError::UnsupportedRepresentation(0, _))
        ));
        let mut compressed = ico(&[(16, 16, dib(16, 16, 32))]);
        let offset = le32(&compressed[18..22]) as usize;
        compressed[offset + 16..offset + 20].copy_from_slice(&3_u32.to_le_bytes());
        assert!(matches!(
            parse(&compressed),
            Err(IcoError::UnsupportedRepresentation(0, _))
        ));
        let mut short_dib = ico(&[(16, 16, dib(16, 16, 32))]);
        short_dib.truncate(short_dib.len() - 64);
        short_dib[14..18].copy_from_slice(&(40 + 16 * 16 * 4_u32).to_le_bytes());
        assert!(matches!(
            parse(&short_dib),
            Err(IcoError::Truncated("DIB XOR or AND bitmap"))
        ));
    }

    #[test]
    fn arbitrary_short_buffers_never_panic() {
        let mut state = 0x1234_5678_u32;
        for len in 0..128 {
            for _ in 0..16 {
                let mut bytes = vec![0; len];
                for byte in &mut bytes {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    *byte = state as u8;
                }
                let _ = parse(&bytes);
            }
        }
    }
}
