use crate::raster::RgbaImage;

const CHANNELS: usize = 4;
const GRID_WIDTH: u32 = 9;
const GRID_HEIGHT: u32 = 8;
const VISIBLE_ALPHA: u8 = 20;
const OPAQUE_ALPHA: u8 = 250;
const BORDER_SAMPLES: u32 = 32;
const BACKGROUND_TOLERANCE: u32 = 60;
const BACKGROUND_SHARE_PERCENT: usize = 80;
const LIGHT_IMAGE_LUMINANCE: u64 = 153;
const FLAT_RANGE: u8 = 8;
const MIN_STEP: u8 = 3;
const BYTE_MAX: u64 = 255;
const RED_WEIGHT: u64 = 299;
const GREEN_WEIGHT: u64 = 587;
const BLUE_WEIGHT: u64 = 114;
const WEIGHT_SCALE: u64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VisualHash(u64);

impl VisualHash {
    pub const BYTE_LENGTH: usize = 8;

    #[must_use]
    pub fn distance(self, other: Self) -> u32 {
        (self.0 ^ other.0).count_ones()
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; Self::BYTE_LENGTH] {
        self.0.to_be_bytes()
    }

    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let bytes: [u8; Self::BYTE_LENGTH] = bytes.try_into().ok()?;
        Some(Self(u64::from_be_bytes(bytes)))
    }

    #[must_use]
    pub fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
}

#[must_use]
pub fn visual_hash(image: &RgbaImage) -> Option<VisualHash> {
    hash_of_normalized(&normalized(image)?)
}

pub(super) fn normalized(image: &RgbaImage) -> Option<RgbaImage> {
    trimmed_to_visible(&without_plain_background(image))
}

pub(super) fn hash_of_normalized(trimmed: &RgbaImage) -> Option<VisualHash> {
    let grid = gray_grid(trimmed)?;
    let lightest = *grid.iter().max()?;
    let darkest = *grid.iter().min()?;
    if lightest - darkest < FLAT_RANGE {
        return None;
    }
    let mut bits = 0_u64;
    for row in grid.chunks(GRID_WIDTH as usize) {
        for pair in row.windows(2) {
            bits = (bits << 1) | u64::from(pair[0] > pair[1].saturating_add(MIN_STEP));
        }
    }
    Some(VisualHash(bits))
}

fn without_plain_background(image: &RgbaImage) -> RgbaImage {
    let mut pixels = image.pixels().to_vec();
    let is_opaque = pixels
        .as_chunks::<CHANNELS>()
        .0
        .iter()
        .all(|pixel| pixel[3] >= OPAQUE_ALPHA);
    if is_opaque && let Some(background) = plain_border_color(image) {
        for pixel in pixels.as_chunks_mut::<CHANNELS>().0 {
            if color_distance([pixel[0], pixel[1], pixel[2]], background) < BACKGROUND_TOLERANCE {
                pixel[3] = 0;
            }
        }
    }
    RgbaImage::new(image.width(), image.height(), pixels)
        .expect("the same dimensions and pixel count as the source")
}

fn plain_border_color(image: &RgbaImage) -> Option<[u8; 3]> {
    let (width, height) = (image.width(), image.height());
    let across = (width / BORDER_SAMPLES).max(1);
    let down = (height / BORDER_SAMPLES).max(1);
    let mut samples: Vec<[u8; 3]> = Vec::new();
    for x in (0..width).step_by(across as usize) {
        samples.push(rgb_at(image, x, 0));
        samples.push(rgb_at(image, x, height - 1));
    }
    for y in (0..height).step_by(down as usize) {
        samples.push(rgb_at(image, 0, y));
        samples.push(rgb_at(image, width - 1, y));
    }
    samples.sort_by_key(|color| color.iter().map(|channel| u32::from(*channel)).sum::<u32>());
    let median = samples[samples.len() / 2];
    let alike = samples
        .iter()
        .filter(|color| color_distance(**color, median) < BACKGROUND_TOLERANCE)
        .count();
    (alike * 100 >= samples.len() * BACKGROUND_SHARE_PERCENT).then_some(median)
}

fn rgb_at(image: &RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let start = (y as usize * image.width() as usize + x as usize) * CHANNELS;
    let pixels = image.pixels();
    [pixels[start], pixels[start + 1], pixels[start + 2]]
}

fn color_distance(first: [u8; 3], second: [u8; 3]) -> u32 {
    first
        .iter()
        .zip(second)
        .map(|(a, b)| u32::from(*a).abs_diff(u32::from(b)))
        .sum()
}

fn trimmed_to_visible(image: &RgbaImage) -> Option<RgbaImage> {
    let width = image.width() as usize;
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    for (index, pixel) in image.pixels().as_chunks::<CHANNELS>().0.iter().enumerate() {
        if pixel[3] > VISIBLE_ALPHA {
            let (x, y) = (index % width, index / width);
            bounds = Some(match bounds {
                None => (x, y, x, y),
                Some((left, top, right, bottom)) => {
                    (left.min(x), top.min(y), right.max(x), bottom.max(y))
                }
            });
        }
    }
    let (left, top, right, bottom) = bounds?;
    Some(image.cropped(
        u32::try_from(left).ok()?,
        u32::try_from(top).ok()?,
        u32::try_from(right - left + 1).ok()?,
        u32::try_from(bottom - top + 1).ok()?,
    ))
}

fn gray_grid(image: &RgbaImage) -> Option<Vec<u8>> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let (columns, rows) = (GRID_WIDTH as usize, GRID_HEIGHT as usize);
    if width < columns || height < rows {
        return None;
    }
    let background = background_for(image);
    let grays: Vec<u64> = image
        .pixels()
        .as_chunks::<CHANNELS>()
        .0
        .iter()
        .map(|pixel| {
            let alpha = u64::from(pixel[3]);
            (luminance(pixel) * alpha + background * (BYTE_MAX - alpha)) / BYTE_MAX
        })
        .collect();
    let mut grid = Vec::with_capacity(columns * rows);
    for row in 0..rows {
        for column in 0..columns {
            let (first_x, last_x) = cell_bounds(column, columns, width);
            let (first_y, last_y) = cell_bounds(row, rows, height);
            let mut sum = 0_u64;
            for y in first_y..last_y {
                sum += grays[y * width + first_x..y * width + last_x]
                    .iter()
                    .sum::<u64>();
            }
            let count = u64::try_from((last_x - first_x) * (last_y - first_y)).ok()?;
            grid.push(u8::try_from(sum / count).unwrap_or(u8::MAX));
        }
    }
    Some(grid)
}

pub(super) fn cell_bounds(index: usize, cells: usize, length: usize) -> (usize, usize) {
    let first = index * length / cells;
    let last = ((index + 1) * length).div_ceil(cells);
    (first, last.max(first + 1).min(length))
}

fn background_for(image: &RgbaImage) -> u64 {
    let (mut weighted, mut weight) = (0_u64, 0_u64);
    for pixel in image.pixels().as_chunks::<CHANNELS>().0 {
        let alpha = u64::from(pixel[3]);
        weighted += luminance(pixel) * alpha;
        weight += alpha;
    }
    if weight > 0 && weighted / weight > LIGHT_IMAGE_LUMINANCE {
        0
    } else {
        BYTE_MAX
    }
}

fn luminance(pixel: &[u8]) -> u64 {
    (u64::from(pixel[0]) * RED_WEIGHT
        + u64::from(pixel[1]) * GREEN_WEIGHT
        + u64::from(pixel[2]) * BLUE_WEIGHT)
        / WEIGHT_SCALE
}

#[cfg(test)]
mod tests {
    use super::{VisualHash, visual_hash};
    use crate::raster::RgbaImage;

    const INK: [u8; 4] = [30, 30, 30, 255];
    const CLEAR: [u8; 4] = [0, 0, 0, 0];
    const WHITE: [u8; 4] = [255, 255, 255, 255];

    fn disc(u: f64, v: f64) -> bool {
        (u - 0.5).powi(2) + (v - 0.5).powi(2) < 0.16
    }

    fn corner(u: f64, v: f64) -> bool {
        u < 0.35 || v > 0.65
    }

    fn band(u: f64, v: f64) -> bool {
        (u - v).abs() < 0.2
    }

    fn picture(
        side: u32,
        margin: f64,
        background: [u8; 4],
        ink: [u8; 4],
        shape: fn(f64, f64) -> bool,
    ) -> RgbaImage {
        let mut pixels = Vec::new();
        for y in 0..side {
            for x in 0..side {
                let u = (f64::from(x) + 0.5) / f64::from(side);
                let v = (f64::from(y) + 0.5) / f64::from(side);
                let inside = u >= margin && u <= 1.0 - margin && v >= margin && v <= 1.0 - margin;
                let (u, v) = (
                    (u - margin) / (1.0 - 2.0 * margin),
                    (v - margin) / (1.0 - 2.0 * margin),
                );
                pixels.extend(if inside && shape(u, v) {
                    ink
                } else {
                    background
                });
            }
        }
        RgbaImage::new(side, side, pixels).expect("a square picture")
    }

    fn hash(image: &RgbaImage) -> VisualHash {
        visual_hash(image).expect("a picture with something in it")
    }

    #[test]
    fn a_reduced_copy_stays_very_close() {
        for shape in [disc, corner, band] {
            let original = picture(256, 0.0, CLEAR, INK, shape);
            let large = hash(&original);
            let small = hash(&original.resized(48, 48));
            assert!(large.distance(small) <= 6, "{}", large.distance(small));
        }
    }

    #[test]
    fn different_drawings_are_far_apart() {
        let disc = hash(&picture(256, 0.0, CLEAR, INK, disc));
        let corner = hash(&picture(256, 0.0, CLEAR, INK, corner));
        let band = hash(&picture(256, 0.0, CLEAR, INK, band));
        assert!(disc.distance(corner) > 10);
        assert!(disc.distance(band) > 10);
        assert!(corner.distance(band) > 10);
    }

    #[test]
    fn transparent_margins_do_not_change_the_drawing() {
        let plain = hash(&picture(256, 0.0, CLEAR, INK, band));
        let framed = hash(&picture(256, 0.1, CLEAR, INK, band));
        assert!(plain.distance(framed) <= 6, "{}", plain.distance(framed));
    }

    #[test]
    fn a_plain_background_is_ignored_like_transparency() {
        let transparent = hash(&picture(256, 0.0, CLEAR, INK, corner));
        let on_white = hash(&picture(256, 0.0, WHITE, INK, corner));
        assert!(
            transparent.distance(on_white) <= 6,
            "{}",
            transparent.distance(on_white)
        );
    }

    #[test]
    fn a_light_drawing_does_not_vanish_on_a_light_background() {
        let first = hash(&picture(256, 0.0, CLEAR, WHITE, disc));
        let second = hash(&picture(256, 0.0, CLEAR, WHITE, corner));
        assert!(first.distance(second) > 10);
    }

    #[test]
    fn empty_flat_and_tiny_pictures_have_no_hash() {
        assert!(visual_hash(&picture(64, 0.0, CLEAR, CLEAR, disc)).is_none());
        assert!(visual_hash(&picture(64, 0.0, WHITE, WHITE, disc)).is_none());
        assert!(visual_hash(&picture(6, 0.0, CLEAR, INK, disc)).is_none());
    }

    #[test]
    fn bytes_round_trip_and_wrong_lengths_are_refused() {
        let original = hash(&picture(128, 0.0, CLEAR, INK, band));
        assert_eq!(VisualHash::from_bytes(&original.to_bytes()), Some(original));
        assert_eq!(VisualHash::from_bytes(&[1, 2, 3]), None);
        assert_eq!(VisualHash::from_bytes(&[]), None);
    }
}
