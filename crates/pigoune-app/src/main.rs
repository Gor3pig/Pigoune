mod animation;
mod application;
mod asset_details;
mod asset_facts;
mod asset_grid;
mod asset_object;
mod asset_preview;
mod asset_sort;
mod asset_tile;
mod background_import;
mod clipboard_content;
mod collection_choice;
mod collection_drop;
mod collection_editor;
mod collection_name_dialog;
mod collection_sort;
mod config;
mod drag_content;
mod drag_icon;
mod drop_message;
mod error_messages;
mod filter_popover;
mod grid_header;
mod image_check;
mod import_progress_dialog;
mod import_report;
mod new_library_dialog;
mod preferences_dialog;
mod settings;
mod sidebar;
mod sidebar_item;
mod sidebar_row;
mod tag_editor;
mod tag_input;
mod thumbnails;
mod undo_message;
mod view_setting;
mod window;
mod zoom_math;
mod zoom_view;

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};
use gtk::prelude::*;
use gtk::{gio, glib};

use crate::config::{GETTEXT_PACKAGE, LOCALEDIR};

fn main() -> glib::ExitCode {
    setup_translations();

    gio::resources_register_include!("pigoune.gresource")
        .expect("compiled resources are embedded in the binary");

    application::build().run()
}

fn setup_translations() {
    let result = bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR)
        .and_then(|_| bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8"))
        .and_then(|_| textdomain(GETTEXT_PACKAGE));
    if let Err(error) = result {
        glib::g_warning!("pigoune", "Translations are unavailable: {error}");
    }
}
