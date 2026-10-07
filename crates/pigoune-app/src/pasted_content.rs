use std::path::PathBuf;

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

const NAME_TIME_FORMAT: &str = "%Y-%m-%d %H.%M.%S";

pub enum Pasted {
    Files(Vec<PathBuf>),
    Image(gdk::Texture),
    Nothing,
}

pub async fn read(clipboard: &gdk::Clipboard) -> Pasted {
    let formats = clipboard.formats();
    if formats.contains_type(gdk::FileList::static_type())
        && let Ok(value) = clipboard
            .read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT)
            .await
        && let Ok(list) = value.get::<gdk::FileList>()
    {
        let paths: Vec<PathBuf> = list.files().iter().filter_map(gio::File::path).collect();
        if !paths.is_empty() {
            return Pasted::Files(paths);
        }
    }
    if formats.contains_type(gdk::Texture::static_type())
        && let Ok(Some(texture)) = clipboard.read_texture_future().await
    {
        return Pasted::Image(texture);
    }
    Pasted::Nothing
}

pub fn image_name() -> String {
    let stamp = glib::DateTime::now_local()
        .ok()
        .and_then(|now| now.format(NAME_TIME_FORMAT).ok());
    match stamp {
        Some(stamp) => format!("{} {stamp}", gettext("Pasted image")),
        None => gettext("Pasted image"),
    }
}
