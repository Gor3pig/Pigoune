use gettextrs::pgettext;
use gtk::gdk;
use gtk::prelude::*;
use pigoune_core::AssetColor;

const CHANNELS: usize = 4;

#[must_use]
pub fn color_name(color: AssetColor) -> String {
    match color {
        AssetColor::Red => pgettext("resource color", "Red"),
        AssetColor::Orange => pgettext("resource color", "Orange"),
        AssetColor::Yellow => pgettext("resource color", "Yellow"),
        AssetColor::Green => pgettext("resource color", "Green"),
        AssetColor::Teal => pgettext("resource color", "Teal"),
        AssetColor::Blue => pgettext("resource color", "Blue"),
        AssetColor::Purple => pgettext("resource color", "Purple"),
        AssetColor::Pink => pgettext("resource color", "Pink"),
        AssetColor::Brown => pgettext("resource color", "Brown"),
        AssetColor::Black => pgettext("resource color", "Black"),
        AssetColor::Gray => pgettext("resource color", "Gray"),
        AssetColor::White => pgettext("resource color", "White"),
    }
}

#[must_use]
pub fn swatch_class(color: AssetColor) -> String {
    format!("resource-swatch-{}", color.code())
}

#[must_use]
pub fn rgba_pixels(texture: &gdk::Texture) -> Vec<u8> {
    let mut downloader = gdk::TextureDownloader::new(texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    let row_bytes = usize::try_from(texture.width()).unwrap_or_default() * CHANNELS;
    if stride == 0 || row_bytes == 0 {
        return Vec::new();
    }
    bytes
        .chunks(stride)
        .flat_map(|row| row.iter().take(row_bytes))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use pigoune_core::AssetColor;

    use super::swatch_class;

    #[test]
    fn every_color_has_its_own_swatch_class() {
        let classes: std::collections::HashSet<String> =
            AssetColor::ALL.into_iter().map(swatch_class).collect();

        assert_eq!(classes.len(), AssetColor::ALL.len());
        assert!(classes.contains("resource-swatch-red"));
    }
}
