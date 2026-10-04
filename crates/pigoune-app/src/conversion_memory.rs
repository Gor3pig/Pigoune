use gtk::gio;
use gtk::prelude::*;

use crate::export_size::{CustomSize, SizeUnit};
use crate::icon_sides::IconSides;
use crate::image_conversion::{ConversionSettings, TargetFormat};
use crate::settings;

const PIXELS: &str = "pixels";
const PERCENT: &str = "percent";

pub fn load(stored: &gio::Settings, natural: (u32, u32)) -> ConversionSettings {
    let defaults = ConversionSettings::default();
    ConversionSettings {
        format: format_of(&stored.string(settings::EXPORT_FORMAT)).unwrap_or(defaults.format),
        quality: u8::try_from(stored.int(settings::EXPORT_QUALITY)).unwrap_or(defaults.quality),
        background: color_of(&stored.string(settings::EXPORT_BACKGROUND))
            .unwrap_or(defaults.background),
        custom: CustomSize::original(
            if stored.string(settings::EXPORT_SIZE_UNIT) == PIXELS {
                SizeUnit::Pixels
            } else {
                SizeUnit::Percent
            },
            natural,
        ),
        keep_transparency: stored.boolean(settings::EXPORT_KEEP_TRANSPARENCY),
        icon_sides: icon_sides_of(&stored.get::<Vec<i32>>(settings::EXPORT_ICON_SIZES)),
    }
}

pub fn store(stored: &gio::Settings, chosen: &ConversionSettings) {
    settings::store_string(stored, settings::EXPORT_FORMAT, code_of(chosen.format));
    settings::store_int(stored, settings::EXPORT_QUALITY, i32::from(chosen.quality));
    settings::store_string(
        stored,
        settings::EXPORT_BACKGROUND,
        &hex_of(chosen.background),
    );
    settings::store_bool(
        stored,
        settings::EXPORT_KEEP_TRANSPARENCY,
        chosen.keep_transparency,
    );
    let unit = match chosen.custom.unit {
        SizeUnit::Pixels => PIXELS,
        SizeUnit::Percent => PERCENT,
    };
    settings::store_string(stored, settings::EXPORT_SIZE_UNIT, unit);
    let sides: Vec<i32> = chosen
        .icon_sides
        .sides()
        .filter_map(|side| i32::try_from(side).ok())
        .collect();
    settings::store_value(stored, settings::EXPORT_ICON_SIZES, &sides.to_variant());
}

fn code_of(format: TargetFormat) -> &'static str {
    match format {
        TargetFormat::Png => "png",
        TargetFormat::Jpeg => "jpeg",
        TargetFormat::Webp => "webp",
        TargetFormat::Avif => "avif",
        TargetFormat::Ico => "ico",
    }
}

fn format_of(code: &str) -> Option<TargetFormat> {
    TargetFormat::ALL
        .into_iter()
        .find(|format| code_of(*format) == code)
}

fn hex_of([red, green, blue]: [u8; 3]) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

fn color_of(hex: &str) -> Option<[u8; 3]> {
    let digits = hex.strip_prefix('#').filter(|digits| digits.len() == 6)?;
    let channel = |start: usize| u8::from_str_radix(digits.get(start..start + 2)?, 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

fn icon_sides_of(sides: &[i32]) -> IconSides {
    sides
        .iter()
        .filter_map(|side| u32::try_from(*side).ok())
        .fold(IconSides::none(), |chosen, side| chosen.with(side, true))
}

#[cfg(test)]
mod tests {
    use super::{code_of, color_of, format_of, hex_of, icon_sides_of};
    use crate::image_conversion::TargetFormat;

    #[test]
    fn every_format_survives_a_round_trip_through_its_code() {
        for format in TargetFormat::ALL {
            assert_eq!(format_of(code_of(format)), Some(format));
        }
        assert_eq!(format_of("gif"), None);
    }

    #[test]
    fn a_color_survives_a_round_trip_through_text() {
        assert_eq!(hex_of([255, 128, 0]), "#ff8000");
        assert_eq!(color_of("#ff8000"), Some([255, 128, 0]));
        for broken in ["", "ff8000", "#ff80", "#gg8000", "#ff80001"] {
            assert_eq!(color_of(broken), None, "{broken}");
        }
    }

    #[test]
    fn only_known_icon_sizes_are_restored() {
        let sides: Vec<u32> = icon_sides_of(&[16, 100, 64, -3]).sides().collect();
        assert_eq!(sides, [16, 64]);
        assert!(icon_sides_of(&[]).is_empty());
    }
}
