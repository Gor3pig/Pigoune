use std::time::Duration;

use gettextrs::{gettext, ngettext};
use gtk::glib;
use pigoune_core::{AnimationTiming, Asset, AssetFormat, Dimensions};

const MILLISECONDS_PER_SECOND: i64 = 1000;
const MILLISECONDS_PER_TENTH: u128 = 100;
const TENTHS_PER_SECOND: u128 = 10;
const SUMMARY_SEPARATOR: &str = " · ";

pub fn format_name(format: AssetFormat) -> &'static str {
    match format {
        AssetFormat::Svg => "SVG",
        AssetFormat::Png => "PNG",
        AssetFormat::Jpeg => "JPEG",
        AssetFormat::Webp => "WebP",
        AssetFormat::Gif => "GIF",
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

pub fn summary_text(asset: &Asset, timing: Option<AnimationTiming>) -> String {
    let mut parts = timing.map_or_else(|| still_parts(asset), animation_parts);
    parts.push(byte_size_text(asset.byte_size));
    parts.join(SUMMARY_SEPARATOR)
}

pub fn animation_text(timing: Option<AnimationTiming>) -> String {
    timing.map_or_else(
        || gettext("Animated GIF"),
        |timing| animation_parts(timing).join(SUMMARY_SEPARATOR),
    )
}

fn still_parts(asset: &Asset) -> Vec<String> {
    let mut parts = vec![format_name(asset.format).to_owned()];
    parts.extend(asset.dimensions.map(side_by_side));
    parts
}

fn animation_parts(timing: AnimationTiming) -> Vec<String> {
    vec![
        gettext("Animated GIF"),
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

    use super::{date_pattern, duration_text, embedded_sizes_text, frames_text};

    #[test]
    fn a_duration_is_written_in_seconds_with_one_decimal() {
        assert_eq!(duration_text(Duration::from_millis(2400)), "2.4 s");
        assert_eq!(duration_text(Duration::from_millis(300)), "0.3 s");
        assert_eq!(duration_text(Duration::from_millis(12_050)), "12.1 s");
        assert_eq!(duration_text(Duration::from_secs(3)), "3.0 s");
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
