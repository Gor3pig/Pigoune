use std::fs;
use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

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
    if let Ok(texture) = gdk::Texture::from_filename(copy) {
        providers.push(gdk::ContentProvider::for_value(&texture.to_value()));
    }
    providers
}
