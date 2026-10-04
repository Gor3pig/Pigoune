use std::f64::consts::PI;

const CHANNELS: usize = 4;
const ALPHA: usize = 3;
const LOBES: f64 = 3.0;
const BYTE_MAX: f64 = 255.0;

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
    pub fn centered_on_square(&self, side: u32) -> Self {
        let side = side.max(self.width).max(self.height);
        let side_length = to_index(side);
        let mut pixels = vec![0; side_length * side_length * CHANNELS];
        let left = to_index((side - self.width) / 2);
        let top = to_index((side - self.height) / 2);
        let row_length = to_index(self.width) * CHANNELS;
        for (row, source) in self.pixels.chunks(row_length).enumerate() {
            let start = ((top + row) * side_length + left) * CHANNELS;
            pixels[start..start + row_length].copy_from_slice(source);
        }
        Self {
            width: side,
            height: side,
            pixels,
        }
    }
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

fn to_index(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
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
}
