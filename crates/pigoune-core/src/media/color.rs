use super::rgb::Rgb;

const CHANNELS: usize = 4;
const CODE_SEPARATOR: char = ',';
const AVERAGE_SEPARATOR: char = '#';
const VISIBLE_ALPHA: u8 = 128;
const MIN_SHARE_PERCENT: u64 = 15;
const MAX_COLORS: usize = 3;
const BYTE_MAX: f64 = 255.0;
const GRAY_SATURATION: f64 = 0.15;
const GRAY_CHROMA: f64 = 0.08;
const BLACK_LIGHTNESS: f64 = 0.2;
const WHITE_LIGHTNESS: f64 = 0.85;
const DARKEST_HUE_LIGHTNESS: f64 = 0.12;
const LIGHTEST_HUE_LIGHTNESS: f64 = 0.94;
const BROWN_LIGHTNESS: f64 = 0.45;
const PINK_LIGHTNESS: f64 = 0.7;
const BEIGE_LIGHTNESS: f64 = 0.7;
const BEIGE_CHROMA: f64 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AssetColor {
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
    Brown,
    Beige,
    Black,
    Gray,
    White,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DominantColor {
    pub family: AssetColor,
    pub average: Rgb,
}

impl AssetColor {
    pub const ALL: [Self; 13] = [
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Teal,
        Self::Blue,
        Self::Purple,
        Self::Pink,
        Self::Brown,
        Self::Beige,
        Self::Black,
        Self::Gray,
        Self::White,
    ];

    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Teal => "teal",
            Self::Blue => "blue",
            Self::Purple => "purple",
            Self::Pink => "pink",
            Self::Brown => "brown",
            Self::Beige => "beige",
            Self::Black => "black",
            Self::Gray => "gray",
            Self::White => "white",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|color| color.code() == code)
    }

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|color| *color == self)
            .unwrap_or_default()
    }
}

#[must_use]
pub fn families_text(families: &[AssetColor]) -> String {
    families
        .iter()
        .map(|family| family.code())
        .collect::<Vec<_>>()
        .join(&CODE_SEPARATOR.to_string())
}

#[must_use]
pub fn families_from_text(text: &str) -> Vec<AssetColor> {
    text.split(CODE_SEPARATOR)
        .filter_map(AssetColor::from_code)
        .collect()
}

#[must_use]
pub fn dominant_text(colors: &[DominantColor]) -> String {
    colors
        .iter()
        .map(|color| {
            format!(
                "{}{AVERAGE_SEPARATOR}{}",
                color.family.code(),
                color.average.hex()
            )
        })
        .collect::<Vec<_>>()
        .join(&CODE_SEPARATOR.to_string())
}

#[must_use]
pub fn dominant_from_text(text: &str) -> Vec<DominantColor> {
    text.split(CODE_SEPARATOR)
        .filter_map(|part| {
            let (family, average) = part.split_once(AVERAGE_SEPARATOR)?;
            Some(DominantColor {
                family: AssetColor::from_code(family)?,
                average: Rgb::from_hex(average)?,
            })
        })
        .collect()
}

#[derive(Clone, Copy, Default)]
struct Tally {
    pixels: u64,
    red: u64,
    green: u64,
    blue: u64,
}

impl Tally {
    fn add(&mut self, red: u8, green: u8, blue: u8) {
        self.pixels += 1;
        self.red += u64::from(red);
        self.green += u64::from(green);
        self.blue += u64::from(blue);
    }

    fn average(&self) -> Rgb {
        let mean = |sum: u64| u8::try_from(sum / self.pixels.max(1)).unwrap_or(u8::MAX);
        Rgb::new(mean(self.red), mean(self.green), mean(self.blue))
    }
}

#[must_use]
pub fn dominant_colors(rgba: &[u8]) -> Vec<DominantColor> {
    let mut tallies = [Tally::default(); AssetColor::ALL.len()];
    let mut visible = 0_u64;
    for [red, green, blue, alpha] in rgba.as_chunks::<CHANNELS>().0 {
        if *alpha < VISIBLE_ALPHA {
            continue;
        }
        visible += 1;
        tallies[color_of(*red, *green, *blue).index()].add(*red, *green, *blue);
    }
    let minimum = visible * MIN_SHARE_PERCENT;
    let mut ranked: Vec<(AssetColor, Tally)> = AssetColor::ALL
        .into_iter()
        .zip(tallies)
        .filter(|(_, tally)| tally.pixels > 0 && tally.pixels * 100 >= minimum)
        .collect();
    ranked.sort_by_key(|(_, tally)| std::cmp::Reverse(tally.pixels));
    ranked
        .into_iter()
        .take(MAX_COLORS)
        .map(|(family, tally)| DominantColor {
            family,
            average: tally.average(),
        })
        .collect()
}

#[must_use]
pub fn color_of(red: u8, green: u8, blue: u8) -> AssetColor {
    let [red, green, blue] = [red, green, blue].map(|channel| f64::from(channel) / BYTE_MAX);
    let brightest = red.max(green).max(blue);
    let darkest = red.min(green).min(blue);
    let chroma = brightest - darkest;
    let lightness = f64::midpoint(brightest, darkest);
    let saturation = if chroma == 0.0 {
        0.0
    } else {
        chroma / (1.0 - (2.0 * lightness - 1.0).abs())
    };
    if chroma < GRAY_CHROMA || saturation < GRAY_SATURATION {
        return shade_of_gray(lightness);
    }
    if lightness < DARKEST_HUE_LIGHTNESS {
        return AssetColor::Black;
    }
    if lightness > LIGHTEST_HUE_LIGHTNESS {
        return AssetColor::White;
    }
    let hue = hue_of(red, green, blue, brightest, chroma);
    if (20.0..65.0).contains(&hue) && lightness > BEIGE_LIGHTNESS && chroma < BEIGE_CHROMA {
        return AssetColor::Beige;
    }
    family_of(hue, lightness)
}

fn shade_of_gray(lightness: f64) -> AssetColor {
    if lightness < BLACK_LIGHTNESS {
        AssetColor::Black
    } else if lightness > WHITE_LIGHTNESS {
        AssetColor::White
    } else {
        AssetColor::Gray
    }
}

fn hue_of(red: f64, green: f64, blue: f64, brightest: f64, chroma: f64) -> f64 {
    let sixth = if (brightest - red).abs() < f64::EPSILON {
        ((green - blue) / chroma).rem_euclid(6.0)
    } else if (brightest - green).abs() < f64::EPSILON {
        (blue - red) / chroma + 2.0
    } else {
        (red - green) / chroma + 4.0
    };
    sixth * 60.0
}

fn family_of(hue: f64, lightness: f64) -> AssetColor {
    if !(15.0..345.0).contains(&hue) {
        return if lightness > PINK_LIGHTNESS {
            AssetColor::Pink
        } else {
            AssetColor::Red
        };
    }
    if hue < 50.0 && lightness < BROWN_LIGHTNESS {
        return AssetColor::Brown;
    }
    match hue {
        hue if hue < 40.0 => AssetColor::Orange,
        hue if hue < 70.0 => AssetColor::Yellow,
        hue if hue < 165.0 => AssetColor::Green,
        hue if hue < 195.0 => AssetColor::Teal,
        hue if hue < 255.0 => AssetColor::Blue,
        hue if hue < 290.0 => AssetColor::Purple,
        hue if hue < 320.0 && lightness <= PINK_LIGHTNESS => AssetColor::Purple,
        _ => AssetColor::Pink,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AssetColor, DominantColor, Rgb, color_of, dominant_colors, dominant_from_text,
        dominant_text, families_from_text, families_text,
    };

    fn hex(value: u32) -> AssetColor {
        let [_, red, green, blue] = value.to_be_bytes();
        color_of(red, green, blue)
    }

    fn picture(parts: &[([u8; 4], usize)]) -> Vec<u8> {
        parts
            .iter()
            .flat_map(|(pixel, count)| std::iter::repeat_n(*pixel, *count).flatten())
            .collect()
    }

    fn families(rgba: &[u8]) -> Vec<AssetColor> {
        dominant_colors(rgba)
            .into_iter()
            .map(|color| color.family)
            .collect()
    }

    #[test]
    fn the_gnome_palette_falls_into_its_own_families() {
        assert_eq!(hex(0x00e0_1b24), AssetColor::Red);
        assert_eq!(hex(0x00f6_6151), AssetColor::Red);
        assert_eq!(hex(0x00ff_7800), AssetColor::Orange);
        assert_eq!(hex(0x00ff_be6f), AssetColor::Orange);
        assert_eq!(hex(0x00f6_d32d), AssetColor::Yellow);
        assert_eq!(hex(0x00f9_f06b), AssetColor::Yellow);
        assert_eq!(hex(0x0033_d17a), AssetColor::Green);
        assert_eq!(hex(0x0021_90a4), AssetColor::Teal);
        assert_eq!(hex(0x0035_84e4), AssetColor::Blue);
        assert_eq!(hex(0x0091_41ac), AssetColor::Purple);
        assert_eq!(hex(0x00d5_6199), AssetColor::Pink);
        assert_eq!(hex(0x0098_6a44), AssetColor::Brown);
        assert_eq!(hex(0x001e_1e1e), AssetColor::Black);
        assert_eq!(hex(0x009a_9996), AssetColor::Gray);
        assert_eq!(hex(0x00ff_ffff), AssetColor::White);
    }

    #[test]
    fn cream_sand_and_paper_tones_are_beige() {
        assert_eq!(hex(0x00f5_f5dc), AssetColor::Beige);
        assert_eq!(hex(0x00e8_d5b0), AssetColor::Beige);
        assert_eq!(hex(0x00ff_fdd0), AssetColor::Beige);
    }

    #[test]
    fn very_dark_or_very_light_tints_count_as_black_or_white() {
        assert_eq!(hex(0x0010_0818), AssetColor::Black);
        assert_eq!(hex(0x00fa_fcff), AssetColor::White);
        assert_eq!(hex(0x00ff_b3c0), AssetColor::Pink);
    }

    #[test]
    fn magenta_is_purple_unless_it_is_light() {
        assert_eq!(hex(0x0080_0180), AssetColor::Purple);
        assert_eq!(hex(0x00ff_00ff), AssetColor::Purple);
        assert_eq!(hex(0x00ff_80ff), AssetColor::Pink);
    }

    #[test]
    fn every_color_has_a_code_that_names_it_back() {
        for color in AssetColor::ALL {
            assert_eq!(AssetColor::from_code(color.code()), Some(color));
        }
        assert_eq!(AssetColor::from_code("gold"), None);
    }

    #[test]
    fn families_are_stored_as_text_and_read_back() {
        let chosen = [AssetColor::Red, AssetColor::Beige];

        assert_eq!(families_text(&chosen), "red,beige");
        assert_eq!(families_from_text("red,beige"), chosen);
        assert_eq!(families_from_text("red,gold"), [AssetColor::Red]);
        assert!(families_from_text("").is_empty());
    }

    #[test]
    fn main_colors_are_stored_with_their_average() {
        let colors = [
            DominantColor {
                family: AssetColor::Red,
                average: Rgb::new(0xc0, 0x39, 0x2b),
            },
            DominantColor {
                family: AssetColor::White,
                average: Rgb::new(0xff, 0xff, 0xff),
            },
        ];

        assert_eq!(dominant_text(&colors), "red#c0392b,white#ffffff");
        assert_eq!(dominant_from_text("red#c0392b,white#ffffff"), colors);
        assert!(dominant_from_text("red,white").is_empty());
        assert!(dominant_from_text("").is_empty());
    }

    #[test]
    fn a_logo_keeps_its_main_colors_from_the_largest() {
        let logo = picture(&[([255, 255, 255, 255], 30), ([224, 27, 36, 255], 60)]);

        assert_eq!(families(&logo), [AssetColor::Red, AssetColor::White]);
    }

    #[test]
    fn each_main_color_keeps_the_average_of_its_pixels() {
        let shades = picture(&[([200, 20, 20, 255], 50), ([220, 40, 40, 255], 50)]);

        assert_eq!(
            dominant_colors(&shades),
            [DominantColor {
                family: AssetColor::Red,
                average: Rgb::new(210, 30, 30),
            }]
        );
    }

    #[test]
    fn transparent_pixels_do_not_count() {
        let icon = picture(&[([0, 0, 0, 0], 900), ([30, 30, 30, 255], 100)]);

        assert_eq!(families(&icon), [AssetColor::Black]);
        assert!(dominant_colors(&picture(&[([255, 0, 0, 0], 50)])).is_empty());
    }

    #[test]
    fn a_color_needs_fifteen_percent_of_the_visible_picture() {
        let below = picture(&[([53, 132, 228, 255], 86), ([246, 211, 45, 255], 14)]);
        let enough = picture(&[([53, 132, 228, 255], 85), ([246, 211, 45, 255], 15)]);

        assert_eq!(families(&below), [AssetColor::Blue]);
        assert_eq!(families(&enough), [AssetColor::Blue, AssetColor::Yellow]);
    }

    #[test]
    fn at_most_three_colors_are_kept() {
        let flag = picture(&[
            ([224, 27, 36, 255], 25),
            ([255, 255, 255, 255], 27),
            ([53, 132, 228, 255], 26),
            ([51, 209, 122, 255], 22),
        ]);

        assert_eq!(
            families(&flag),
            [AssetColor::White, AssetColor::Blue, AssetColor::Red]
        );
    }
}
