use gtk::gio;
use gtk::prelude::*;

use crate::config::{APP_ID, DEVELOPMENT_SCHEMAS_DIR};

pub const LAST_LIBRARY_PATH: &str = "last-library-path";
pub const WINDOW_WIDTH: &str = "window-width";
pub const WINDOW_HEIGHT: &str = "window-height";
pub const WINDOW_MAXIMIZED: &str = "window-maximized";
pub const THUMBNAIL_SIZE: &str = "thumbnail-size";
pub const SHOW_DETAILS: &str = "show-details";
pub const SORT_CRITERION: &str = "sort-criterion";
pub const SORT_REVERSED: &str = "sort-reversed";
pub const PREVIEW_BACKGROUND: &str = "preview-background";
pub const PREVIEW_BOUNDS: &str = "preview-bounds";
pub const COLLECTION_SORT: &str = "collection-sort";
pub const COLLECTION_SORT_REVERSED: &str = "collection-sort-reversed";
pub const SHOW_COUNTS: &str = "show-counts";
pub const SHOW_NAMES: &str = "show-names";
pub const RESTORE_LAST_VIEW: &str = "restore-last-view";
pub const LAST_VIEW: &str = "last-view";
pub const CONFIRM_EMPTY_TRASH: &str = "confirm-empty-trash";
pub const AUTO_EMPTY_TRASH: &str = "auto-empty-trash";

pub fn load() -> gio::Settings {
    let schema = installed_schema()
        .or_else(development_schema)
        .expect("the settings schema is installed or compiled at build time");
    gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None)
}

pub fn store_string(settings: &gio::Settings, key: &str, value: &str) {
    if let Err(error) = settings.set_string(key, value) {
        glib_warning(key, &error);
    }
}

pub fn store_int(settings: &gio::Settings, key: &str, value: i32) {
    if let Err(error) = settings.set_int(key, value) {
        glib_warning(key, &error);
    }
}

pub fn store_bool(settings: &gio::Settings, key: &str, value: bool) {
    if let Err(error) = settings.set_boolean(key, value) {
        glib_warning(key, &error);
    }
}

fn glib_warning(key: &str, error: &gtk::glib::BoolError) {
    gtk::glib::g_warning!("pigoune", "Unable to save the setting {key}: {error}");
}

fn installed_schema() -> Option<gio::SettingsSchema> {
    gio::SettingsSchemaSource::default()?.lookup(APP_ID, true)
}

fn development_schema() -> Option<gio::SettingsSchema> {
    gio::SettingsSchemaSource::from_directory(
        DEVELOPMENT_SCHEMAS_DIR,
        gio::SettingsSchemaSource::default().as_ref(),
        false,
    )
    .ok()?
    .lookup(APP_ID, false)
}
