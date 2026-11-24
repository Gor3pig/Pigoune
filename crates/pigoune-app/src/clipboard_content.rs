use std::fs;
use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::thumbnails;

const PAGE_PICTURE_PIXELS: u32 = 2048;
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
    if pigoune_core::page_file(copy).is_some() {
        return thumbnails::pages_picture(copy, PAGE_PICTURE_PIXELS).ok();
    }
    if pigoune_core::is_gimp_file(copy) {
        return thumbnails::gimp_picture(copy, None).map(|(texture, _)| texture);
    }
    if pigoune_core::container_format_of(copy).is_some() {
        let image = pigoune_core::container_preview(copy)?;
        return gdk::Texture::from_bytes(&glib::Bytes::from_owned(image)).ok();
    }
    gdk::Texture::from_filename(copy).ok()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use gtk::prelude::*;

    use super::picture_of;

    fn fixture(name: &str) -> std::path::PathBuf {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures")
            .join(name);
        let directory =
            std::env::temp_dir().join(format!("pigoune-fixtures-{}", std::process::id()));
        let copy = directory.join(name);
        std::fs::create_dir_all(copy.parent().expect("a folder")).expect("a scratch folder");
        std::fs::copy(&source, &copy).expect("a copy of the fixture");
        copy
    }

    #[test]
    fn a_photoshop_image_is_copied_as_a_picture() {
        let picture = picture_of(&fixture("rle-gradient.psd")).expect("the picture is read");

        assert_eq!((picture.width(), picture.height()), (16, 8));
    }

    #[test]
    fn a_krita_or_openraster_image_is_copied_as_a_picture() {
        for name in ["gradient.kra", "gradient.ora"] {
            let picture = picture_of(&fixture(name)).expect(name);

            assert_eq!((picture.width(), picture.height()), (16, 8), "{name}");
        }
    }

    #[test]
    fn a_pdf_page_is_copied_as_a_large_picture() {
        let installed = std::process::Command::new("gs")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success());
        if !installed {
            return;
        }
        let picture = picture_of(&fixture("pages-cairo.pdf")).expect("the picture is drawn");

        assert_eq!((picture.width(), picture.height()), (2048, 1447));
    }

    #[test]
    fn a_gimp_image_is_copied_as_its_composed_picture() {
        let picture = picture_of(&fixture("gimp-basic.xcf")).expect("the picture is read");

        assert_eq!((picture.width(), picture.height()), (200, 150));
    }

    #[test]
    fn a_macos_icon_is_copied_as_its_largest_picture() {
        let picture = picture_of(&fixture("modern.icns")).expect("the picture is read");

        assert_eq!((picture.width(), picture.height()), (256, 256));
    }

    #[test]
    fn an_unreadable_photoshop_image_gives_no_picture() {
        assert!(picture_of(&fixture("no-preview.psd")).is_none());
    }
}
