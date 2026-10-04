use std::time::Duration;

use gettextrs::{gettext, ngettext};
use gtk::glib;
use pigoune_core::{AnimationTiming, Asset, AssetFormat, Dimensions};

const MILLISECONDS_PER_SECOND: i64 = 1000;
const MILLISECONDS_PER_TENTH: u128 = 100;
const TENTHS_PER_SECOND: u128 = 10;
pub const SUMMARY_SEPARATOR: &str = " · ";

pub fn format_name(format: AssetFormat) -> &'static str {
    match format {
        AssetFormat::Svg => "SVG",
        AssetFormat::Png => "PNG",
        AssetFormat::Jpeg => "JPEG",
        AssetFormat::Webp => "WebP",
        AssetFormat::Avif => "AVIF",
        AssetFormat::Jxl => "JPEG XL",
        AssetFormat::Gif => "GIF",
        AssetFormat::Tiff => "TIFF",
        AssetFormat::Bmp => "BMP",
        AssetFormat::Ico => "ICO",
    }
}

pub fn dimensions_text(dimensions: Option<Dimensions>) -> String {
    dimensions.map_or_else(
        || gettext("Unknown dimensions"),
        |dimensions| {
            gettext("{width} × {height} pixels")
                .replace("{width}", &dimensions.width().to_string())
                .replace("{height}", &dimensions.height().to_string())
        },
    )
}

pub fn embedded_sizes_text(sizes: &[Dimensions]) -> String {
    sizes
        .iter()
        .map(|size| side_by_side(*size))
        .collect::<Vec<_>>()
        .join(", ")
}

fn side_by_side(size: Dimensions) -> String {
    format!("{} × {}", size.width(), size.height())
}

pub fn animation_badge_text(format: AssetFormat) -> &'static str {
    match format {
        AssetFormat::Png => "APNG",
        _ => format_name(format),
    }
}

pub fn summary_text(asset: &Asset, timing: Option<AnimationTiming>) -> String {
    let mut parts = timing.map_or_else(
        || still_parts(asset),
        |timing| animation_parts(asset.format, timing),
    );
    parts.push(byte_size_text(asset.byte_size));
    parts.join(SUMMARY_SEPARATOR)
}

pub fn group_summary_text(assets: &[&Asset]) -> String {
    let total: u64 = assets.iter().map(|asset| asset.byte_size).sum();
    [
        formats_text(assets.iter().map(|asset| asset.format)),
        gettext("{size} in total").replace("{size}", &byte_size_text(total)),
    ]
    .join(SUMMARY_SEPARATOR)
}

fn formats_text(formats: impl Iterator<Item = AssetFormat>) -> String {
    let mut seen = Vec::new();
    for format in formats {
        if !seen.contains(&format) {
            seen.push(format);
        }
    }
    seen.iter()
        .map(|format| format_name(*format))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn animation_text(format: AssetFormat, timing: Option<AnimationTiming>) -> String {
    timing.map_or_else(
        || animated_format_text(format),
        |timing| animation_parts(format, timing).join(SUMMARY_SEPARATOR),
    )
}

fn animated_format_text(format: AssetFormat) -> String {
    gettext("Animated {format}").replace("{format}", format_name(format))
}

fn still_parts(asset: &Asset) -> Vec<String> {
    let mut parts = vec![format_name(asset.format).to_owned()];
    parts.extend(asset.dimensions.map(side_by_side));
    parts
}

fn animation_parts(format: AssetFormat, timing: AnimationTiming) -> Vec<String> {
    vec![
        animated_format_text(format),
        frames_text(timing.frames),
        duration_text(timing.duration),
    ]
}

fn frames_text(frames: usize) -> String {
    ngettext(
        "{count} frame",
        "{count} frames",
        u32::try_from(frames).unwrap_or(u32::MAX),
    )
    .replace("{count}", &frames.to_string())
}

fn duration_text(duration: Duration) -> String {
    let tenths = (duration.as_millis() + MILLISECONDS_PER_TENTH / 2) / MILLISECONDS_PER_TENTH;
    gettext("{seconds}.{tenths} s")
        .replace("{seconds}", &(tenths / TENTHS_PER_SECOND).to_string())
        .replace("{tenths}", &(tenths % TENTHS_PER_SECOND).to_string())
}

pub fn byte_size_text(byte_size: u64) -> String {
    glib::format_size(byte_size).to_string()
}

pub fn added_at_text(added_at_unix_ms: i64) -> String {
    glib::DateTime::from_unix_local(added_at_unix_ms / MILLISECONDS_PER_SECOND)
        .and_then(|moment| moment.format(&date_pattern()))
        .map_or_else(|_| gettext("Unknown date"), |text| text.to_string())
}

fn date_pattern() -> String {
    gettext("%B %-d, %Y at %H:%M")
}

#[cfg(test)]
mod tests {
    use pigoune_core::Dimensions;

    use gtk::glib;

    use std::time::Duration;

    use pigoune_core::AssetFormat;

    use super::{
        animation_badge_text, date_pattern, duration_text, embedded_sizes_text, formats_text,
        frames_text,
    };

    #[test]
    fn each_format_is_listed_once_in_order_of_appearance() {
        let formats = [
            AssetFormat::Svg,
            AssetFormat::Png,
            AssetFormat::Svg,
            AssetFormat::Gif,
            AssetFormat::Png,
        ];
        assert_eq!(formats_text(formats.into_iter()), "SVG, PNG, GIF");
        assert_eq!(formats_text(std::iter::empty()), "");
    }

    #[test]
    fn a_duration_is_written_in_seconds_with_one_decimal() {
        assert_eq!(duration_text(Duration::from_millis(2400)), "2.4 s");
        assert_eq!(duration_text(Duration::from_millis(300)), "0.3 s");
        assert_eq!(duration_text(Duration::from_millis(12_050)), "12.1 s");
        assert_eq!(duration_text(Duration::from_secs(3)), "3.0 s");
    }

    #[test]
    fn an_animated_png_is_badged_by_its_own_name() {
        assert_eq!(animation_badge_text(AssetFormat::Png), "APNG");
        assert_eq!(animation_badge_text(AssetFormat::Webp), "WebP");
        assert_eq!(animation_badge_text(AssetFormat::Gif), "GIF");
    }

    #[test]
    fn the_number_of_frames_agrees_with_its_count() {
        assert_eq!(frames_text(1), "1 frame");
        assert_eq!(frames_text(24), "24 frames");
    }

    #[test]
    fn the_date_pattern_is_understood_by_glib() {
        let moment = glib::DateTime::from_utc(2026, 10, 2, 9, 5, 0.0).expect("valid date");
        let text = moment.format(&date_pattern()).expect("pattern understood");
        assert!(text.contains(" 2, 2026 at 09:05"), "{text}");
    }

    #[test]
    fn embedded_sizes_are_listed_in_order() {
        let sizes = [16, 32, 256].map(|side| Dimensions::new(side, side).expect("valid"));
        assert_eq!(embedded_sizes_text(&sizes), "16 × 16, 32 × 32, 256 × 256");
    }
}
