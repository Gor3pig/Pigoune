use crate::raster::RgbaImage;

const SIGNATURE: &[u8; 4] = b"8BPS";
const VERSION: u16 = 1;
const HEADER_FIELDS_LENGTH: usize = 20;
const MOST_CHANNELS: u16 = 56;
const LARGEST_SIDE: u32 = 30_000;
const MOST_PIXELS: usize = 64_000_000;
const PALETTE_LENGTH: usize = 768;
const PALETTE_SIDE: usize = 256;
const OPAQUE: u8 = 255;
const RGBA_CHANNELS: usize = 4;
const BITS_PER_BYTE: usize = 8;
const COMPRESSION_RAW: u16 = 0;
const COMPRESSION_RLE: u16 = 1;
const REPEAT_MARKER: i8 = -128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PsdError {
    #[error("the file is not a readable PSD image")]
    Unreadable,
    #[error("the image is too large to be read")]
    TooLarge,
    #[error("this kind of PSD image cannot be read")]
    Unsupported,
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

    fn planes_read(&self) -> usize {
        (self.mode.colorant_channels() + 1).min(self.channels)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], PsdError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(PsdError::Unreadable)?;
        let taken = self
            .bytes
            .get(self.position..end)
            .ok_or(PsdError::Unreadable)?;
        self.position = end;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PsdError> {
        self.take(N)?.try_into().map_err(|_| PsdError::Unreadable)
    }

    fn u16(&mut self) -> Result<u16, PsdError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, PsdError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn section(&mut self) -> Result<&'a [u8], PsdError> {
        let length = usize::try_from(self.u32()?).map_err(|_| PsdError::Unreadable)?;
        self.take(length)
    }
}

pub fn read_merged_image(bytes: &[u8]) -> Result<RgbaImage, PsdError> {
    let mut reader = Reader { bytes, position: 0 };
    let header = read_header(&mut reader)?;
    let palette = reader.section()?;
    reader.section()?;
    reader.section()?;
    let planes = read_planes(&mut reader, &header)?;
    let planes = eight_bit_planes(planes, &header);
    compose(&header, palette, &planes)
}

fn read_header(reader: &mut Reader) -> Result<Header, PsdError> {
    if reader.take(SIGNATURE.len())? != SIGNATURE {
        return Err(PsdError::Unreadable);
    }
    if reader.u16()? != VERSION {
        return Err(PsdError::Unsupported);
    }
    reader.take(HEADER_FIELDS_LENGTH - 14)?;
    let channels = reader.u16()?;
    let height = reader.u32()?;
    let width = reader.u32()?;
    let depth = reader.u16()?;
    let mode = ColorMode::from_code(reader.u16()?)?;
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
    if header.width * header.height > MOST_PIXELS {
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

fn read_planes(reader: &mut Reader, header: &Header) -> Result<Vec<Vec<u8>>, PsdError> {
    match reader.u16()? {
        COMPRESSION_RAW => (0..header.planes_read())
            .map(|_| Ok(reader.take(header.plane_bytes())?.to_vec()))
            .collect(),
        COMPRESSION_RLE => read_run_length_planes(reader, header),
        _ => Err(PsdError::Unsupported),
    }
}

fn read_run_length_planes(reader: &mut Reader, header: &Header) -> Result<Vec<Vec<u8>>, PsdError> {
    let row_count = header.channels * header.height;
    let mut lengths = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        lengths.push(usize::from(reader.u16()?));
    }
    let mut lengths = lengths.into_iter();
    (0..header.planes_read())
        .map(|_| {
            let mut plane = Vec::with_capacity(header.plane_bytes());
            for _ in 0..header.height {
                let length = lengths.next().ok_or(PsdError::Unreadable)?;
                plane.extend(unpack_row(reader.take(length)?, header.row_bytes())?);
            }
            Ok(plane)
        })
        .collect()
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
        if marker >= 0 {
            let count = usize::from(marker.unsigned_abs()) + 1;
            row.extend_from_slice(rest.get(..count).ok_or(PsdError::Unreadable)?);
            rest = &rest[count..];
        } else {
            let count = usize::from(marker.unsigned_abs()) + 1;
            let (&value, after) = rest.split_first().ok_or(PsdError::Unreadable)?;
            row.extend(std::iter::repeat_n(value, count));
            rest = after;
        }
    }
    (row.len() == row_bytes)
        .then_some(row)
        .ok_or(PsdError::Unreadable)
}

fn eight_bit_planes(planes: Vec<Vec<u8>>, header: &Header) -> Vec<Vec<u8>> {
    planes
        .into_iter()
        .map(|plane| match header.depth {
            16 => plane
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| pair[0])
                .collect(),
            1 => unpack_bits(&plane, header),
            _ => plane,
        })
        .collect()
}

fn unpack_bits(plane: &[u8], header: &Header) -> Vec<u8> {
    let row_bytes = header.row_bytes();
    let mut pixels = Vec::with_capacity(header.width * header.height);
    for row in plane.chunks_exact(row_bytes) {
        for x in 0..header.width {
            let black = (row[x / BITS_PER_BYTE] >> (BITS_PER_BYTE - 1 - x % BITS_PER_BYTE)) & 1;
            pixels.push(if black == 1 { 0 } else { OPAQUE });
        }
    }
    pixels
}

fn compose(header: &Header, palette: &[u8], planes: &[Vec<u8>]) -> Result<RgbaImage, PsdError> {
    if header.mode == ColorMode::Indexed && palette.len() != PALETTE_LENGTH {
        return Err(PsdError::Unreadable);
    }
    let colorants = header.mode.colorant_channels();
    let alpha = planes.get(colorants);
    let mut pixels = Vec::with_capacity(header.width * header.height * RGBA_CHANNELS);
    for index in 0..header.width * header.height {
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
        pixels.extend([red, green, blue, alpha.map_or(OPAQUE, |plane| plane[index])]);
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
    use super::{PsdError, read_merged_image};

    const RGB: u16 = 3;
    const GRAY: u16 = 1;
    const INDEXED: u16 = 2;
    const CMYK: u16 = 4;
    const BITMAP: u16 = 0;
    const RAW: u16 = 0;
    const RLE: u16 = 1;

    struct Parts<'a> {
        mode: u16,
        depth: u16,
        channels: u16,
        width: u32,
        height: u32,
        color_data: &'a [u8],
        compression: u16,
        image_data: &'a [u8],
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
        bytes.extend_from_slice(&u32::try_from(parts.color_data.len()).unwrap().to_be_bytes());
        bytes.extend_from_slice(parts.color_data);
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&parts.compression.to_be_bytes());
        bytes.extend_from_slice(parts.image_data);
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
            compression: RAW,
            image_data: data,
        })
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
    fn keeps_the_first_extra_channel_as_transparency() {
        let bytes = raw(RGB, 8, 4, 1, 1, &[10, 20, 30, 40]);

        assert_eq!(pixels(&bytes), [10, 20, 30, 40]);
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
            compression: RLE,
            image_data: &data,
        });

        assert_eq!(
            pixels(&bytes),
            [1, 9, 7, 255, 2, 9, 7, 255, 3, 9, 7, 255, 4, 9, 7, 255]
        );
    }

    #[test]
    fn keeps_the_high_byte_of_sixteen_bit_samples() {
        let bytes = raw(GRAY, 16, 1, 1, 1, &[0x80, 0x01]);

        assert_eq!(pixels(&bytes), [0x80, 0x80, 0x80, 255]);
    }

    #[test]
    fn reads_gray_with_transparency() {
        let bytes = raw(GRAY, 8, 2, 1, 1, &[100, 50]);

        assert_eq!(pixels(&bytes), [100, 100, 100, 50]);
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
        let zipped = file(&Parts {
            mode: RGB,
            depth: 8,
            channels: 3,
            width: 1,
            height: 1,
            color_data: &[],
            compression: 2,
            image_data: &[0; 3],
        });

        assert_eq!(
            read_merged_image(&thirty_two_bit),
            Err(PsdError::Unsupported)
        );
        assert_eq!(read_merged_image(&lab), Err(PsdError::Unsupported));
        assert_eq!(read_merged_image(&zipped), Err(PsdError::Unsupported));
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

    #[test]
    fn never_panics_on_truncated_or_damaged_files() {
        let valid = raw(RGB, 8, 3, 3, 2, &(0..18).collect::<Vec<u8>>());
        for length in 0..valid.len() {
            let _ = read_merged_image(&valid[..length]);
        }
        for position in 0..valid.len() {
            for value in [0, 1, 127, 128, 255] {
                let mut damaged = valid.clone();
                damaged[position] = value;
                let _ = read_merged_image(&damaged);
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
            compression: RLE,
            image_data: &data,
        });

        assert_eq!(read_merged_image(&bytes), Err(PsdError::Unreadable));
    }

    #[test]
    fn reads_a_file_written_by_another_program() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/rle-gradient.psd"
        ))
        .expect("the fixture exists");

        let image = read_merged_image(&bytes).expect("the image is read");

        assert_eq!((image.width(), image.height()), (16, 8));
        assert_eq!(&image.pixels()[..4], [255, 0, 0, 255]);
        let last = &image.pixels()[image.pixels().len() - 4..];
        assert!(last[0] < 40 && last[2] > 215, "ends bluish: {last:?}");
    }

    #[test]
    fn refuses_a_zip_compressed_merged_image() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/zip-layers-gradient.psd"
        ))
        .expect("the fixture exists");

        assert_eq!(read_merged_image(&bytes), Err(PsdError::Unsupported));
    }
}
