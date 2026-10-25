use std::path::Path;

use gtk::glib;
use gtk::prelude::*;

use crate::config::FONT_DIR;

pub const CARTOON_FAMILY: &str = "Comic Neue";

const FILES: [&str; 4] = [
    "ComicNeue-Regular.ttf",
    "ComicNeue-Bold.ttf",
    "ComicNeue-Italic.ttf",
    "ComicNeue-BoldItalic.ttf",
];

pub fn register() {
    let Some(map) = gtk::Label::new(None).pango_context().font_map() else {
        return;
    };
    for file in FILES {
        if let Err(error) = map.add_font_file(Path::new(FONT_DIR).join(file)) {
            glib::g_warning!("pigoune", "The cartoon font {file} is unavailable: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{FILES, FONT_DIR};

    #[test]
    fn every_font_file_of_the_cartoon_family_is_shipped_with_its_license() {
        for file in FILES {
            let path = Path::new(FONT_DIR).join(file);
            assert!(path.is_file(), "{file} is missing");
        }
        assert!(Path::new(FONT_DIR).join("LICENSE-comic-neue").is_file());
    }
}
