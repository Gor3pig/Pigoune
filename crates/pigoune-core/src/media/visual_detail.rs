use crate::raster::RgbaImage;

use super::visual_hash::{
    VisualHash, cell_bounds, trimmed_to_visible, visual_hash, without_plain_background,
};

const CHANNELS: usize = 4;
pub const DETAIL_SIDE: usize = 16;
pub const DETAIL_BYTES: usize = DETAIL_SIDE * DETAIL_SIDE * CHANNELS;
const FAINT_ALPHA: f64 = 8.0;
const BYTE_MAX: f64 = 255.0;
const COLOR_CHANNELS: f64 = 3.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualDetail(Box<[u8]>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualAnalysis {
    pub hash: VisualHash,
    pub detail: VisualDetail,
}

#[must_use]
pub fn visual_analysis(image: &RgbaImage) -> Option<VisualAnalysis> {
    Some(VisualAnalysis {
        hash: visual_hash(image)?,
        detail: visual_detail(image)?,
    })
}

impl VisualDetail {
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        (bytes.len() == DETAIL_BYTES).then(|| Self(bytes.into()))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn gap(&self, other: &Self) -> f64 {
        let cells = self
            .0
            .as_chunks::<CHANNELS>()
            .0
            .iter()
            .zip(other.0.as_chunks::<CHANNELS>().0.iter());
        let total: f64 = cells.map(|(first, second)| cell_gap(first, second)).sum();
        total / f64::from(u32::try_from(DETAIL_SIDE * DETAIL_SIDE).unwrap_or(1))
    }
}

fn cell_gap(first: &[u8], second: &[u8]) -> f64 {
    let (first_alpha, second_alpha) = (f64::from(first[3]), f64::from(second[3]));
    let alpha_gap = (first_alpha - second_alpha).abs();
    if first_alpha < FAINT_ALPHA || second_alpha < FAINT_ALPHA {
        return alpha_gap;
    }
    let colors: f64 = first[..3]
        .iter()
        .zip(&second[..3])
        .map(|(a, b)| (f64::from(*a) - f64::from(*b)).abs())
        .sum::<f64>()
        / COLOR_CHANNELS;
    let weight = first_alpha.max(second_alpha) / BYTE_MAX;
    weight * colors + alpha_gap
}

#[must_use]
pub fn visual_detail(image: &RgbaImage) -> Option<VisualDetail> {
    let keyed = without_plain_background(image);
    let trimmed = trimmed_to_visible(&keyed)?;
    let (width, height) = (trimmed.width() as usize, trimmed.height() as usize);
    let pixels = trimmed.pixels();
    let mut detail = Vec::with_capacity(DETAIL_BYTES);
    for row in 0..DETAIL_SIDE {
        let (first_y, last_y) = cell_bounds(row, DETAIL_SIDE, height);
        for column in 0..DETAIL_SIDE {
            let (first_x, last_x) = cell_bounds(column, DETAIL_SIDE, width);
            let (mut red, mut green, mut blue, mut alpha, mut count) =
                (0_u64, 0_u64, 0_u64, 0_u64, 0_u64);
            for y in first_y..last_y {
                for x in first_x..last_x {
                    let start = (y * width + x) * CHANNELS;
                    let weight = u64::from(pixels[start + 3]);
                    red += u64::from(pixels[start]) * weight;
                    green += u64::from(pixels[start + 1]) * weight;
                    blue += u64::from(pixels[start + 2]) * weight;
                    alpha += weight;
                    count += 1;
                }
            }
            let average =
                |sum: u64| u8::try_from(sum.checked_div(alpha).unwrap_or(0)).unwrap_or(u8::MAX);
            detail.extend_from_slice(&[
                average(red),
                average(green),
                average(blue),
                u8::try_from(alpha.checked_div(count).unwrap_or(0)).unwrap_or(u8::MAX),
            ]);
        }
    }
    Some(VisualDetail(detail.into()))
}

#[cfg(test)]
mod tests {
    use super::{DETAIL_BYTES, VisualDetail, visual_detail};
    use crate::raster::RgbaImage;

    const CLEAR: [u8; 4] = [0, 0, 0, 0];
    const DARK: [u8; 4] = [30, 30, 30, 255];
    const RED: [u8; 4] = [200, 40, 40, 255];
    const BLUE: [u8; 4] = [40, 60, 200, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];

    fn disc(u: f64, v: f64) -> bool {
        (u - 0.5).powi(2) + (v - 0.5).powi(2) < 0.16
    }

    fn corner(u: f64, v: f64) -> bool {
        u < 0.35 || v > 0.65
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

    fn detail(image: &RgbaImage) -> VisualDetail {
        visual_detail(image).expect("a picture with something in it")
    }

    #[test]
    fn a_reduced_copy_is_almost_the_same_detail() {
        let original = picture(256, 0.0, CLEAR, RED, disc);
        let gap = detail(&original).gap(&detail(&original.resized(64, 64)));
        assert!(gap < 10.0, "{gap}");
    }

    #[test]
    fn margins_do_not_change_the_detail() {
        let plain = detail(&picture(256, 0.0, CLEAR, RED, corner));
        let framed = detail(&picture(256, 0.12, CLEAR, RED, corner));
        assert!(plain.gap(&framed) < 4.0, "{}", plain.gap(&framed));
    }

    #[test]
    fn another_drawing_is_far_away() {
        let first = detail(&picture(256, 0.0, CLEAR, RED, disc));
        let second = detail(&picture(256, 0.0, CLEAR, RED, corner));
        assert!(first.gap(&second) > 20.0, "{}", first.gap(&second));
    }

    #[test]
    fn the_same_drawing_in_another_color_is_far_away() {
        let red = detail(&picture(256, 0.0, CLEAR, RED, corner));
        let blue = detail(&picture(256, 0.0, CLEAR, BLUE, corner));
        assert!(red.gap(&blue) > 20.0, "{}", red.gap(&blue));
    }

    #[test]
    fn an_inverted_drawing_is_far_away() {
        let dark = detail(&picture(256, 0.0, [255, 255, 255, 255], DARK, disc));
        let light = detail(&picture(256, 0.0, DARK, WHITE, disc));
        assert!(dark.gap(&light) > 20.0, "{}", dark.gap(&light));
    }

    #[test]
    fn nothing_visible_gives_no_detail() {
        assert!(visual_detail(&picture(64, 0.0, CLEAR, CLEAR, disc)).is_none());
    }

    #[test]
    fn bytes_round_trip_and_wrong_lengths_are_refused() {
        let original = detail(&picture(128, 0.0, CLEAR, RED, disc));
        assert_eq!(original.as_bytes().len(), DETAIL_BYTES);
        assert_eq!(
            VisualDetail::from_bytes(original.as_bytes()),
            Some(original)
        );
        assert_eq!(VisualDetail::from_bytes(&[1, 2, 3]), None);
    }
}
