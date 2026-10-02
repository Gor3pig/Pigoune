use gettextrs::gettext;
use gtk::glib;
use pigoune_core::{AssetFormat, Dimensions};

const MILLISECONDS_PER_SECOND: i64 = 1000;

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
        .map(|size| format!("{} × {}", size.width(), size.height()))
        .collect::<Vec<_>>()
        .join(", ")
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

    use super::{date_pattern, embedded_sizes_text};

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
