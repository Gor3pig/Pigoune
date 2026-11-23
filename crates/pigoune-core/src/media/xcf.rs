use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

const SIGNATURE: &[u8; 9] = b"gimp xcf ";
const VERSION_ZERO_TAG: &[u8; 5] = b"file\0";
const HEADER_TAG_LENGTH: usize = 14;
const FIRST_PRECISION_VERSION: u32 = 4;
const FIRST_CURRENT_PRECISION_VERSION: u32 = 7;
const FIRST_LONG_POINTER_VERSION: u32 = 11;
const NEWEST_KNOWN_VERSION: u32 = 13;
const LARGEST_SIDE: u32 = 30_000;
const MOST_PIXELS: usize = 64_000_000;
const MOST_SAMPLE_BYTES: usize = 1 << 30;
const MOST_LAYERS: usize = 4096;
const MOST_NAME_BYTES: usize = 4096;
const MOST_PALETTE_COLORS: usize = 256;
const MOST_PROPERTY_BYTES: u32 = 1 << 20;
const MOST_PATH_DEPTH: usize = 64;
const TILE_SIDE: usize = 64;
const EIGHT_BITS: usize = 255;
const RUN_LENGTH_LONG: usize = 128;
const COMPRESSED_SLACK: usize = 64;

const PROP_END: u32 = 0;
const PROP_COLORMAP: u32 = 1;
const PROP_OPACITY: u32 = 6;
const PROP_MODE: u32 = 7;
const PROP_VISIBLE: u32 = 8;
const PROP_APPLY_MASK: u32 = 11;
const PROP_OFFSETS: u32 = 15;
const PROP_COMPRESSION: u32 = 17;
const PROP_GROUP_ITEM: u32 = 29;
const PROP_ITEM_PATH: u32 = 30;
const PROP_FLOAT_OPACITY: u32 = 33;
const PROP_COMPOSITE_MODE: u32 = 35;
const PROP_COMPOSITE_SPACE: u32 = 36;
const PROP_BLEND_SPACE: u32 = 37;
const PROP_FILTERS: u32 = 44;

const COMPRESSION_NONE: u8 = 0;
const COMPRESSION_RUN_LENGTH: u8 = 1;
const COMPRESSION_ZLIB: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum XcfError {
    #[error("the file is not a readable GIMP image")]
    Unreadable,
    #[error("the image is too large to be read")]
    TooLarge,
    #[error("this kind of GIMP image cannot be read")]
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseType {
    Rgb,
    Gray,
    Indexed,
}

impl BaseType {
    fn from_code(code: u32) -> Result<Self, XcfError> {
        match code {
            0 => Ok(Self::Rgb),
            1 => Ok(Self::Gray),
            2 => Ok(Self::Indexed),
            _ => Err(XcfError::Unsupported),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    U8,
    U16,
    U32,
    Half,
    Float,
    Double,
}

impl SampleFormat {
    fn bytes(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 | Self::Half => 2,
            Self::U32 | Self::Float => 4,
            Self::Double => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Precision {
    pub format: SampleFormat,
    pub linear: bool,
}

impl Precision {
    const EIGHT_BITS: Self = Self {
        format: SampleFormat::U8,
        linear: false,
    };

    fn from_code(version: u32, code: u32) -> Result<Self, XcfError> {
        if version < FIRST_PRECISION_VERSION {
            return Ok(Self::EIGHT_BITS);
        }
        if version < FIRST_CURRENT_PRECISION_VERSION {
            return Err(XcfError::Unsupported);
        }
        let format = match code / 100 {
            1 => SampleFormat::U8,
            2 => SampleFormat::U16,
            3 => SampleFormat::U32,
            5 => SampleFormat::Half,
            6 => SampleFormat::Float,
            7 => SampleFormat::Double,
            _ => return Err(XcfError::Unsupported),
        };
        match code % 100 {
            0 => Ok(Self {
                format,
                linear: true,
            }),
            50 | 75 => Ok(Self {
                format,
                linear: false,
            }),
            _ => Err(XcfError::Unsupported),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Rgb,
    RgbAlpha,
    Gray,
    GrayAlpha,
    Indexed,
    IndexedAlpha,
}

impl LayerKind {
    fn from_code(code: u32) -> Result<Self, XcfError> {
        match code {
            0 => Ok(Self::Rgb),
            1 => Ok(Self::RgbAlpha),
            2 => Ok(Self::Gray),
            3 => Ok(Self::GrayAlpha),
            4 => Ok(Self::Indexed),
            5 => Ok(Self::IndexedAlpha),
            _ => Err(XcfError::Unsupported),
        }
    }

    #[must_use]
    pub fn channels(self) -> usize {
        match self {
            Self::Gray | Self::Indexed => 1,
            Self::GrayAlpha | Self::IndexedAlpha => 2,
            Self::Rgb => 3,
            Self::RgbAlpha => 4,
        }
    }

    #[must_use]
    pub fn has_alpha(self) -> bool {
        matches!(self, Self::RgbAlpha | Self::GrayAlpha | Self::IndexedAlpha)
    }

    #[must_use]
    pub fn is_indexed(self) -> bool {
        matches!(self, Self::Indexed | Self::IndexedAlpha)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerRole {
    Layer,
    Group,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerInfo {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub kind: LayerKind,
    pub offset_x: i32,
    pub offset_y: i32,
    pub opacity: f32,
    pub visible: bool,
    pub mode: u32,
    pub blend_space: i32,
    pub composite_space: i32,
    pub composite_mode: i32,
    pub role: LayerRole,
    pub path: Vec<u32>,
    pub apply_mask: bool,
    pub has_filters: bool,
    hierarchy_offset: u64,
    mask_offset: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XcfDocument {
    pub version: u32,
    pub width: usize,
    pub height: usize,
    pub base_type: BaseType,
    pub precision: Precision,
    pub palette: Vec<[u8; 3]>,
    pub layers: Vec<LayerInfo>,
    compression: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Samples {
    Eight(Vec<u8>),
    Sixteen(Vec<u16>),
    Float(Vec<f32>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampleGrid {
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub samples: Samples,
}

struct Stream<R> {
    reader: R,
    length: u64,
    long_pointers: bool,
}

impl<R: Read + Seek> Stream<R> {
    fn new(mut reader: R) -> Result<Self, XcfError> {
        let length = reader
            .seek(SeekFrom::End(0))
            .map_err(|_| XcfError::Unreadable)?;
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|_| XcfError::Unreadable)?;
        Ok(Self {
            reader,
            length,
            long_pointers: false,
        })
    }

    fn position(&mut self) -> Result<u64, XcfError> {
        self.reader
            .stream_position()
            .map_err(|_| XcfError::Unreadable)
    }

    fn remaining(&mut self) -> Result<u64, XcfError> {
        Ok(self.length.saturating_sub(self.position()?))
    }

    fn seek_to(&mut self, position: u64) -> Result<(), XcfError> {
        if position >= self.length {
            return Err(XcfError::Unreadable);
        }
        self.reader
            .seek(SeekFrom::Start(position))
            .map_err(|_| XcfError::Unreadable)?;
        Ok(())
    }

    fn skip(&mut self, count: u64) -> Result<(), XcfError> {
        if count > self.remaining()? {
            return Err(XcfError::Unreadable);
        }
        let target = self
            .position()?
            .checked_add(count)
            .ok_or(XcfError::Unreadable)?;
        self.reader
            .seek(SeekFrom::Start(target))
            .map_err(|_| XcfError::Unreadable)?;
        Ok(())
    }

    fn take(&mut self, count: usize) -> Result<Vec<u8>, XcfError> {
        let wanted = u64::try_from(count).map_err(|_| XcfError::Unreadable)?;
        if wanted > self.remaining()? {
            return Err(XcfError::Unreadable);
        }
        let mut bytes = vec![0_u8; count];
        self.reader
            .read_exact(&mut bytes)
            .map_err(|_| XcfError::Unreadable)?;
        Ok(bytes)
    }

    fn take_at_most(&mut self, count: usize) -> Result<Vec<u8>, XcfError> {
        let available = self.remaining()?;
        let count = usize::try_from(available).map_or(count, |available| count.min(available));
        self.take(count)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], XcfError> {
        let mut bytes = [0_u8; N];
        self.reader
            .read_exact(&mut bytes)
            .map_err(|_| XcfError::Unreadable)?;
        Ok(bytes)
    }

    fn u32(&mut self) -> Result<u32, XcfError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn size(&mut self) -> Result<usize, XcfError> {
        usize::try_from(self.u32()?).map_err(|_| XcfError::TooLarge)
    }

    fn pointer(&mut self) -> Result<u64, XcfError> {
        if self.long_pointers {
            Ok(u64::from_be_bytes(self.array()?))
        } else {
            Ok(u64::from(self.u32()?))
        }
    }

    fn string(&mut self) -> Result<String, XcfError> {
        let length = self.size()?;
        if length > MOST_NAME_BYTES {
            return Err(XcfError::Unreadable);
        }
        let bytes = self.take(length)?;
        let text = bytes.strip_suffix(&[0]).unwrap_or(&bytes);
        Ok(String::from_utf8_lossy(text).into_owned())
    }

    fn property_header(&mut self) -> Result<(u32, u32), XcfError> {
        let code = self.u32()?;
        Ok((code, self.u32()?))
    }

    fn property_payload(&mut self, length: u32) -> Result<Vec<u8>, XcfError> {
        if length > MOST_PROPERTY_BYTES {
            return Err(XcfError::Unreadable);
        }
        self.take(usize::try_from(length).map_err(|_| XcfError::Unreadable)?)
    }
}

struct Payload<'a>(&'a [u8]);

impl Payload<'_> {
    fn u32_at(&self, index: usize) -> Option<u32> {
        let start = index.checked_mul(4)?;
        let bytes = self.0.get(start..start.checked_add(4)?)?;
        Some(u32::from_be_bytes(bytes.try_into().ok()?))
    }

    fn i32_at(&self, index: usize) -> Option<i32> {
        self.u32_at(index).map(u32::cast_signed)
    }

    fn f32_at(&self, index: usize) -> Option<f32> {
        self.u32_at(index).map(f32::from_bits)
    }

    fn count(&self) -> usize {
        self.0.len() / 4
    }
}

#[must_use]
pub fn is_gimp_file(path: &Path) -> bool {
    let mut tag = [0_u8; HEADER_TAG_LENGTH];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut tag))
        .is_ok()
        && tag_version(&tag).is_some()
}

#[must_use]
pub fn gimp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let tag = bytes.get(..HEADER_TAG_LENGTH)?;
    tag_version(tag)?;
    let width = u32::from_be_bytes(bytes.get(14..18)?.try_into().ok()?);
    let height = u32::from_be_bytes(bytes.get(18..22)?.try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

fn tag_version(tag: &[u8]) -> Option<u32> {
    let rest = tag.strip_prefix(SIGNATURE)?;
    if rest == VERSION_ZERO_TAG {
        return Some(0);
    }
    let digits = rest.strip_prefix(b"v")?.strip_suffix(&[0])?;
    if digits.len() != 3 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

pub struct XcfReader<R> {
    stream: Stream<R>,
    document: XcfDocument,
}

impl XcfReader<BufReader<File>> {
    pub fn open(path: &Path) -> Result<Self, XcfError> {
        let file = File::open(path).map_err(|_| XcfError::Unreadable)?;
        Self::new(BufReader::new(file))
    }
}

impl<'a> XcfReader<Cursor<&'a [u8]>> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, XcfError> {
        Self::new(Cursor::new(bytes))
    }
}

impl<R: Read + Seek> XcfReader<R> {
    pub fn new(reader: R) -> Result<Self, XcfError> {
        let mut stream = Stream::new(reader)?;
        let document = read_document(&mut stream)?;
        Ok(Self { stream, document })
    }

    #[must_use]
    pub fn document(&self) -> &XcfDocument {
        &self.document
    }

    pub fn layer_pixels(&mut self, index: usize) -> Result<SampleGrid, XcfError> {
        let layer = self
            .document
            .layers
            .get(index)
            .ok_or(XcfError::Unreadable)?;
        let (width, height, kind, offset) = (
            layer.width,
            layer.height,
            layer.kind,
            layer.hierarchy_offset,
        );
        let format = self.document.sample_format(kind);
        let compression = self.document.compression;
        read_grid(
            &mut self.stream,
            &GridRequest {
                offset,
                width,
                height,
                channels: kind.channels(),
                format,
                compression,
            },
        )
    }

    pub fn mask_pixels(&mut self, index: usize) -> Result<Option<SampleGrid>, XcfError> {
        let layer = self
            .document
            .layers
            .get(index)
            .ok_or(XcfError::Unreadable)?;
        if layer.mask_offset == 0 {
            return Ok(None);
        }
        let (width, height, offset) = (layer.width, layer.height, layer.mask_offset);
        let format = self.document.precision.format;
        let compression = self.document.compression;
        let hierarchy_offset = read_mask_hierarchy_offset(&mut self.stream, offset, width, height)?;
        read_grid(
            &mut self.stream,
            &GridRequest {
                offset: hierarchy_offset,
                width,
                height,
                channels: 1,
                format,
                compression,
            },
        )
        .map(Some)
    }
}

impl LayerInfo {
    #[must_use]
    pub fn has_mask(&self) -> bool {
        self.mask_offset != 0
    }
}

impl XcfDocument {
    #[must_use]
    pub fn sample_format(&self, kind: LayerKind) -> SampleFormat {
        if kind.is_indexed() {
            SampleFormat::U8
        } else {
            self.precision.format
        }
    }
}

fn read_document<R: Read + Seek>(stream: &mut Stream<R>) -> Result<XcfDocument, XcfError> {
    let tag = stream.array::<HEADER_TAG_LENGTH>()?;
    let version = tag_version(&tag).ok_or(XcfError::Unreadable)?;
    if version > NEWEST_KNOWN_VERSION {
        return Err(XcfError::Unsupported);
    }
    stream.long_pointers = version >= FIRST_LONG_POINTER_VERSION;
    let (width, height) = (stream.u32()?, stream.u32()?);
    check_image_size(width, height)?;
    let base_type = BaseType::from_code(stream.u32()?)?;
    let precision = if version >= FIRST_PRECISION_VERSION {
        Precision::from_code(version, stream.u32()?)?
    } else {
        Precision::EIGHT_BITS
    };
    let (palette, compression) = read_image_properties(stream)?;
    let layer_offsets = read_pointer_list(stream)?;
    let mut document = XcfDocument {
        version,
        width: usize::try_from(width).map_err(|_| XcfError::TooLarge)?,
        height: usize::try_from(height).map_err(|_| XcfError::TooLarge)?,
        base_type,
        precision,
        palette,
        layers: Vec::with_capacity(layer_offsets.len()),
        compression,
    };
    for offset in layer_offsets {
        let layer = read_layer(stream, offset)?;
        check_layer(&document, &layer)?;
        document.layers.push(layer);
    }
    Ok(document)
}

fn check_image_size(width: u32, height: u32) -> Result<(), XcfError> {
    if width == 0 || height == 0 {
        return Err(XcfError::Unreadable);
    }
    if width > LARGEST_SIDE || height > LARGEST_SIDE {
        return Err(XcfError::TooLarge);
    }
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(usize::try_from(height).ok()?));
    match pixels {
        Some(pixels) if pixels <= MOST_PIXELS => Ok(()),
        _ => Err(XcfError::TooLarge),
    }
}

fn check_layer(document: &XcfDocument, layer: &LayerInfo) -> Result<(), XcfError> {
    let kind_matches = match document.base_type {
        BaseType::Rgb => matches!(layer.kind, LayerKind::Rgb | LayerKind::RgbAlpha),
        BaseType::Gray => matches!(layer.kind, LayerKind::Gray | LayerKind::GrayAlpha),
        BaseType::Indexed => layer.kind.is_indexed(),
    };
    if !kind_matches {
        return Err(XcfError::Unreadable);
    }
    let pixels = layer
        .width
        .checked_mul(layer.height)
        .filter(|pixels| *pixels <= MOST_PIXELS)
        .ok_or(XcfError::TooLarge)?;
    let sample_bytes = pixels
        .checked_mul(layer.kind.channels() * document.sample_format(layer.kind).bytes())
        .filter(|bytes| *bytes <= MOST_SAMPLE_BYTES);
    sample_bytes.map(|_| ()).ok_or(XcfError::TooLarge)
}

fn read_image_properties<R: Read + Seek>(
    stream: &mut Stream<R>,
) -> Result<(Vec<[u8; 3]>, u8), XcfError> {
    let mut palette = Vec::new();
    let mut compression = COMPRESSION_RUN_LENGTH;
    loop {
        let (code, length) = stream.property_header()?;
        match code {
            PROP_END => return Ok((palette, compression)),
            PROP_COLORMAP => palette = read_palette(&stream.property_payload(length)?)?,
            PROP_COMPRESSION => {
                let payload = stream.property_payload(length)?;
                compression = *payload.first().ok_or(XcfError::Unreadable)?;
                if compression > COMPRESSION_ZLIB {
                    return Err(XcfError::Unsupported);
                }
            }
            _ => stream.skip(u64::from(length))?,
        }
    }
}

fn read_palette(payload: &[u8]) -> Result<Vec<[u8; 3]>, XcfError> {
    let colors = usize::try_from(Payload(payload).u32_at(0).ok_or(XcfError::Unreadable)?)
        .map_err(|_| XcfError::Unreadable)?;
    if colors > MOST_PALETTE_COLORS {
        return Err(XcfError::Unreadable);
    }
    let bytes = payload.get(4..).ok_or(XcfError::Unreadable)?;
    let (triples, _) = bytes.as_chunks::<3>();
    if triples.len() < colors {
        return Err(XcfError::Unreadable);
    }
    Ok(triples[..colors].to_vec())
}

fn read_pointer_list<R: Read + Seek>(stream: &mut Stream<R>) -> Result<Vec<u64>, XcfError> {
    let mut pointers = Vec::new();
    loop {
        let pointer = stream.pointer()?;
        if pointer == 0 {
            return Ok(pointers);
        }
        if pointers.len() == MOST_LAYERS {
            return Err(XcfError::TooLarge);
        }
        pointers.push(pointer);
    }
}

fn read_layer<R: Read + Seek>(stream: &mut Stream<R>, offset: u64) -> Result<LayerInfo, XcfError> {
    stream.seek_to(offset)?;
    let width = stream.size()?;
    let height = stream.size()?;
    let kind = LayerKind::from_code(stream.u32()?)?;
    let name = stream.string()?;
    let mut layer = LayerInfo {
        name,
        width,
        height,
        kind,
        offset_x: 0,
        offset_y: 0,
        opacity: 1.0,
        visible: true,
        mode: 0,
        blend_space: 0,
        composite_space: 0,
        composite_mode: 0,
        role: LayerRole::Layer,
        path: Vec::new(),
        apply_mask: true,
        has_filters: false,
        hierarchy_offset: 0,
        mask_offset: 0,
    };
    let mut has_float_opacity = false;
    loop {
        let (code, length) = stream.property_header()?;
        if code == PROP_END {
            break;
        }
        if !is_layer_property(code) {
            stream.skip(u64::from(length))?;
            continue;
        }
        let payload = stream.property_payload(length)?;
        has_float_opacity |= code == PROP_FLOAT_OPACITY;
        apply_layer_property(&mut layer, code, &Payload(&payload), has_float_opacity);
    }
    layer.hierarchy_offset = stream.pointer()?;
    layer.mask_offset = stream.pointer()?;
    if layer.width == 0 || layer.height == 0 || layer.hierarchy_offset == 0 {
        return Err(XcfError::Unreadable);
    }
    Ok(layer)
}

fn is_layer_property(code: u32) -> bool {
    matches!(
        code,
        PROP_OPACITY
            | PROP_MODE
            | PROP_VISIBLE
            | PROP_APPLY_MASK
            | PROP_OFFSETS
            | PROP_GROUP_ITEM
            | PROP_ITEM_PATH
            | PROP_FLOAT_OPACITY
            | PROP_COMPOSITE_MODE
            | PROP_COMPOSITE_SPACE
            | PROP_BLEND_SPACE
            | PROP_FILTERS
    )
}

fn apply_layer_property(
    layer: &mut LayerInfo,
    code: u32,
    payload: &Payload<'_>,
    has_float_opacity: bool,
) {
    match code {
        PROP_OPACITY if !has_float_opacity => {
            if let Some(value) = payload.u32_at(0) {
                layer.opacity = (f32::from(u8::try_from(value.min(255)).unwrap_or(u8::MAX))
                    / f32::from(u8::MAX))
                .clamp(0.0, 1.0);
            }
        }
        PROP_FLOAT_OPACITY => {
            if let Some(value) = payload.f32_at(0) {
                layer.opacity = if value.is_finite() {
                    value.clamp(0.0, 1.0)
                } else {
                    1.0
                };
            }
        }
        PROP_MODE => layer.mode = payload.u32_at(0).unwrap_or(0),
        PROP_VISIBLE => layer.visible = payload.u32_at(0).unwrap_or(1) != 0,
        PROP_APPLY_MASK => layer.apply_mask = payload.u32_at(0).unwrap_or(1) != 0,
        PROP_OFFSETS => {
            layer.offset_x = payload.i32_at(0).unwrap_or(0);
            layer.offset_y = payload.i32_at(1).unwrap_or(0);
        }
        PROP_GROUP_ITEM => layer.role = LayerRole::Group,
        PROP_ITEM_PATH => {
            layer.path = (0..payload.count().min(MOST_PATH_DEPTH))
                .filter_map(|index| payload.u32_at(index))
                .collect();
        }
        PROP_COMPOSITE_MODE => layer.composite_mode = payload.i32_at(0).unwrap_or(0),
        PROP_COMPOSITE_SPACE => layer.composite_space = payload.i32_at(0).unwrap_or(0),
        PROP_BLEND_SPACE => layer.blend_space = payload.i32_at(0).unwrap_or(0),
        PROP_FILTERS => layer.has_filters = payload.count() > 0,
        _ => {}
    }
}

fn read_mask_hierarchy_offset<R: Read + Seek>(
    stream: &mut Stream<R>,
    offset: u64,
    width: usize,
    height: usize,
) -> Result<u64, XcfError> {
    stream.seek_to(offset)?;
    if stream.size()? != width || stream.size()? != height {
        return Err(XcfError::Unreadable);
    }
    stream.string()?;
    loop {
        let (code, length) = stream.property_header()?;
        if code == PROP_END {
            break;
        }
        stream.skip(u64::from(length))?;
    }
    let hierarchy = stream.pointer()?;
    if hierarchy == 0 {
        return Err(XcfError::Unreadable);
    }
    Ok(hierarchy)
}

struct GridRequest {
    offset: u64,
    width: usize,
    height: usize,
    channels: usize,
    format: SampleFormat,
    compression: u8,
}

struct TileSpot {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

fn read_grid<R: Read + Seek>(
    stream: &mut Stream<R>,
    request: &GridRequest,
) -> Result<SampleGrid, XcfError> {
    let bytes_per_pixel = request.channels * request.format.bytes();
    let level = read_level_offset(stream, request, bytes_per_pixel)?;
    let tile_offsets = read_tile_offsets(stream, level, request)?;
    let sample_count = request.width * request.height * request.channels;
    let mut grid = SampleGrid {
        width: request.width,
        height: request.height,
        channels: request.channels,
        samples: empty_samples(request.format, sample_count),
    };
    let columns = request.width.div_ceil(TILE_SIDE);
    for (index, offset) in tile_offsets.into_iter().enumerate() {
        let x = (index % columns) * TILE_SIDE;
        let y = (index / columns) * TILE_SIDE;
        let spot = TileSpot {
            x,
            y,
            width: TILE_SIDE.min(request.width - x),
            height: TILE_SIDE.min(request.height - y),
        };
        let pixels = spot.width * spot.height;
        stream.seek_to(offset)?;
        let packed = stream.take_at_most(pixels * bytes_per_pixel * 2 + COMPRESSED_SLACK)?;
        let tile = unpack_tile(&packed, request.compression, pixels, bytes_per_pixel)?;
        copy_tile(&mut grid, request.format, &spot, &tile);
    }
    Ok(grid)
}

fn empty_samples(format: SampleFormat, count: usize) -> Samples {
    match format {
        SampleFormat::U8 => Samples::Eight(vec![0; count]),
        SampleFormat::U16 => Samples::Sixteen(vec![0; count]),
        SampleFormat::U32 | SampleFormat::Half | SampleFormat::Float | SampleFormat::Double => {
            Samples::Float(vec![0.0; count])
        }
    }
}

fn read_level_offset<R: Read + Seek>(
    stream: &mut Stream<R>,
    request: &GridRequest,
    bytes_per_pixel: usize,
) -> Result<u64, XcfError> {
    stream.seek_to(request.offset)?;
    let width = stream.size()?;
    let height = stream.size()?;
    let stored_bytes = stream.size()?;
    if (width, height) != (request.width, request.height) || stored_bytes != bytes_per_pixel {
        return Err(XcfError::Unreadable);
    }
    let level = stream.pointer()?;
    if level == 0 {
        return Err(XcfError::Unreadable);
    }
    Ok(level)
}

fn read_tile_offsets<R: Read + Seek>(
    stream: &mut Stream<R>,
    level: u64,
    request: &GridRequest,
) -> Result<Vec<u64>, XcfError> {
    stream.seek_to(level)?;
    if (stream.size()?, stream.size()?) != (request.width, request.height) {
        return Err(XcfError::Unreadable);
    }
    let count = request.width.div_ceil(TILE_SIDE) * request.height.div_ceil(TILE_SIDE);
    let mut offsets = Vec::with_capacity(count);
    for _ in 0..count {
        let offset = stream.pointer()?;
        if offset == 0 {
            return Err(XcfError::Unreadable);
        }
        offsets.push(offset);
    }
    Ok(offsets)
}

fn unpack_tile(
    packed: &[u8],
    compression: u8,
    pixels: usize,
    bytes_per_pixel: usize,
) -> Result<Vec<u8>, XcfError> {
    let size = pixels * bytes_per_pixel;
    match compression {
        COMPRESSION_NONE => packed
            .get(..size)
            .map(<[u8]>::to_vec)
            .ok_or(XcfError::Unreadable),
        COMPRESSION_RUN_LENGTH => unpack_run_length(packed, pixels, bytes_per_pixel),
        COMPRESSION_ZLIB => {
            let bytes = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(packed, size)
                .map_err(|_| XcfError::Unreadable)?;
            if bytes.len() == size {
                Ok(bytes)
            } else {
                Err(XcfError::Unreadable)
            }
        }
        _ => Err(XcfError::Unsupported),
    }
}

fn unpack_run_length(
    packed: &[u8],
    pixels: usize,
    bytes_per_pixel: usize,
) -> Result<Vec<u8>, XcfError> {
    let mut tile = vec![0_u8; pixels * bytes_per_pixel];
    let mut input = packed.iter().copied();
    for plane in 0..bytes_per_pixel {
        let mut written = 0;
        while written < pixels {
            let code = usize::from(input.next().ok_or(XcfError::Unreadable)?);
            let (length, repeats) = if code > EIGHT_BITS / 2 {
                (EIGHT_BITS + 1 - code, false)
            } else {
                (code + 1, true)
            };
            let length = if length == RUN_LENGTH_LONG {
                let high = usize::from(input.next().ok_or(XcfError::Unreadable)?);
                let low = usize::from(input.next().ok_or(XcfError::Unreadable)?);
                (high << 8) | low
            } else {
                length
            };
            if length == 0 || length > pixels - written {
                return Err(XcfError::Unreadable);
            }
            let repeated = if repeats {
                Some(input.next().ok_or(XcfError::Unreadable)?)
            } else {
                None
            };
            for position in written..written + length {
                let value = match repeated {
                    Some(value) => value,
                    None => input.next().ok_or(XcfError::Unreadable)?,
                };
                tile[position * bytes_per_pixel + plane] = value;
            }
            written += length;
        }
    }
    Ok(tile)
}

fn copy_tile(grid: &mut SampleGrid, format: SampleFormat, spot: &TileSpot, tile: &[u8]) {
    let channels = grid.channels;
    let per_row = spot.width * channels;
    let sample_bytes = format.bytes();
    for row in 0..spot.height {
        let target = ((spot.y + row) * grid.width + spot.x) * channels;
        let source = row * per_row * sample_bytes;
        let bytes = &tile[source..source + per_row * sample_bytes];
        match &mut grid.samples {
            Samples::Eight(data) => data[target..target + per_row].copy_from_slice(bytes),
            Samples::Sixteen(data) => {
                for (slot, chunk) in data[target..target + per_row]
                    .iter_mut()
                    .zip(bytes.as_chunks::<2>().0)
                {
                    *slot = u16::from_be_bytes(*chunk);
                }
            }
            Samples::Float(data) => {
                for (slot, chunk) in data[target..target + per_row]
                    .iter_mut()
                    .zip(bytes.chunks_exact(sample_bytes))
                {
                    *slot = float_sample(format, chunk);
                }
            }
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
fn float_sample(format: SampleFormat, chunk: &[u8]) -> f32 {
    match format {
        SampleFormat::Float => f32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
        SampleFormat::U32 => {
            (f64::from(u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                / f64::from(u32::MAX)) as f32
        }
        SampleFormat::Half => half_to_float(u16::from_be_bytes([chunk[0], chunk[1]])),
        SampleFormat::Double => f64::from_be_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]) as f32,
        SampleFormat::U8 | SampleFormat::U16 => 0.0,
    }
}

fn half_to_float(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((bits >> 10) & 0x1F);
    let fraction = f32::from(bits & 0x03FF);
    let magnitude = match exponent {
        0 => fraction / 1024.0 * 2.0_f32.powi(-14),
        0x1F if fraction == 0.0 => f32::INFINITY,
        0x1F => f32::NAN,
        _ => (1.0 + fraction / 1024.0) * 2.0_f32.powi(exponent - 15),
    };
    sign * magnitude
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        BaseType, LayerKind, LayerRole, SampleFormat, Samples, XcfError, XcfReader,
        gimp_dimensions, is_gimp_file,
    };

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn fixture_bytes(name: &str) -> Vec<u8> {
        std::fs::read(fixture(name)).unwrap()
    }

    fn eight(samples: &Samples) -> &[u8] {
        match samples {
            Samples::Eight(data) => data,
            other => panic!("expected 8-bit samples, got {other:?}"),
        }
    }

    fn pixel8(grid: &super::SampleGrid, x: usize, y: usize) -> Vec<u8> {
        let start = (y * grid.width + x) * grid.channels;
        eight(&grid.samples)[start..start + grid.channels].to_vec()
    }

    fn put32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn put64(bytes: &mut Vec<u8>, value: usize) {
        bytes.extend_from_slice(&u64::try_from(value).unwrap().to_be_bytes());
    }

    fn run_length_plane(plane: &[u8]) -> Vec<u8> {
        let mut packed = Vec::new();
        let mut at = 0;
        while at < plane.len() {
            let value = plane[at];
            let same = plane[at..]
                .iter()
                .take_while(|byte| **byte == value)
                .count();
            if same >= 3 {
                let length = same.min(32_767);
                if length > 127 {
                    packed.push(127);
                    packed.extend_from_slice(&u16::try_from(length).unwrap().to_be_bytes());
                } else {
                    packed.push(u8::try_from(length - 1).unwrap());
                }
                packed.push(value);
                at += length;
            } else {
                let literal = plane[at..].len().min(127).min(same.max(1) + 1);
                packed.push(u8::try_from(256 - literal).unwrap());
                packed.extend_from_slice(&plane[at..at + literal]);
                at += literal;
            }
        }
        packed
    }

    fn pack_tile(compression: u8, tile: &[u8], bytes_per_pixel: usize) -> Vec<u8> {
        match compression {
            0 => tile.to_vec(),
            2 => miniz_oxide::deflate::compress_to_vec_zlib(tile, 6),
            _ => (0..bytes_per_pixel)
                .flat_map(|plane| {
                    let bytes: Vec<u8> = tile
                        .iter()
                        .skip(plane)
                        .step_by(bytes_per_pixel)
                        .copied()
                        .collect();
                    run_length_plane(&bytes)
                })
                .collect(),
        }
    }

    fn rgba_file(compression: u8, width: usize, height: usize, pixels: &[u8]) -> Vec<u8> {
        let columns = width.div_ceil(64);
        let rows = height.div_ceil(64);
        let mut tiles = Vec::new();
        for row in 0..rows {
            for column in 0..columns {
                let (x0, y0) = (column * 64, row * 64);
                let (tile_width, tile_height) = (64.min(width - x0), 64.min(height - y0));
                let mut tile = Vec::new();
                for y in y0..y0 + tile_height {
                    let start = (y * width + x0) * 4;
                    tile.extend_from_slice(&pixels[start..start + tile_width * 4]);
                }
                tiles.push(pack_tile(compression, &tile, 4));
            }
        }
        let layer_at = 14 + 16 + 9 + 8 + 24;
        let hierarchy_at = layer_at + 12 + 6 + 8 + 16;
        let level_at = hierarchy_at + 12 + 16;
        let mut next = level_at + 8 + 8 * tiles.len() + 8;
        let mut file = b"gimp xcf v011\0".to_vec();
        for value in [
            u32::try_from(width).unwrap(),
            u32::try_from(height).unwrap(),
            0,
            150,
            17,
            1,
        ] {
            put32(&mut file, value);
        }
        file.extend_from_slice(&[compression, 0, 0, 0, 0, 0, 0, 0, 0]);
        put64(&mut file, layer_at);
        put64(&mut file, 0);
        put64(&mut file, 0);
        for value in [
            u32::try_from(width).unwrap(),
            u32::try_from(height).unwrap(),
            1,
            2,
        ] {
            put32(&mut file, value);
        }
        file.extend_from_slice(b"a\0");
        put32(&mut file, 0);
        put32(&mut file, 0);
        put64(&mut file, hierarchy_at);
        put64(&mut file, 0);
        for value in [
            u32::try_from(width).unwrap(),
            u32::try_from(height).unwrap(),
            4,
        ] {
            put32(&mut file, value);
        }
        put64(&mut file, level_at);
        put64(&mut file, 0);
        for value in [
            u32::try_from(width).unwrap(),
            u32::try_from(height).unwrap(),
        ] {
            put32(&mut file, value);
        }
        for tile in &tiles {
            put64(&mut file, next);
            next += tile.len();
        }
        put64(&mut file, 0);
        for tile in &tiles {
            file.extend_from_slice(tile);
        }
        file
    }

    fn gradient(width: usize, height: usize) -> Vec<u8> {
        (0..width * height)
            .flat_map(|index| {
                let (x, y) = (index % width, index / width);
                [
                    u8::try_from(x % 256).unwrap(),
                    u8::try_from(y % 256).unwrap(),
                    u8::try_from((x * 3 + y) % 256).unwrap(),
                    255,
                ]
            })
            .collect()
    }

    fn flat(width: usize, height: usize) -> Vec<u8> {
        [12, 200, 90, 255].repeat(width * height)
    }

    #[test]
    fn recognises_the_header_and_reads_the_size() {
        let bytes = fixture_bytes("gimp-basic.xcf");
        assert_eq!(gimp_dimensions(&bytes), Some((200, 150)));
        assert!(is_gimp_file(&fixture("gimp-basic.xcf")));
        assert!(!is_gimp_file(&fixture("gimp-real.psd")));
        assert_eq!(gimp_dimensions(b"gimp xcf v011"), None);
        assert_eq!(gimp_dimensions(b"gimp xcf v0x1\0\0\0\0\0\0\0\0\0\0"), None);
        assert_eq!(gimp_dimensions(b"not a gimp image at all"), None);
    }

    #[test]
    fn reads_the_layers_of_a_real_gimp_image() {
        let bytes = fixture_bytes("gimp-basic.xcf");
        let reader = XcfReader::from_bytes(&bytes).unwrap();
        let document = reader.document();
        assert_eq!((document.width, document.height), (200, 150));
        assert_eq!(document.base_type, BaseType::Rgb);
        assert_eq!(document.precision.format, SampleFormat::U8);
        assert!(!document.precision.linear);
        let [top, background] = document.layers.as_slice() else {
            panic!("expected two layers");
        };
        assert_eq!(top.name, "top");
        assert_eq!((top.width, top.height), (130, 70));
        assert_eq!((top.offset_x, top.offset_y), (40, 30));
        assert!((top.opacity - 0.6).abs() < 0.01);
        assert_eq!(top.mode, 28);
        assert_eq!(top.kind, LayerKind::RgbAlpha);
        assert!(top.visible && !top.has_mask());
        assert_eq!(background.name, "bg");
        assert_eq!((background.width, background.height), (200, 150));
        assert!((background.opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn reads_the_pixels_of_a_real_gimp_image() {
        let bytes = fixture_bytes("gimp-basic.xcf");
        let mut reader = XcfReader::from_bytes(&bytes).unwrap();
        let top = reader.layer_pixels(0).unwrap();
        assert_eq!((top.width, top.height, top.channels), (130, 70, 4));
        assert_eq!(pixel8(&top, 0, 0), [0x7c, 0x89, 0xf3, 255]);
        assert_eq!(pixel8(&top, 129, 69), [0x7c, 0x89, 0xf3, 255]);
        let background = reader.layer_pixels(1).unwrap();
        assert_eq!(pixel8(&background, 0, 0), [231, 137, 137, 255]);
        assert_eq!(pixel8(&background, 100, 80), [124, 218, 149, 255]);
        assert_eq!(pixel8(&background, 199, 149), [124, 218, 149, 255]);
        assert_eq!(pixel8(&background, 199, 59), [231, 137, 137, 255]);
    }

    #[test]
    fn reads_tiles_across_the_boundaries() {
        let bytes = fixture_bytes("gimp-tiles.xcf");
        let mut reader = XcfReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.document().width, 300);
        let background = reader.layer_pixels(1).unwrap();
        let red = [231, 137, 137, 255];
        let green = [124, 218, 149, 255];
        assert_eq!(pixel8(&background, 69, 59), red);
        assert_eq!(pixel8(&background, 70, 60), green);
        assert_eq!(pixel8(&background, 219, 149), green);
        assert_eq!(pixel8(&background, 220, 149), red);
        assert_eq!(pixel8(&background, 299, 199), red);
        assert_eq!(pixel8(&background, 127, 127), green);
        assert_eq!(pixel8(&background, 128, 128), green);
        assert_eq!(pixel8(&background, 191, 59), red);
        assert_eq!(pixel8(&background, 192, 64), green);
    }

    #[test]
    fn reads_sixteen_bit_samples() {
        let bytes = fixture_bytes("gimp-p16.xcf");
        let mut reader = XcfReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.document().precision.format, SampleFormat::U16);
        let top = reader.layer_pixels(0).unwrap();
        let Samples::Sixteen(data) = &top.samples else {
            panic!("expected 16-bit samples");
        };
        assert_eq!(data[..4], [0x7c0a, 0x897f, 0xf465, 0xffff]);
    }

    #[test]
    fn reads_floating_point_samples() {
        let bytes = fixture_bytes("gimp-p32.xcf");
        let mut reader = XcfReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.document().precision.format, SampleFormat::Float);
        assert!(reader.document().precision.linear);
        let top = reader.layer_pixels(0).unwrap();
        let Samples::Float(data) = &top.samples else {
            panic!("expected floating point samples");
        };
        for (actual, expected) in data[..4].iter().zip([0.2, 0.25, 0.9, 1.0]) {
            assert!((actual - expected).abs() < 1e-6, "{actual} vs {expected}");
        }
    }

    #[test]
    fn reads_gray_and_indexed_images() {
        let bytes = fixture_bytes("gimp-gray.xcf");
        let mut reader = XcfReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.document().base_type, BaseType::Gray);
        let top = reader.layer_pixels(0).unwrap();
        assert_eq!(top.channels, 2);
        assert_eq!(pixel8(&top, 0, 0), [0x90, 255]);

        let bytes = fixture_bytes("gimp-indexed.xcf");
        let mut reader = XcfReader::from_bytes(&bytes).unwrap();
        let palette_length = reader.document().palette.len();
        assert_eq!(reader.document().base_type, BaseType::Indexed);
        assert!(palette_length > 1);
        let layer = reader.layer_pixels(0).unwrap();
        assert!(
            eight(&layer.samples)
                .chunks(layer.channels)
                .all(|pixel| usize::from(pixel[0]) < palette_length)
        );
    }

    #[test]
    fn reads_layer_masks() {
        let bytes = fixture_bytes("gimp-mask.xcf");
        let mut reader = XcfReader::from_bytes(&bytes).unwrap();
        assert!(reader.document().layers[0].has_mask());
        assert!(!reader.document().layers[1].has_mask());
        let mask = reader.mask_pixels(0).unwrap().unwrap();
        assert_eq!((mask.width, mask.height, mask.channels), (130, 70, 1));
        let values = eight(&mask.samples);
        assert!(values.contains(&0) && values.contains(&255));
        assert_eq!(reader.mask_pixels(1).unwrap(), None);
    }

    #[test]
    fn reads_groups_hidden_layers_and_paths() {
        let bytes = fixture_bytes("gimp-group.xcf");
        let reader = XcfReader::from_bytes(&bytes).unwrap();
        let names: Vec<&str> = reader
            .document()
            .layers
            .iter()
            .map(|layer| layer.name.as_str())
            .collect();
        assert_eq!(names, ["hidden", "group", "inner", "bg"]);
        let [hidden, group, inner, background] = reader.document().layers.as_slice() else {
            panic!("expected four layers");
        };
        assert!(!hidden.visible);
        assert_eq!(group.role, LayerRole::Group);
        assert!((group.opacity - 0.5).abs() < 0.01);
        assert_eq!(inner.role, LayerRole::Layer);
        assert_eq!(inner.path, [1, 0]);
        assert_eq!((inner.offset_x, inner.offset_y), (30, 20));
        assert!(background.path.is_empty());
    }

    #[test]
    fn reads_every_compression_the_same_way() {
        let (width, height) = (70, 65);
        let pixels = gradient(width, height);
        for compression in [0, 1, 2] {
            let file = rgba_file(compression, width, height, &pixels);
            let mut reader = XcfReader::from_bytes(&file).unwrap();
            let grid = reader.layer_pixels(0).unwrap();
            assert_eq!(eight(&grid.samples), pixels, "compression {compression}");
        }
    }

    #[test]
    fn reads_long_runs_of_identical_pixels() {
        let (width, height) = (128, 128);
        let pixels = flat(width, height);
        for compression in [0, 1, 2] {
            let file = rgba_file(compression, width, height, &pixels);
            let mut reader = XcfReader::from_bytes(&file).unwrap();
            let grid = reader.layer_pixels(0).unwrap();
            assert_eq!(eight(&grid.samples), pixels, "compression {compression}");
        }
    }

    #[test]
    fn refuses_files_that_are_not_gimp_images() {
        for bytes in [
            Vec::new(),
            b"gimp xcf".to_vec(),
            b"8BPS\0\x01\0\0\0\0\0\0\0\x03\0\0\0\x10".to_vec(),
            vec![0_u8; 400],
        ] {
            assert!(matches!(
                XcfReader::from_bytes(&bytes),
                Err(XcfError::Unreadable)
            ));
        }
    }

    #[test]
    fn refuses_versions_from_the_future_and_giant_images() {
        let mut future = fixture_bytes("gimp-basic.xcf");
        future[10..13].copy_from_slice(b"099");
        assert!(matches!(
            XcfReader::from_bytes(&future),
            Err(XcfError::Unsupported)
        ));
        let mut giant = fixture_bytes("gimp-basic.xcf");
        giant[14..18].copy_from_slice(&100_000_u32.to_be_bytes());
        assert!(matches!(
            XcfReader::from_bytes(&giant),
            Err(XcfError::TooLarge)
        ));
        let mut wide = fixture_bytes("gimp-basic.xcf");
        wide[14..18].copy_from_slice(&29_000_u32.to_be_bytes());
        wide[18..22].copy_from_slice(&29_000_u32.to_be_bytes());
        assert!(matches!(
            XcfReader::from_bytes(&wide),
            Err(XcfError::TooLarge)
        ));
    }

    fn read_everything(bytes: &[u8]) -> bool {
        let Ok(mut reader) = XcfReader::from_bytes(bytes) else {
            return false;
        };
        let count = reader.document().layers.len();
        (0..count)
            .all(|index| reader.layer_pixels(index).is_ok() && reader.mask_pixels(index).is_ok())
    }

    #[test]
    fn a_cut_file_is_refused_without_panicking() {
        for name in [
            "gimp-basic.xcf",
            "gimp-mask.xcf",
            "gimp-group.xcf",
            "gimp-p16.xcf",
        ] {
            let bytes = fixture_bytes(name);
            assert!(read_everything(&bytes), "{name} whole");
            for length in 0..bytes.len() - 64 {
                assert!(!read_everything(&bytes[..length]), "{name} cut at {length}");
            }
        }
    }

    #[test]
    fn damaged_bytes_never_panic() {
        for name in [
            "gimp-basic.xcf",
            "gimp-mask.xcf",
            "gimp-group.xcf",
            "gimp-indexed.xcf",
        ] {
            let bytes = fixture_bytes(name);
            for position in 0..bytes.len() {
                for pattern in [0xFF_u8, 0x80, 0x01] {
                    let mut damaged = bytes.clone();
                    damaged[position] ^= pattern;
                    let _ = read_everything(&damaged);
                }
            }
        }
    }

    #[test]
    fn a_swollen_zlib_tile_cannot_exceed_its_size() {
        let (width, height) = (64, 64);
        let mut file = rgba_file(2, width, height, &flat(width, height));
        let bomb = miniz_oxide::deflate::compress_to_vec_zlib(&vec![0_u8; 1 << 22], 9);
        let tile_at =
            file.len() - miniz_oxide::deflate::compress_to_vec_zlib(&flat(width, height), 6).len();
        file.truncate(tile_at);
        file.extend_from_slice(&bomb);
        let mut reader = XcfReader::from_bytes(&file).unwrap();
        assert!(reader.layer_pixels(0).is_err());
    }
}
