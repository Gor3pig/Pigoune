//! Structural ICNS inventory over an internal staged reader.

use std::{
    error::Error,
    fmt,
    io::{self, Read, Seek, SeekFrom},
};

use crate::{
    ContainerCodec, ContainerMetadata, ContainerRepresentation, ICON_CONTAINER_MAX_REPRESENTATIONS,
};

pub const ICNS_MAX_ELEMENTS: usize = 2_048;
pub const ICNS_MAX_ELEMENT_ENCODED_BYTES: u64 = 64 * 1024 * 1024;
pub const ICNS_MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
pub const ICNS_MAX_TOTAL_DECODED_BYTES: u64 = 256 * 1024 * 1024;

const PNG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const JP2: [u8; 12] = [0, 0, 0, 12, b'j', b'P', b' ', b' ', 13, 10, 135, 10];

#[derive(Debug)]
pub enum IcnsError {
    Io(io::Error),
    Invalid(&'static str),
    InvalidElement(u16, &'static str),
    KnownUnsupported(u16, [u8; 4]),
    LimitExceeded(&'static str),
    MissingRepresentation,
    LegacyPair(&'static str),
}
impl fmt::Display for IcnsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "ICNS I/O error: {e}"),
            Self::Invalid(why) => write!(f, "invalid ICNS: {why}"),
            Self::InvalidElement(n, why) => write!(f, "invalid ICNS element {n}: {why}"),
            Self::KnownUnsupported(n, kind) => write!(
                f,
                "known unsupported ICNS representation {n}: {}",
                String::from_utf8_lossy(kind)
            ),
            Self::LimitExceeded(why) => write!(f, "ICNS safety limit exceeded: {why}"),
            Self::MissingRepresentation => f.write_str("ICNS has no supported representation"),
            Self::LegacyPair(why) => write!(f, "invalid ICNS legacy color/mask pair: {why}"),
        }
    }
}
impl Error for IcnsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Io(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Jpeg2000Kind {
    Jp2,
    Codestream,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IcnsPayload {
    pub offset: u64,
    pub size: u64,
    pub kind: [u8; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IcnsPrimary {
    pub color: IcnsPayload,
    pub mask: Option<IcnsPayload>,
    pub jpeg2000_kind: Option<Jpeg2000Kind>,
    /// The crate skips four leading zeros unconditionally; prepend a disposable
    /// prefix when a valid unprefixed stream happens to begin with four zeros.
    pub add_rle_prefix: bool,
}

#[derive(Clone, Debug)]
pub struct IcnsContainer {
    metadata: ContainerMetadata,
    primary: IcnsPrimary,
    unknown_count: u16,
}
impl IcnsContainer {
    pub fn metadata(&self) -> &ContainerMetadata {
        &self.metadata
    }
    pub const fn primary(&self) -> IcnsPrimary {
        self.primary
    }
    pub const fn unknown_count(&self) -> u16 {
        self.unknown_count
    }
}

#[derive(Clone, Copy)]
struct Span {
    ordinal: u16,
    kind: [u8; 4],
    offset: u64,
    size: u64,
}
impl Span {
    fn payload(self) -> IcnsPayload {
        IcnsPayload {
            offset: self.offset,
            size: self.size,
            kind: self.kind,
        }
    }
}
#[derive(Clone, Copy)]
enum Class {
    Modern(u32, u8),
    Argb(u32, u8),
    Rgb(u32),
    Mask(u32),
    Metadata,
    Unsupported,
    Unknown,
}
fn class(kind: &[u8; 4]) -> Class {
    match kind {
        b"icp4" => Class::Modern(16, 1),
        b"ic11" => Class::Modern(32, 2),
        b"icp5" => Class::Modern(32, 1),
        b"ic12" => Class::Modern(64, 2),
        b"icp6" => Class::Modern(64, 1),
        b"ic07" => Class::Modern(128, 1),
        b"ic13" => Class::Modern(256, 2),
        b"ic08" => Class::Modern(256, 1),
        b"ic14" => Class::Modern(512, 2),
        b"ic09" => Class::Modern(512, 1),
        b"ic10" => Class::Modern(1024, 2),
        b"ic04" => Class::Argb(16, 1),
        b"ic05" => Class::Argb(32, 2),
        b"icsb" => Class::Argb(18, 1),
        b"icsB" => Class::Modern(36, 2),
        b"sb24" => Class::Modern(24, 1),
        b"SB24" => Class::Modern(48, 2),
        b"is32" => Class::Rgb(16),
        b"il32" => Class::Rgb(32),
        b"ih32" => Class::Rgb(48),
        b"it32" => Class::Rgb(128),
        b"s8mk" => Class::Mask(16),
        b"l8mk" => Class::Mask(32),
        b"h8mk" => Class::Mask(48),
        b"t8mk" => Class::Mask(128),
        b"TOC " | b"icnV" | b"info" => Class::Metadata,
        b"ICON" | b"ICN#" | b"icm#" | b"ics#" | b"ich#" | b"icm4" | b"ics4" | b"icl4" | b"ich4"
        | b"icm8" | b"ics8" | b"icl8" | b"ich8" => Class::Unsupported,
        _ => Class::Unknown,
    }
}
fn mask_for(color: [u8; 4]) -> Option<[u8; 4]> {
    match &color {
        b"is32" => Some(*b"s8mk"),
        b"il32" => Some(*b"l8mk"),
        b"ih32" => Some(*b"h8mk"),
        b"it32" => Some(*b"t8mk"),
        _ => None,
    }
}
fn color_for(mask: [u8; 4]) -> Option<[u8; 4]> {
    match &mask {
        b"s8mk" => Some(*b"is32"),
        b"l8mk" => Some(*b"il32"),
        b"h8mk" => Some(*b"ih32"),
        b"t8mk" => Some(*b"it32"),
        _ => None,
    }
}

pub fn parse_icns<R: Read + Seek>(reader: &mut R) -> Result<IcnsContainer, IcnsError> {
    let len = reader.seek(SeekFrom::End(0)).map_err(IcnsError::Io)?;
    if len > ICNS_MAX_FILE_BYTES {
        return Err(IcnsError::LimitExceeded("file size"));
    }
    if len < 8 {
        return Err(IcnsError::Invalid("truncated header"));
    }
    reader.seek(SeekFrom::Start(0)).map_err(IcnsError::Io)?;
    let mut header = [0; 8];
    read(reader, &mut header)?;
    if &header[..4] != b"icns" {
        return Err(IcnsError::Invalid("magic"));
    }
    if u64::from(be32(&header[4..])) != len {
        return Err(IcnsError::Invalid("declared size differs from file"));
    }
    let mut spans = Vec::new();
    let mut position = 8_u64;
    while position < len {
        if spans.len() == ICNS_MAX_ELEMENTS {
            return Err(IcnsError::LimitExceeded("element count"));
        }
        let remaining = len
            .checked_sub(position)
            .ok_or(IcnsError::Invalid("offset overflow"))?;
        if remaining < 8 {
            return Err(IcnsError::Invalid("truncated element header"));
        }
        reader
            .seek(SeekFrom::Start(position))
            .map_err(IcnsError::Io)?;
        let mut raw = [0; 8];
        read(reader, &mut raw)?;
        let length = u64::from(be32(&raw[4..]));
        if length < 8 {
            return Err(IcnsError::Invalid("element length below header"));
        }
        if length > ICNS_MAX_ELEMENT_ENCODED_BYTES {
            return Err(IcnsError::LimitExceeded("element encoded size"));
        }
        let end = position
            .checked_add(length)
            .ok_or(IcnsError::Invalid("element offset overflow"))?;
        if end > len {
            return Err(IcnsError::Invalid("element exceeds container"));
        }
        spans.push(Span {
            ordinal: u16::try_from(spans.len())
                .map_err(|_| IcnsError::LimitExceeded("element count"))?,
            kind: raw[..4].try_into().expect("fixed length"),
            offset: position
                .checked_add(8)
                .ok_or(IcnsError::Invalid("payload offset overflow"))?,
            size: length - 8,
        });
        position = end;
    }
    if position != len {
        return Err(IcnsError::Invalid("trailing bytes"));
    }
    let mut representations = Vec::new();
    let mut candidates = Vec::new();
    let mut unknown_count = 0_u16;
    let mut decoded_total = 0_u64;
    for span in &spans {
        let (dimension, scale, codec, depth, jpeg_kind, add_rle_prefix, mask) =
            match class(&span.kind) {
                Class::Metadata => continue,
                Class::Unknown => {
                    unknown_count = unknown_count
                        .checked_add(1)
                        .ok_or(IcnsError::LimitExceeded("unknown element count"))?;
                    continue;
                }
                Class::Unsupported => {
                    return Err(IcnsError::KnownUnsupported(span.ordinal, span.kind));
                }
                Class::Mask(dimension) => {
                    let expected = u64::from(dimension)
                        .checked_mul(u64::from(dimension))
                        .ok_or(IcnsError::Invalid("mask size overflow"))?;
                    if span.size != expected {
                        return Err(IcnsError::InvalidElement(span.ordinal, "mask size"));
                    }
                    let color = color_for(span.kind).expect("mask table");
                    if spans.iter().filter(|s| s.kind == span.kind).count() != 1
                        || spans.iter().filter(|s| s.kind == color).count() != 1
                    {
                        return Err(IcnsError::LegacyPair("missing or duplicate color/mask"));
                    }
                    continue;
                }
                Class::Rgb(dimension) => {
                    let expected_mask = mask_for(span.kind).expect("color table");
                    let colors = spans.iter().filter(|s| s.kind == span.kind).count();
                    let masks: Vec<_> = spans.iter().filter(|s| s.kind == expected_mask).collect();
                    if colors != 1 || masks.len() != 1 {
                        return Err(IcnsError::LegacyPair("missing or duplicate color/mask"));
                    }
                    let add_prefix =
                        validate_legacy(reader, *span, dimension, 3, span.kind == *b"it32")?;
                    (
                        dimension,
                        1,
                        ContainerCodec::IcnsRgb,
                        Some(24),
                        None,
                        add_prefix,
                        Some(masks[0].payload()),
                    )
                }
                Class::Argb(dimension, scale) => {
                    let (codec, depth, jpeg_kind, add_prefix) =
                        validate_image(reader, *span, dimension, true)?;
                    (dimension, scale, codec, depth, jpeg_kind, add_prefix, None)
                }
                Class::Modern(dimension, scale) => {
                    let (codec, depth, jpeg_kind, add_prefix) =
                        validate_image(reader, *span, dimension, false)?;
                    (dimension, scale, codec, depth, jpeg_kind, add_prefix, None)
                }
            };
        if dimension % u32::from(scale) != 0 {
            return Err(IcnsError::InvalidElement(
                span.ordinal,
                "scale does not divide dimensions",
            ));
        }
        if representations.len() == ICON_CONTAINER_MAX_REPRESENTATIONS {
            return Err(IcnsError::LimitExceeded("representation count"));
        }
        let decoded = u64::from(dimension)
            .checked_mul(u64::from(dimension))
            .and_then(|n| n.checked_mul(4))
            .ok_or(IcnsError::LimitExceeded("decoded size overflow"))?;
        decoded_total = decoded_total
            .checked_add(decoded)
            .ok_or(IcnsError::LimitExceeded("total decoded size overflow"))?;
        if decoded_total > ICNS_MAX_TOTAL_DECODED_BYTES {
            return Err(IcnsError::LimitExceeded("total decoded size"));
        }
        let encoded_size = mask
            .map_or(Some(span.size), |mask| span.size.checked_add(mask.size))
            .ok_or(IcnsError::LimitExceeded(
                "representation encoded size overflow",
            ))?;
        let rep = ContainerRepresentation::new(
            span.ordinal,
            dimension,
            dimension,
            depth,
            codec,
            encoded_size,
            Some(scale),
        )
        .map_err(|_| IcnsError::LimitExceeded("representation"))?;
        representations.push(rep);
        candidates.push(IcnsPrimary {
            color: span.payload(),
            mask,
            jpeg2000_kind: jpeg_kind,
            add_rle_prefix,
        });
    }
    if representations.is_empty() {
        return Err(IcnsError::MissingRepresentation);
    }
    let index = (0..representations.len())
        .max_by_key(|&i| {
            let r = &representations[i];
            let area = r
                .width()
                .checked_mul(r.height())
                .expect("bounded dimensions");
            let codec = match r.codec() {
                ContainerCodec::Png => 4,
                ContainerCodec::Jpeg2000 => 3,
                ContainerCodec::IcnsArgb => 2,
                ContainerCodec::IcnsRgb => 1,
                ContainerCodec::Dib => 0,
            };
            (
                area,
                r.width().max(r.height()),
                std::cmp::Reverse(r.scale().expect("ICNS scale")),
                codec,
                r.bit_depth().unwrap_or(0),
                std::cmp::Reverse(r.ordinal()),
            )
        })
        .expect("nonempty representations");
    let selected_ordinal = representations[index].ordinal();
    let metadata = ContainerMetadata::new(representations, selected_ordinal)
        .map_err(|_| IcnsError::Invalid("inventory"))?;
    Ok(IcnsContainer {
        metadata,
        primary: candidates[index],
        unknown_count,
    })
}

fn validate_image<R: Read + Seek>(
    reader: &mut R,
    span: Span,
    expected: u32,
    allow_argb: bool,
) -> Result<(ContainerCodec, Option<u16>, Option<Jpeg2000Kind>, bool), IcnsError> {
    let mut prefix = [0; 12];
    reader
        .seek(SeekFrom::Start(span.offset))
        .map_err(IcnsError::Io)?;
    let n = usize::try_from(span.size.min(12))
        .map_err(|_| IcnsError::Invalid("payload prefix length"))?;
    read(reader, &mut prefix[..n])?;
    if n >= 8 && prefix[..8] == PNG {
        let data_offset = span
            .offset
            .checked_add(8)
            .ok_or(IcnsError::Invalid("PNG data offset overflow"))?;
        reader
            .seek(SeekFrom::Start(data_offset))
            .map_err(IcnsError::Io)?;
        let end = span
            .offset
            .checked_add(span.size)
            .ok_or(IcnsError::Invalid("payload end overflow"))?;
        let (width, height, depth) = crate::ico::parse_png(reader, end, span.ordinal)
            .map_err(|_| IcnsError::InvalidElement(span.ordinal, "PNG structure"))?;
        if width != expected || height != expected {
            return Err(IcnsError::InvalidElement(
                span.ordinal,
                "PNG dimensions disagree with FourCC",
            ));
        }
        return Ok((ContainerCodec::Png, Some(depth), None, false));
    }
    if n >= 12 && prefix == JP2 {
        let dimensions = jp2_dimensions(reader, span)?;
        if dimensions != (expected, expected) {
            return Err(IcnsError::InvalidElement(
                span.ordinal,
                "JP2 dimensions disagree with FourCC",
            ));
        }
        return Ok((
            ContainerCodec::Jpeg2000,
            None,
            Some(Jpeg2000Kind::Jp2),
            false,
        ));
    }
    if n >= 2 && prefix[..2] == [0xff, 0x4f] {
        let dimensions = j2k_dimensions(reader, span)?;
        if dimensions != (expected, expected) {
            return Err(IcnsError::InvalidElement(
                span.ordinal,
                "J2K dimensions disagree with FourCC",
            ));
        }
        return Ok((
            ContainerCodec::Jpeg2000,
            None,
            Some(Jpeg2000Kind::Codestream),
            false,
        ));
    }
    if allow_argb && n >= 4 && &prefix[..4] == b"ARGB" {
        let data_offset = span
            .offset
            .checked_add(4)
            .ok_or(IcnsError::Invalid("ARGB data offset overflow"))?;
        let data_size = span
            .size
            .checked_sub(4)
            .ok_or(IcnsError::Invalid("ARGB payload truncated"))?;
        validate_rle(reader, data_offset, data_size, expected, 4, false)?;
        return Ok((ContainerCodec::IcnsArgb, Some(32), None, false));
    }
    Err(IcnsError::InvalidElement(
        span.ordinal,
        "unrecognized image codec",
    ))
}

fn validate_legacy<R: Read + Seek>(
    reader: &mut R,
    span: Span,
    dimension: u32,
    channels: u8,
    prefix_allowed: bool,
) -> Result<bool, IcnsError> {
    let mut start = [0; 4];
    reader
        .seek(SeekFrom::Start(span.offset))
        .map_err(IcnsError::Io)?;
    let n =
        usize::try_from(span.size.min(4)).map_err(|_| IcnsError::Invalid("RLE prefix length"))?;
    read(reader, &mut start[..n])?;
    let unprefixed =
        validate_rle(reader, span.offset, span.size, dimension, channels, false).is_ok();
    let prefixed = prefix_allowed
        && n == 4
        && start == [0; 4]
        && validate_rle(reader, span.offset, span.size, dimension, channels, true).is_ok();
    match (unprefixed, prefixed) {
        (true, true) => Err(IcnsError::InvalidElement(
            span.ordinal,
            "ambiguous RLE prefix",
        )),
        (false, false) => Err(IcnsError::InvalidElement(
            span.ordinal,
            "invalid RLE stream",
        )),
        (true, false) => Ok(n == 4 && start == [0; 4]),
        (false, true) => Ok(false),
    }
}

fn validate_rle<R: Read + Seek>(
    reader: &mut R,
    offset: u64,
    size: u64,
    dimension: u32,
    channels: u8,
    skip_prefix: bool,
) -> Result<(), IcnsError> {
    let start = offset
        .checked_add(if skip_prefix { 4 } else { 0 })
        .ok_or(IcnsError::Invalid("RLE offset overflow"))?;
    let length = size
        .checked_sub(if skip_prefix { 4 } else { 0 })
        .ok_or(IcnsError::Invalid("RLE prefix truncated"))?;
    reader.seek(SeekFrom::Start(start)).map_err(IcnsError::Io)?;
    let mut input = reader.take(length);
    let pixels = u64::from(dimension)
        .checked_mul(u64::from(dimension))
        .ok_or(IcnsError::Invalid("RLE pixel count overflow"))?;
    for _ in 0..channels {
        let mut decoded = 0_u64;
        while decoded < pixels {
            let mut control = [0];
            read(&mut input, &mut control)?;
            let run = if control[0] < 128 {
                u64::from(control[0]) + 1
            } else {
                u64::from(control[0]) - 125
            };
            decoded = decoded
                .checked_add(run)
                .ok_or(IcnsError::Invalid("RLE run overflow"))?;
            if decoded > pixels {
                return Err(IcnsError::Invalid("RLE run exceeds channel"));
            }
            if control[0] < 128 {
                let mut buf = [0; 128];
                read(
                    &mut input,
                    &mut buf[..usize::try_from(run)
                        .map_err(|_| IcnsError::Invalid("RLE run conversion"))?],
                )?;
            } else {
                let mut value = [0];
                read(&mut input, &mut value)?;
            }
        }
    }
    if input.limit() != 0 {
        return Err(IcnsError::Invalid("RLE trailing bytes"));
    }
    Ok(())
}

fn jp2_dimensions<R: Read + Seek>(reader: &mut R, span: Span) -> Result<(u32, u32), IcnsError> {
    let end = span
        .offset
        .checked_add(span.size)
        .ok_or(IcnsError::Invalid("JP2 end overflow"))?;
    let mut pos = span.offset;
    let mut found = None;
    let mut saw_ftyp = false;
    let mut saw_codestream = false;
    while pos < end {
        if end - pos < 8 {
            return Err(IcnsError::InvalidElement(span.ordinal, "JP2 box header"));
        }
        reader.seek(SeekFrom::Start(pos)).map_err(IcnsError::Io)?;
        let mut box_header = [0; 8];
        read(reader, &mut box_header)?;
        let size = u64::from(be32(&box_header[..4]));
        if size < 8 {
            return Err(IcnsError::InvalidElement(span.ordinal, "JP2 box length"));
        }
        let next = pos
            .checked_add(size)
            .ok_or(IcnsError::Invalid("JP2 box overflow"))?;
        if next > end {
            return Err(IcnsError::InvalidElement(
                span.ordinal,
                "JP2 box exceeds payload",
            ));
        }
        if &box_header[4..] == b"ftyp" {
            if size < 16 {
                return Err(IcnsError::InvalidElement(span.ordinal, "JP2 file type box"));
            }
            saw_ftyp = true;
        }
        if &box_header[4..] == b"jp2c" {
            if size < 10 {
                return Err(IcnsError::InvalidElement(
                    span.ordinal,
                    "empty JP2 codestream",
                ));
            }
            let data_offset = pos
                .checked_add(8)
                .ok_or(IcnsError::Invalid("JP2 codestream offset overflow"))?;
            reader
                .seek(SeekFrom::Start(data_offset))
                .map_err(IcnsError::Io)?;
            let mut soc = [0; 2];
            read(reader, &mut soc)?;
            if soc != [0xff, 0x4f] {
                return Err(IcnsError::InvalidElement(
                    span.ordinal,
                    "JP2 codestream signature",
                ));
            }
            saw_codestream = true;
        }
        if &box_header[4..] == b"jp2h" {
            let mut inner = pos
                .checked_add(8)
                .ok_or(IcnsError::Invalid("JP2 subbox offset overflow"))?;
            while inner < next {
                if next - inner < 8 {
                    return Err(IcnsError::InvalidElement(span.ordinal, "JP2 subbox header"));
                }
                reader.seek(SeekFrom::Start(inner)).map_err(IcnsError::Io)?;
                let mut h = [0; 8];
                read(reader, &mut h)?;
                let n = u64::from(be32(&h[..4]));
                if n < 8 || inner.checked_add(n).is_none_or(|v| v > next) {
                    return Err(IcnsError::InvalidElement(span.ordinal, "JP2 subbox length"));
                }
                if &h[4..] == b"ihdr" {
                    if found.is_some() {
                        return Err(IcnsError::InvalidElement(
                            span.ordinal,
                            "duplicate JP2 image header",
                        ));
                    }
                    if n < 16 {
                        return Err(IcnsError::InvalidElement(span.ordinal, "JP2 image header"));
                    }
                    let mut dimensions = [0; 8];
                    read(reader, &mut dimensions)?;
                    found = Some((be32(&dimensions[4..]), be32(&dimensions[..4])));
                }
                inner = inner
                    .checked_add(n)
                    .ok_or(IcnsError::Invalid("JP2 subbox offset overflow"))?;
            }
        }
        pos = next;
    }
    if !saw_ftyp || !saw_codestream || found.is_none() {
        return Err(IcnsError::InvalidElement(
            span.ordinal,
            "incomplete JP2 structure",
        ));
    }
    found.ok_or(IcnsError::InvalidElement(
        span.ordinal,
        "missing JP2 image header",
    ))
}
fn j2k_dimensions<R: Read + Seek>(reader: &mut R, span: Span) -> Result<(u32, u32), IcnsError> {
    if span.size < 42 {
        return Err(IcnsError::InvalidElement(span.ordinal, "J2K SIZ header"));
    }
    reader
        .seek(SeekFrom::Start(
            span.offset
                .checked_add(2)
                .ok_or(IcnsError::Invalid("J2K data offset overflow"))?,
        ))
        .map_err(IcnsError::Io)?;
    let mut h = [0; 40];
    read(reader, &mut h)?;
    let segment_length = u64::from(u16::from_be_bytes([h[2], h[3]]));
    let components = u64::from(u16::from_be_bytes([h[38], h[39]]));
    let expected_segment_length = components.checked_mul(3).and_then(|n| n.checked_add(38));
    if h[..2] != [0xff, 0x51]
        || components == 0
        || expected_segment_length != Some(segment_length)
        || segment_length.checked_add(4).is_none_or(|n| n > span.size)
    {
        return Err(IcnsError::InvalidElement(span.ordinal, "J2K SIZ marker"));
    }
    let xsiz = be32(&h[6..10]);
    let ysiz = be32(&h[10..14]);
    let xosiz = be32(&h[14..18]);
    let yosiz = be32(&h[18..22]);
    let width = xsiz
        .checked_sub(xosiz)
        .ok_or(IcnsError::InvalidElement(span.ordinal, "J2K width"))?;
    let height = ysiz
        .checked_sub(yosiz)
        .ok_or(IcnsError::InvalidElement(span.ordinal, "J2K height"))?;
    Ok((width, height))
}
fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().expect("four bytes"))
}
fn read(reader: &mut impl Read, bytes: &mut [u8]) -> Result<(), IcnsError> {
    reader.read_exact(bytes).map_err(IcnsError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0_u32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 0 {
                    crc >> 1
                } else {
                    (crc >> 1) ^ 0xedb8_8320
                };
            }
        }
        !crc
    }
    fn chunk(kind: &[u8; 4], data: &[u8], result: &mut Vec<u8>) {
        result.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
        let start = result.len();
        result.extend_from_slice(kind);
        result.extend_from_slice(data);
        result.extend_from_slice(&crc32(&result[start..]).to_be_bytes());
    }
    fn png(dimension: u32) -> Vec<u8> {
        let mut result = PNG.to_vec();
        let mut ihdr = [0; 13];
        ihdr[..4].copy_from_slice(&dimension.to_be_bytes());
        ihdr[4..8].copy_from_slice(&dimension.to_be_bytes());
        ihdr[8] = 8;
        ihdr[9] = 6;
        chunk(b"IHDR", &ihdr, &mut result);
        chunk(b"IDAT", &[1], &mut result);
        chunk(b"IEND", &[], &mut result);
        result
    }
    fn file(elements: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut bytes = b"icns".to_vec();
        bytes.extend_from_slice(&[0; 4]);
        for (kind, payload) in elements {
            bytes.extend_from_slice(kind);
            bytes.extend_from_slice(&u32::try_from(payload.len() + 8).unwrap().to_be_bytes());
            bytes.extend_from_slice(payload);
        }
        let length = u32::try_from(bytes.len()).unwrap();
        bytes[4..8].copy_from_slice(&length.to_be_bytes());
        bytes
    }
    fn parse(bytes: &[u8]) -> Result<IcnsContainer, IcnsError> {
        parse_icns(&mut Cursor::new(bytes))
    }
    fn rle(dimension: u32, prefix: bool) -> Vec<u8> {
        let mut bytes = if prefix { vec![0; 4] } else { Vec::new() };
        let pixels = dimension * dimension;
        for value in [12, 34, 56] {
            append_runs(&mut bytes, pixels, value);
        }
        bytes
    }
    fn append_runs(bytes: &mut Vec<u8>, pixels: u32, value: u8) {
        let mut left = pixels;
        while left > 0 {
            let run = left.min(130);
            if run >= 3 {
                bytes.extend_from_slice(&[(run + 125) as u8, value]);
            } else {
                bytes.push((run - 1) as u8);
                bytes.extend(std::iter::repeat_n(value, run as usize));
            }
            left -= run;
        }
    }
    fn pair(color: [u8; 4], mask: [u8; 4], dimension: u32, prefix: bool) -> Vec<u8> {
        file(&[
            (color, rle(dimension, prefix)),
            (mask, vec![200; (dimension * dimension) as usize]),
        ])
    }

    #[test]
    fn rejects_bad_envelopes_and_element_lengths() {
        assert!(matches!(
            parse(b"icn"),
            Err(IcnsError::Invalid("truncated header"))
        ));
        assert!(matches!(
            parse(b"nope\0\0\0\x08"),
            Err(IcnsError::Invalid("magic"))
        ));
        let mut bytes = file(&[(*b"icp4", png(16))]);
        bytes[7] += 1;
        assert!(matches!(
            parse(&bytes),
            Err(IcnsError::Invalid("declared size differs from file"))
        ));
        let mut bytes = file(&[(*b"icp4", png(16))]);
        bytes[12..16].copy_from_slice(&7_u32.to_be_bytes());
        assert!(matches!(
            parse(&bytes),
            Err(IcnsError::Invalid("element length below header"))
        ));
        bytes[12..16].copy_from_slice(&9999_u32.to_be_bytes());
        assert!(matches!(
            parse(&bytes),
            Err(IcnsError::Invalid("element exceeds container"))
        ));
        let bytes = file(&vec![(*b"TOC ", vec![]); ICNS_MAX_ELEMENTS + 1]);
        assert!(matches!(
            parse(&bytes),
            Err(IcnsError::LimitExceeded("element count"))
        ));
    }

    #[test]
    fn modern_inventory_uses_physical_ordinals_and_scale() {
        let bytes = file(&[
            (*b"icp4", png(16)),
            (*b"TOC ", vec![1]),
            (*b"zzzz", vec![2]),
            (*b"ic11", png(32)),
            (*b"icnV", vec![3]),
            (*b"ic09", png(512)),
            (*b"ic14", png(512)),
        ]);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.unknown_count(), 1);
        assert_eq!(
            parsed
                .metadata()
                .representations()
                .iter()
                .map(ContainerRepresentation::ordinal)
                .collect::<Vec<_>>(),
            [0, 3, 5, 6]
        );
        assert_eq!(parsed.metadata().primary_ordinal(), 5);
        assert_eq!(parsed.metadata().primary().scale(), Some(1));
        assert_eq!(parsed.metadata().representations()[1].scale(), Some(2));
        assert_eq!(parsed.metadata().representations()[1].width(), 32);
        assert!(matches!(
            parse(&file(&[(*b"icp4", png(32))])),
            Err(IcnsError::InvalidElement(
                _,
                "PNG dimensions disagree with FourCC"
            ))
        ));
    }

    #[test]
    fn recognizes_jp2_and_j2k_and_checks_dimensions() {
        let mut jp2 = JP2.to_vec();
        jp2.extend_from_slice(&16_u32.to_be_bytes());
        jp2.extend_from_slice(b"ftyp");
        jp2.extend_from_slice(b"jp2 \0\0\0\0");
        jp2.extend_from_slice(&24_u32.to_be_bytes());
        jp2.extend_from_slice(b"jp2h");
        jp2.extend_from_slice(&16_u32.to_be_bytes());
        jp2.extend_from_slice(b"ihdr");
        jp2.extend_from_slice(&16_u32.to_be_bytes());
        jp2.extend_from_slice(&16_u32.to_be_bytes());
        jp2.extend_from_slice(&10_u32.to_be_bytes());
        jp2.extend_from_slice(b"jp2c");
        jp2.extend_from_slice(&[0xff, 0x4f]);
        assert_eq!(
            parse(&file(&[(*b"icp4", jp2.clone())]))
                .unwrap()
                .metadata()
                .primary()
                .codec(),
            ContainerCodec::Jpeg2000
        );
        let mut empty_codestream = jp2.clone();
        empty_codestream.truncate(empty_codestream.len() - 2);
        empty_codestream[52..56].copy_from_slice(&8_u32.to_be_bytes());
        assert!(parse(&file(&[(*b"icp4", empty_codestream)])).is_err());
        jp2[47] = 32;
        assert!(parse(&file(&[(*b"icp4", jp2)])).is_err());
        let mut j2k = vec![0xff, 0x4f, 0xff, 0x51, 0, 41, 0, 0];
        j2k.extend_from_slice(&16_u32.to_be_bytes());
        j2k.extend_from_slice(&16_u32.to_be_bytes());
        j2k.extend_from_slice(&[0; 26]);
        j2k[41] = 1;
        j2k.extend_from_slice(&[7, 1, 1]);
        assert_eq!(
            parse(&file(&[(*b"icp4", j2k.clone())]))
                .unwrap()
                .primary()
                .jpeg2000_kind,
            Some(Jpeg2000Kind::Codestream)
        );
        j2k[11] = 17;
        assert!(parse(&file(&[(*b"icp4", j2k)])).is_err());
    }

    #[test]
    fn legacy_pairs_are_exact_and_rle_is_complete() {
        for (color, mask, dimension, prefix) in [
            (*b"is32", *b"s8mk", 16, false),
            (*b"il32", *b"l8mk", 32, false),
            (*b"ih32", *b"h8mk", 48, false),
            (*b"it32", *b"t8mk", 128, true),
        ] {
            let parsed = parse(&pair(color, mask, dimension, prefix)).unwrap();
            assert_eq!(parsed.metadata().primary().codec(), ContainerCodec::IcnsRgb);
            assert_eq!(parsed.metadata().primary().width(), dimension);
            assert_eq!(parsed.primary().mask.unwrap().kind, mask);
            assert_eq!(
                parsed.metadata().primary().encoded_size(),
                parsed
                    .primary()
                    .color
                    .size
                    .checked_add(parsed.primary().mask.unwrap().size)
                    .unwrap()
            );
        }
        let color = rle(16, false);
        let mask = vec![255; 256];
        assert!(matches!(
            parse(&file(&[(*b"is32", color.clone())])),
            Err(IcnsError::LegacyPair(_))
        ));
        assert!(matches!(
            parse(&file(&[(*b"s8mk", mask.clone())])),
            Err(IcnsError::LegacyPair(_))
        ));
        assert!(matches!(
            parse(&file(&[
                (*b"is32", color.clone()),
                (*b"is32", color.clone()),
                (*b"s8mk", mask.clone())
            ])),
            Err(IcnsError::LegacyPair(_))
        ));
        assert!(matches!(
            parse(&file(&[
                (*b"is32", color.clone()),
                (*b"s8mk", mask.clone()),
                (*b"s8mk", mask.clone())
            ])),
            Err(IcnsError::LegacyPair(_))
        ));
        let mut truncated = color.clone();
        truncated.pop();
        assert!(parse(&file(&[(*b"is32", truncated), (*b"s8mk", mask.clone())])).is_err());
        let mut extra = color;
        extra.push(1);
        assert!(parse(&file(&[(*b"is32", extra), (*b"s8mk", mask)])).is_err());
    }

    #[test]
    fn unprefixed_rle_starting_with_zeros_is_normalized_for_legacy_decoder() {
        for (color, mask, dimension) in [(*b"is32", *b"s8mk", 16), (*b"it32", *b"t8mk", 128)] {
            let pixels = dimension * dimension;
            let mut stream = vec![0, 0, 0, 0];
            append_runs(&mut stream, pixels - 2, 12);
            append_runs(&mut stream, pixels, 34);
            append_runs(&mut stream, pixels, 56);
            let bytes = file(&[(color, stream), (mask, vec![128; pixels as usize])]);
            assert!(parse(&bytes).unwrap().primary().add_rle_prefix);
        }
        let mut broken = rle(128, true);
        broken.truncate(broken.len() - 1);
        assert!(
            parse(&file(&[
                (*b"it32", broken),
                (*b"t8mk", vec![128; 128 * 128])
            ]))
            .is_err()
        );
    }

    #[test]
    fn specialized_types_follow_their_actual_encoding_classes() {
        for (kind, dimension) in [(*b"ic04", 16), (*b"ic05", 32), (*b"icsb", 18)] {
            let mut payload = b"ARGB".to_vec();
            for value in [128, 12, 34, 56] {
                append_runs(&mut payload, dimension * dimension, value);
            }
            assert_eq!(
                parse(&file(&[(kind, payload)]))
                    .unwrap()
                    .metadata()
                    .primary()
                    .codec(),
                ContainerCodec::IcnsArgb
            );
        }
        for (kind, dimension) in [(*b"icsB", 36), (*b"sb24", 24), (*b"SB24", 48)] {
            assert_eq!(
                parse(&file(&[(kind, png(dimension))]))
                    .unwrap()
                    .metadata()
                    .primary()
                    .codec(),
                ContainerCodec::Png
            );
        }
        assert_eq!(
            parse(&file(&[(*b"ic04", png(16))]))
                .unwrap()
                .metadata()
                .primary()
                .codec(),
            ContainerCodec::Png
        );
        assert!(parse(&file(&[(*b"icsB", b"ARGB".to_vec())])).is_err());
    }

    #[test]
    fn cumulative_decode_budget_is_checked() {
        let payload = png(1024);
        let elements = vec![(*b"ic10", payload); 65];
        assert!(matches!(
            parse(&file(&elements)),
            Err(IcnsError::LimitExceeded("total decoded size"))
        ));
    }

    #[test]
    fn known_historical_types_are_errors() {
        for kind in [*b"ICON", *b"ICN#", *b"icm4", *b"ics8"] {
            assert!(matches!(
                parse(&file(&[(kind, vec![1])])),
                Err(IcnsError::KnownUnsupported(_, _))
            ));
        }
    }

    struct SparseReader {
        len: u64,
        header: Vec<u8>,
        pos: u64,
    }
    impl Read for SparseReader {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            let start = usize::try_from(self.pos).unwrap_or(usize::MAX);
            if start >= self.header.len() {
                return Ok(0);
            }
            let n = out.len().min(self.header.len() - start);
            out[..n].copy_from_slice(&self.header[start..start + n]);
            self.pos += n as u64;
            Ok(n)
        }
    }
    impl Seek for SparseReader {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            let target = match from {
                SeekFrom::Start(n) => i128::from(n),
                SeekFrom::End(n) => i128::from(self.len) + i128::from(n),
                SeekFrom::Current(n) => i128::from(self.pos) + i128::from(n),
            };
            self.pos = u64::try_from(target)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "negative seek"))?;
            Ok(self.pos)
        }
    }
    #[test]
    fn size_limits_are_checked_without_allocating_payloads() {
        let mut too_large = SparseReader {
            len: ICNS_MAX_FILE_BYTES + 1,
            header: vec![],
            pos: 0,
        };
        assert!(matches!(
            parse_icns(&mut too_large),
            Err(IcnsError::LimitExceeded("file size"))
        ));
        let len = ICNS_MAX_ELEMENT_ENCODED_BYTES + 9;
        let mut header = b"icns".to_vec();
        header.extend_from_slice(&(len as u32).to_be_bytes());
        header.extend_from_slice(b"zzzz");
        header.extend_from_slice(&((ICNS_MAX_ELEMENT_ENCODED_BYTES + 1) as u32).to_be_bytes());
        let mut too_large_element = SparseReader {
            len,
            header,
            pos: 0,
        };
        assert!(matches!(
            parse_icns(&mut too_large_element),
            Err(IcnsError::LimitExceeded("element encoded size"))
        ));
    }
}
