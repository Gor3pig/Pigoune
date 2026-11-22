use std::fs;
use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::thumbnails;

const SVG_MIME_TYPE: &str = "image/svg+xml";

pub fn provider(copies: &[PathBuf]) -> gdk::ContentProvider {
    let files: Vec<gio::File> = copies.iter().map(gio::File::for_path).collect();
    let mut providers = vec![gdk::ContentProvider::for_value(
        &gdk::FileList::from_array(&files).to_value(),
    )];
    if let [single] = copies {
        providers.extend(image_providers(single));
    }
    gdk::ContentProvider::new_union(&providers)
}

fn image_providers(copy: &PathBuf) -> Vec<gdk::ContentProvider> {
    let mut providers = Vec::new();
    let is_svg = copy
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"));
    if is_svg && let Ok(markup) = fs::read(copy) {
        providers.push(gdk::ContentProvider::for_bytes(
            SVG_MIME_TYPE,
            &glib::Bytes::from_owned(markup),
        ));
    }
    if let Some(texture) = picture_of(copy) {
        providers.push(gdk::ContentProvider::for_value(&texture.to_value()));
    }
    providers
}

fn picture_of(copy: &Path) -> Option<gdk::Texture> {
    if pigoune_core::is_photoshop_file(copy) {
        return thumbnails::photoshop_picture(copy, None);
    }
    gdk::Texture::from_filename(copy).ok()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use gtk::prelude::*;

    use super::picture_of;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures")
            .join(name)
    }

    #[test]
    fn a_photoshop_image_is_copied_as_a_picture() {
        let picture = picture_of(&fixture("rle-gradient.psd")).expect("the picture is read");

        assert_eq!((picture.width(), picture.height()), (16, 8));
    }

    #[test]
    fn an_unreadable_photoshop_image_gives_no_picture() {
        assert!(picture_of(&fixture("zip-layers-gradient.psd")).is_none());
    }
}
