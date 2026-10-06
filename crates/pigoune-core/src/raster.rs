use std::f64::consts::PI;

const CHANNELS: usize = 4;
const ALPHA: usize = 3;
const LOBES: f64 = 3.0;
const BYTE_MAX: f64 = 255.0;
const BLUR_PASSES: usize = 3;
const LARGEST_AVERAGING_FACTOR: u32 = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RgbaImage {
    #[must_use]
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let expected = usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?
            .checked_mul(CHANNELS)?;
        (width > 0 && height > 0 && pixels.len() == expected).then_some(Self {
            width,
            height,
            pixels,
        })
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    #[must_use]
    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }

    #[must_use]
    pub fn resized(&self, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        if (width, height) == (self.width, self.height) {
            return self.clone();
        }
        let premultiplied = premultiply(&self.pixels);
        let across = resample_rows(&premultiplied, self.width, self.height, width);
        let down = resample_columns(&across, width, self.height, height);
        Self {
            width,
            height,
            pixels: unpremultiply(&down),
        }
    }

    #[must_use]
    pub fn shrunk_to(&self, width: u32, height: u32) -> Self {
        let width = width.clamp(1, self.width);
        let height = height.clamp(1, self.height);
        let factor = (self.width / (width * 2))
            .min(self.height / (height * 2))
            .min(LARGEST_AVERAGING_FACTOR);
        if factor < 2 {
            return self.resized(width, height);
        }
        self.averaged_by(factor).resized(width, height)
    }

    fn averaged_by(&self, factor: u32) -> Self {
        let width = self.width.div_ceil(factor);
        let height = self.height.div_ceil(factor);
        let row_length = to_index(self.width);
        let factor = to_index(factor);
        let mut pixels = Vec::with_capacity(to_index(width) * to_index(height) * CHANNELS);
        for block_rows in self.pixels.chunks(row_length * CHANNELS * factor) {
            for start in (0..row_length).step_by(factor) {
                let end = (start + factor).min(row_length);
                let mut sums = [0_u32; CHANNELS];
                let mut count = 0_u32;
                for row in block_rows.chunks(row_length * CHANNELS) {
                    for pixel in row[start * CHANNELS..end * CHANNELS]
                        .as_chunks::<CHANNELS>()
                        .0
                    {
                        let alpha = u32::from(pixel[ALPHA]);
                        for channel in 0..ALPHA {
                            sums[channel] += u32::from(pixel[channel]) * alpha;
                        }
                        sums[ALPHA] += alpha;
                        count += 1;
                    }
                }
                pixels.extend(averaged_pixel(sums, count));
            }
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    #[must_use]
    pub fn centered_on_square(&self, side: u32) -> Self {
        let side = side.max(self.width).max(self.height);
        self.centered_on(side, side)
    }

    #[must_use]
    pub fn centered_on(&self, width: u32, height: u32) -> Self {
        let width = width.max(self.width);
        let height = height.max(self.height);
        let canvas_row = to_index(width) * CHANNELS;
        let mut pixels = vec![0; canvas_row * to_index(height)];
        let left = to_index((width - self.width) / 2);
        let top = to_index((height - self.height) / 2);
        let row_length = to_index(self.width) * CHANNELS;
        for (row, source) in self.pixels.chunks(row_length).enumerate() {
            let start = (top + row) * canvas_row + left * CHANNELS;
            pixels[start..start + row_length].copy_from_slice(source);
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    #[must_use]
    pub fn cropped_to_center(&self, width: u32, height: u32) -> Self {
        let width = width.clamp(1, self.width);
        let height = height.clamp(1, self.height);
        self.cropped(
            (self.width - width) / 2,
            (self.height - height) / 2,
            width,
            height,
        )
    }

    #[must_use]
    pub fn cropped(&self, left: u32, top: u32, width: u32, height: u32) -> Self {
        let left = left.min(self.width - 1);
        let top = top.min(self.height - 1);
        let width = width.clamp(1, self.width - left);
        let height = height.clamp(1, self.height - top);
        let source_row = to_index(self.width) * CHANNELS;
        let start = to_index(left) * CHANNELS;
        let row_length = to_index(width) * CHANNELS;
        let pixels = self
            .pixels
            .chunks(source_row)
            .skip(to_index(top))
            .take(to_index(height))
            .flat_map(|row| &row[start..start + row_length])
            .copied()
            .collect();
        Self {
            width,
            height,
            pixels,
        }
    }

    #[must_use]
    pub fn placed_on(&self, width: u32, height: u32, left: u32, top: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let canvas_row = to_index(width) * CHANNELS;
        let mut pixels = vec![0; canvas_row * to_index(height)];
        let copied_width = to_index(self.width.min(width.saturating_sub(left)));
        let row_length = to_index(self.width) * CHANNELS;
        if copied_width > 0 {
            let copied_rows = to_index(self.height.min(height.saturating_sub(top)));
            for (row, source) in self.pixels.chunks(row_length).take(copied_rows).enumerate() {
                let start = (to_index(top) + row) * canvas_row + to_index(left) * CHANNELS;
                pixels[start..start + copied_width * CHANNELS]
                    .copy_from_slice(&source[..copied_width * CHANNELS]);
            }
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    #[must_use]
    pub fn placed_over(&self, canvas: &Self, left: u32, top: u32) -> Self {
        let mut pixels = canvas.pixels.clone();
        let canvas_row = to_index(canvas.width) * CHANNELS;
        let copied_width = to_index(self.width.min(canvas.width.saturating_sub(left)));
        let copied_rows = to_index(self.height.min(canvas.height.saturating_sub(top)));
        let row_length = to_index(self.width) * CHANNELS;
        for (row, source) in self.pixels.chunks(row_length).take(copied_rows).enumerate() {
            let start = (to_index(top) + row) * canvas_row + to_index(left) * CHANNELS;
            let target = &mut pixels[start..start + copied_width * CHANNELS];
            for (below, above) in target.chunks_mut(CHANNELS).zip(source.chunks(CHANNELS)) {
                blend_over(below, above);
            }
        }
        Self {
            width: canvas.width,
            height: canvas.height,
            pixels,
        }
    }

    #[must_use]
    pub fn blurred(&self, radius: u32) -> Self {
        let mut values: Vec<f64> = premultiply(&self.pixels);
        for _ in 0..BLUR_PASSES {
            values = box_blur_rows(&values, self.width, self.height, radius);
            values = box_blur_columns(&values, self.width, self.height, radius);
        }
        Self {
            width: self.width,
            height: self.height,
            pixels: unpremultiply(&values),
        }
    }

    #[must_use]
    pub fn linear_gradient(
        width: u32,
        height: u32,
        (start, end): ([u8; 3], [u8; 3]),
        angle_degrees: f64,
    ) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let angle = angle_degrees.to_radians();
        let (across, down) = (angle.sin(), -angle.cos());
        let (sides_width, sides_height) = (f64::from(width), f64::from(height));
        let length = (sides_width * across.abs() + sides_height * down.abs()).max(1.0);
        let mut pixels = Vec::with_capacity(to_index(width) * to_index(height) * CHANNELS);
        for row in 0..height {
            for column in 0..width {
                let x = f64::from(column) + 0.5 - sides_width / 2.0;
                let y = f64::from(row) + 0.5 - sides_height / 2.0;
                let share = ((x * across + y * down) / length + 0.5).clamp(0.0, 1.0);
                let mix = |from: u8, to: u8| {
                    to_byte(f64::from(from) + (f64::from(to) - f64::from(from)) * share)
                };
                pixels.extend([
                    mix(start[0], end[0]),
                    mix(start[1], end[1]),
                    mix(start[2], end[2]),
                    u8::MAX,
                ]);
            }
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    #[must_use]
    pub fn tiled(&self, width: u32, height: u32, left: i64, top: i64) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let tile_width = i64::from(self.width);
        let tile_height = i64::from(self.height);
        let mut pixels = Vec::with_capacity(to_index(width) * to_index(height) * CHANNELS);
        for row in 0..i64::from(height) {
            let source_row = to_index_i64((row - top).rem_euclid(tile_height));
            for column in 0..i64::from(width) {
                let source_column = to_index_i64((column - left).rem_euclid(tile_width));
                let start = (source_row * to_index(self.width) + source_column) * CHANNELS;
                pixels.extend_from_slice(&self.pixels[start..start + CHANNELS]);
            }
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    #[must_use]
    pub fn mirrored(&self) -> Self {
        let row_length = to_index(self.width) * CHANNELS;
        let pixels = self
            .pixels
            .chunks(row_length)
            .flat_map(|row| row.chunks(CHANNELS).rev().flatten().copied())
            .collect();
        Self {
            width: self.width,
            height: self.height,
            pixels,
        }
    }

    #[must_use]
    pub fn dimmed(&self, brightness: f64) -> Self {
        let pixels = self
            .pixels
            .chunks(CHANNELS)
            .flat_map(|pixel| {
                [
                    to_byte(f64::from(pixel[0]) * brightness),
                    to_byte(f64::from(pixel[1]) * brightness),
                    to_byte(f64::from(pixel[2]) * brightness),
                    pixel[ALPHA],
                ]
            })
            .collect();
        Self {
            width: self.width,
            height: self.height,
            pixels,
        }
    }
}

fn blend_over(below: &mut [u8], above: &[u8]) {
    let alpha = f64::from(above[ALPHA]) / BYTE_MAX;
    let below_alpha = f64::from(below[ALPHA]) / BYTE_MAX * (1.0 - alpha);
    let total = alpha + below_alpha;
    for channel in 0..ALPHA {
        let mixed = if total > 0.0 {
            (f64::from(above[channel]) * alpha + f64::from(below[channel]) * below_alpha) / total
        } else {
            0.0
        };
        below[channel] = to_byte(mixed);
    }
    below[ALPHA] = to_byte(total * BYTE_MAX);
}

fn box_blur_rows(values: &[f64], width: u32, height: u32, radius: u32) -> Vec<f64> {
    let row_length = to_index(width) * CHANNELS;
    let mut result = Vec::with_capacity(values.len());
    for row in values.chunks(row_length).take(to_index(height)) {
        result.extend(box_blur_line(row, to_index(width), CHANNELS, radius));
    }
    result
}

fn box_blur_columns(values: &[f64], width: u32, height: u32, radius: u32) -> Vec<f64> {
    let columns = to_index(width);
    let rows = to_index(height);
    let mut result = vec![0.0; values.len()];
    let mut column_values = vec![0.0; rows * CHANNELS];
    for column in 0..columns {
        for row in 0..rows {
            let start = (row * columns + column) * CHANNELS;
            column_values[row * CHANNELS..(row + 1) * CHANNELS]
                .copy_from_slice(&values[start..start + CHANNELS]);
        }
        let blurred = box_blur_line(&column_values, rows, CHANNELS, radius);
        for row in 0..rows {
            let start = (row * columns + column) * CHANNELS;
            result[start..start + CHANNELS]
                .copy_from_slice(&blurred[row * CHANNELS..(row + 1) * CHANNELS]);
        }
    }
    result
}

fn box_blur_line(line: &[f64], length: usize, channels: usize, radius: u32) -> Vec<f64> {
    let radius = to_index(radius);
    let window = f64::from(u32::try_from(radius * 2 + 1).unwrap_or(u32::MAX));
    let at = |index: isize, channel: usize| {
        let clamped = usize::try_from(index.max(0)).unwrap_or(0).min(length - 1);
        line[clamped * channels + channel]
    };
    let mut result = vec![0.0; length * channels];
    let reach = isize::try_from(radius).unwrap_or(isize::MAX);
    for channel in 0..channels {
        let mut sum: f64 = (-reach..=reach).map(|index| at(index, channel)).sum();
        for position in 0..length {
            result[position * channels + channel] = sum / window;
            let index = isize::try_from(position).unwrap_or(isize::MAX);
            sum += at(index + reach + 1, channel) - at(index - reach, channel);
        }
    }
    result
}

#[must_use]
pub fn fitted_within(width: u32, height: u32, longest: u32) -> (u32, u32) {
    let current = width.max(height).max(1);
    let scale = |side: u32| {
        let scaled = u64::from(side) * u64::from(longest) + u64::from(current) / 2;
        u32::try_from(scaled / u64::from(current))
            .unwrap_or(u32::MAX)
            .max(1)
    };
    (scale(width), scale(height))
}

fn to_index_i64(value: i64) -> usize {
    usize::try_from(value).unwrap_or(0)
}

fn to_index(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn averaged_pixel(sums: [u32; CHANNELS], count: u32) -> [u8; CHANNELS] {
    let alpha = sums[ALPHA];
    if alpha == 0 {
        return [0; CHANNELS];
    }
    let color = |sum: u32| to_u8((sum + alpha / 2) / alpha);
    [
        color(sums[0]),
        color(sums[1]),
        color(sums[2]),
        to_u8((alpha + count / 2) / count),
    ]
}

fn to_u8(value: u32) -> u8 {
    u8::try_from(value).unwrap_or(u8::MAX)
}

fn premultiply(pixels: &[u8]) -> Vec<f64> {
    pixels
        .as_chunks::<CHANNELS>()
        .0
        .iter()
        .flat_map(|pixel| {
            let alpha = f64::from(pixel[ALPHA]) / BYTE_MAX;
            [
                f64::from(pixel[0]) * alpha,
                f64::from(pixel[1]) * alpha,
                f64::from(pixel[2]) * alpha,
                f64::from(pixel[ALPHA]),
            ]
        })
        .collect()
}

fn unpremultiply(values: &[f64]) -> Vec<u8> {
    values
        .as_chunks::<CHANNELS>()
        .0
        .iter()
        .flat_map(|pixel| {
            let alpha = pixel[ALPHA].clamp(0.0, BYTE_MAX);
            let color = |value: f64| {
                if alpha <= 0.0 {
                    0
                } else {
                    to_byte(value * BYTE_MAX / alpha)
                }
            };
            [
                color(pixel[0]),
                color(pixel[1]),
                color(pixel[2]),
                to_byte(alpha),
            ]
        })
        .collect()
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is rounded and clamped to the range of a byte"
)]
fn to_byte(value: f64) -> u8 {
    value.round().clamp(0.0, BYTE_MAX) as u8
}

struct Contribution {
    first: usize,
    weights: Vec<f64>,
}

fn contributions(from: u32, to: u32) -> Vec<Contribution> {
    let scale = f64::from(from) / f64::from(to);
    let stretch = scale.max(1.0);
    let support = LOBES * stretch;
    let last = from - 1;
    (0..to)
        .map(|target| {
            let center = (f64::from(target) + 0.5) * scale - 0.5;
            let first = pixel_index(center - support, last);
            let end = pixel_index(center + support, last);
            let mut weights: Vec<f64> = (first..=end)
                .map(|source| lanczos((f64::from(source) - center) / stretch))
                .collect();
            let total: f64 = weights.iter().sum();
            if total.abs() > f64::EPSILON {
                for weight in &mut weights {
                    *weight /= total;
                }
            }
            Contribution {
                first: to_index(first),
                weights,
            }
        })
        .collect()
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the position is rounded and clamped to the pixels of the row"
)]
fn pixel_index(position: f64, last: u32) -> u32 {
    position.round().clamp(0.0, f64::from(last)) as u32
}

fn lanczos(distance: f64) -> f64 {
    if distance.abs() >= LOBES {
        return 0.0;
    }
    sinc(distance) * sinc(distance / LOBES)
}

fn sinc(value: f64) -> f64 {
    if value.abs() < f64::EPSILON {
        1.0
    } else {
        let angle = PI * value;
        angle.sin() / angle
    }
}

fn resample_rows(values: &[f64], width: u32, height: u32, new_width: u32) -> Vec<f64> {
    let contributions = contributions(width, new_width);
    let source_row = to_index(width) * CHANNELS;
    let mut result = Vec::with_capacity(to_index(new_width) * to_index(height) * CHANNELS);
    for row in values.chunks(source_row) {
        for contribution in &contributions {
            let mut pixel = [0.0; CHANNELS];
            for (offset, weight) in contribution.weights.iter().enumerate() {
                let start = (contribution.first + offset) * CHANNELS;
                for channel in 0..CHANNELS {
                    pixel[channel] += row[start + channel] * weight;
                }
            }
            result.extend(pixel);
        }
    }
    result
}

fn resample_columns(values: &[f64], width: u32, height: u32, new_height: u32) -> Vec<f64> {
    let contributions = contributions(height, new_height);
    let row_length = to_index(width) * CHANNELS;
    let mut result = Vec::with_capacity(row_length * to_index(new_height));
    for contribution in &contributions {
        let mut row = vec![0.0; row_length];
        for (offset, weight) in contribution.weights.iter().enumerate() {
            let start = (contribution.first + offset) * row_length;
            for (target, source) in row.iter_mut().zip(&values[start..start + row_length]) {
                *target += source * weight;
            }
        }
        result.extend(row);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{RgbaImage, fitted_within};

    fn uniform(width: u32, height: u32, pixel: [u8; 4]) -> RgbaImage {
        let count = usize::try_from(width * height).expect("small image");
        RgbaImage::new(width, height, pixel.repeat(count)).expect("valid image")
    }

    #[test]
    fn pixels_must_match_the_dimensions() {
        assert!(RgbaImage::new(2, 2, vec![0; 16]).is_some());
        assert!(RgbaImage::new(2, 2, vec![0; 15]).is_none());
        assert!(RgbaImage::new(0, 2, Vec::new()).is_none());
    }

    #[test]
    fn the_longest_side_is_fitted_and_proportions_are_kept() {
        assert_eq!(fitted_within(800, 600, 512), (512, 384));
        assert_eq!(fitted_within(600, 800, 512), (384, 512));
        assert_eq!(fitted_within(24, 24, 256), (256, 256));
        assert_eq!(fitted_within(1000, 1, 16), (16, 1));
    }

    #[test]
    fn a_uniform_color_stays_uniform_at_any_size() {
        let blue = [20, 80, 200, 255];
        for (width, height) in [(7, 5), (31, 17), (1, 1)] {
            let resized = uniform(12, 9, blue).resized(width, height);
            assert_eq!((resized.width(), resized.height()), (width, height));
            assert!(
                resized.pixels().chunks(4).all(|pixel| pixel == blue),
                "{width}x{height}"
            );
        }
    }

    #[test]
    fn a_large_image_is_shrunk_to_the_asked_size() {
        for (source, target) in [
            ((1000, 400), (250, 100)),
            ((4000, 3000), (256, 192)),
            ((513, 777), (50, 76)),
            ((100, 100), (60, 60)),
            ((10, 10), (10, 10)),
        ] {
            let shrunk = uniform(source.0, source.1, [9, 9, 9, 255]).shrunk_to(target.0, target.1);
            assert_eq!((shrunk.width(), shrunk.height()), target, "{source:?}");
        }
    }

    #[test]
    fn shrinking_by_averaging_keeps_a_uniform_color_and_its_transparency() {
        let tinted = [20, 80, 200, 128];
        let shrunk = uniform(1000, 700, tinted).shrunk_to(100, 70);

        assert!(shrunk.pixels().chunks(4).all(|pixel| pixel == tinted));
    }

    #[test]
    fn shrinking_a_half_transparent_image_does_not_darken_its_colors() {
        let mut pixels = Vec::new();
        for _ in 0..800 {
            for column in 0..800 {
                pixels.extend(if column < 400 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 0, 0]
                });
            }
        }
        let image = RgbaImage::new(800, 800, pixels).expect("valid image");

        let shrunk = image.shrunk_to(100, 100);

        assert!(
            shrunk
                .pixels()
                .chunks(4)
                .filter(|pixel| pixel[3] > 0)
                .all(|pixel| pixel[0] >= 250 && pixel[1] <= 5 && pixel[2] <= 5)
        );
    }

    #[test]
    fn shrinking_fast_stays_close_to_the_quality_resampling() {
        let mut pixels = Vec::new();
        for row in 0..600_u32 {
            for column in 0..800_u32 {
                let value = u8::try_from(u32::midpoint(column * 255 / 799, row * 255 / 599))
                    .expect("average of two bytes");
                pixels.extend([value, 255 - value, value / 2, 255]);
            }
        }
        let image = RgbaImage::new(800, 600, pixels).expect("valid image");

        let fast = image.shrunk_to(100, 75);
        let reference = image.resized(100, 75);

        let largest_gap = fast
            .pixels()
            .iter()
            .zip(reference.pixels())
            .map(|(fast, reference)| fast.abs_diff(*reference))
            .max()
            .expect("pixels compared");
        assert!(largest_gap <= 4, "{largest_gap}");
    }

    #[test]
    fn transparent_pixels_do_not_darken_the_edges() {
        let mut pixels = Vec::new();
        for column in 0..8 {
            if column < 4 {
                pixels.extend([255, 255, 255, 255]);
            } else {
                pixels.extend([0, 0, 0, 0]);
            }
        }
        let resized = RgbaImage::new(8, 1, pixels)
            .expect("valid image")
            .resized(4, 1);
        for pixel in resized.pixels().chunks(4).filter(|pixel| pixel[3] > 0) {
            assert_eq!(&pixel[..3], [255, 255, 255]);
        }
    }

    #[test]
    fn halving_a_checkerboard_gives_grey() {
        let mut pixels = Vec::new();
        for row in 0..16 {
            for column in 0..16 {
                let value = if (row + column) % 2 == 0 { 255 } else { 0 };
                pixels.extend([value, value, value, 255]);
            }
        }
        let resized = RgbaImage::new(16, 16, pixels)
            .expect("valid image")
            .resized(8, 8);
        let centre = &resized.pixels()[(4 * 8 + 4) * 4..][..3];
        assert!(
            centre.iter().all(|value| (110..=145).contains(value)),
            "{centre:?}"
        );
    }

    #[test]
    fn a_picture_is_centered_on_a_transparent_square() {
        let red = [255, 0, 0, 255];
        let square = uniform(2, 1, red).centered_on_square(4);
        assert_eq!((square.width(), square.height()), (4, 4));
        let at = |x: usize, y: usize| &square.pixels()[(y * 4 + x) * 4..][..4];
        assert_eq!(at(1, 1), red);
        assert_eq!(at(2, 1), red);
        assert_eq!(at(0, 1), [0, 0, 0, 0]);
        assert_eq!(at(1, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn a_picture_is_centered_on_a_wider_canvas() {
        let red = [255, 0, 0, 255];
        let canvas = uniform(2, 2, red).centered_on(6, 2);
        assert_eq!((canvas.width(), canvas.height()), (6, 2));
        let at = |x: usize, y: usize| &canvas.pixels()[(y * 6 + x) * 4..][..4];
        assert_eq!(at(1, 1), [0, 0, 0, 0]);
        assert_eq!(at(2, 0), red);
        assert_eq!(at(3, 1), red);
        assert_eq!(at(4, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn cropping_keeps_the_middle_of_the_picture() {
        let mut pixels = Vec::new();
        for row in 0..4u8 {
            for column in 0..4u8 {
                pixels.extend([row, column, 0, 255]);
            }
        }
        let cropped = RgbaImage::new(4, 4, pixels)
            .expect("valid image")
            .cropped_to_center(2, 2);
        assert_eq!((cropped.width(), cropped.height()), (2, 2));
        assert_eq!(
            cropped.pixels(),
            [1, 1, 0, 255, 1, 2, 0, 255, 2, 1, 0, 255, 2, 2, 0, 255]
        );
    }

    #[test]
    fn any_part_of_a_picture_can_be_cut_out() {
        let mut pixels = Vec::new();
        for row in 0..3u8 {
            for column in 0..4u8 {
                pixels.extend([row, column, 0, 255]);
            }
        }
        let picture = RgbaImage::new(4, 3, pixels).expect("valid image");
        let corner = picture.cropped(3, 1, 5, 5);
        assert_eq!((corner.width(), corner.height()), (1, 2));
        assert_eq!(corner.pixels(), [1, 3, 0, 255, 2, 3, 0, 255]);
    }

    #[test]
    fn a_picture_is_placed_on_a_transparent_canvas_and_clipped_at_its_edge() {
        let red = [255, 0, 0, 255];
        let canvas = uniform(3, 2, red).placed_on(4, 3, 2, 2);
        assert_eq!((canvas.width(), canvas.height()), (4, 3));
        let at = |x: usize, y: usize| &canvas.pixels()[(y * 4 + x) * 4..][..4];
        assert_eq!(at(2, 2), red);
        assert_eq!(at(3, 2), red);
        assert_eq!(at(1, 2), [0, 0, 0, 0]);
        assert_eq!(at(2, 1), [0, 0, 0, 0]);
        let outside = uniform(2, 2, red).placed_on(2, 2, 5, 0);
        assert!(outside.pixels().iter().all(|value| *value == 0));
    }

    #[test]
    fn a_picture_drawn_over_another_mixes_by_its_transparency() {
        let canvas = uniform(2, 1, [0, 0, 255, 255]);
        let half_red = uniform(1, 1, [255, 0, 0, 128]);
        let mixed = half_red.placed_over(&canvas, 1, 0);
        assert_eq!(&mixed.pixels()[..4], [0, 0, 255, 255]);
        let pixel = &mixed.pixels()[4..];
        assert_eq!(pixel[3], 255);
        assert!((126..=130).contains(&pixel[0]), "{pixel:?}");
        assert!((125..=129).contains(&pixel[2]), "{pixel:?}");
    }

    #[test]
    fn blurring_spreads_a_bright_point_and_keeps_a_uniform_color() {
        let blue = [20, 80, 200, 255];
        assert!(
            uniform(9, 7, blue)
                .blurred(2)
                .pixels()
                .chunks(4)
                .all(|pixel| pixel == blue)
        );
        let mut pixels = [0, 0, 0, 255].repeat(25);
        pixels[12 * 4..12 * 4 + 3].copy_from_slice(&[255, 255, 255]);
        let spread = RgbaImage::new(5, 5, pixels)
            .expect("valid image")
            .blurred(1);
        let at = |x: usize, y: usize| spread.pixels()[(y * 5 + x) * 4];
        assert!(at(2, 2) < 255);
        assert!(at(1, 2) > 0);
        assert!(at(2, 2) >= at(0, 0));
    }

    #[test]
    fn dimming_darkens_the_colors_only() {
        let dimmed = uniform(1, 1, [200, 100, 50, 255]).dimmed(0.5);
        assert_eq!(dimmed.pixels(), [100, 50, 25, 255]);
    }

    #[test]
    fn mirroring_flips_each_row() {
        let pixels = vec![1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255];
        let flipped = RgbaImage::new(2, 2, pixels)
            .expect("valid image")
            .mirrored();
        let reds: Vec<u8> = flipped.pixels().chunks(4).map(|pixel| pixel[0]).collect();
        assert_eq!(reds, [2, 1, 4, 3]);
    }

    #[test]
    fn a_gradient_follows_its_angle() {
        let red = |image: &RgbaImage, x: usize, y: usize| image.pixels()[(y * 4 + x) * 4];
        let down = RgbaImage::linear_gradient(4, 4, ([0, 0, 0], [240, 0, 0]), 180.0);
        assert!(red(&down, 0, 0) < red(&down, 0, 3));
        assert_eq!(red(&down, 0, 1), red(&down, 3, 1));
        let right = RgbaImage::linear_gradient(4, 4, ([0, 0, 0], [240, 0, 0]), 90.0);
        assert!(red(&right, 0, 0) < red(&right, 3, 0));
        assert_eq!(red(&right, 1, 0), red(&right, 1, 3));
        let diagonal = RgbaImage::linear_gradient(4, 4, ([0, 0, 0], [240, 0, 0]), 135.0);
        assert!(red(&diagonal, 0, 0) < red(&diagonal, 3, 3));
        assert_eq!(red(&diagonal, 3, 0), red(&diagonal, 0, 3));
        let up = RgbaImage::linear_gradient(1, 2, ([0, 0, 0], [240, 0, 0]), 0.0);
        assert!(up.pixels()[0] > up.pixels()[4]);
    }

    #[test]
    fn tiles_repeat_from_any_origin() {
        let pixels = vec![1, 0, 0, 255, 2, 0, 0, 255];
        let tile = RgbaImage::new(2, 1, pixels).expect("valid image");
        let reds = |image: &RgbaImage| -> Vec<u8> {
            image.pixels().chunks(4).map(|pixel| pixel[0]).collect()
        };
        assert_eq!(reds(&tile.tiled(5, 1, 0, 0)), [1, 2, 1, 2, 1]);
        assert_eq!(reds(&tile.tiled(5, 1, 1, 0)), [2, 1, 2, 1, 2]);
        assert_eq!(reds(&tile.tiled(3, 2, -1, 0)), [2, 1, 2, 2, 1, 2]);
    }
}
