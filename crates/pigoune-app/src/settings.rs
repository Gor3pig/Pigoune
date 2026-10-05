use gtk::gio;
use gtk::prelude::*;

use crate::config::{APP_ID, DEVELOPMENT_SCHEMAS_DIR};

pub const LAST_LIBRARY_PATH: &str = "last-library-path";
pub const RECENT_LIBRARIES: &str = "recent-libraries";
pub const RECENT_LIBRARIES_LIMIT: &str = "recent-libraries-limit";
pub const WINDOW_WIDTH: &str = "window-width";
pub const WINDOW_HEIGHT: &str = "window-height";
pub const WINDOW_MAXIMIZED: &str = "window-maximized";
pub const THUMBNAIL_SIZE: &str = "thumbnail-size";
pub const SHOW_DETAILS: &str = "show-details";
pub const SORT_CRITERION: &str = "sort-criterion";
pub const SORT_REVERSED: &str = "sort-reversed";
pub const PREVIEW_BACKGROUND: &str = "preview-background";
pub const PREVIEW_BOUNDS: &str = "preview-bounds";
pub const PREVIEW_PIXEL_GRID: &str = "preview-pixel-grid";
pub const PREVIEW_DETAILS: &str = "preview-details";
pub const PREVIEW_STRIP: &str = "preview-strip";
pub const SIDEBAR_COLLECTIONS_EXPANDED: &str = "sidebar-collections-expanded";
pub const SIDEBAR_SMART_COLLECTIONS_EXPANDED: &str = "sidebar-smart-collections-expanded";
pub const SIDEBAR_TAGS_EXPANDED: &str = "sidebar-tags-expanded";
pub const DETAILS_ORGANIZATION_EXPANDED: &str = "details-organization-expanded";
pub const DETAILS_CREDITS_EXPANDED: &str = "details-credits-expanded";
pub const DETAILS_FILE_EXPANDED: &str = "details-file-expanded";
pub const COLLECTION_SORT: &str = "collection-sort";
pub const COLLECTION_SORT_REVERSED: &str = "collection-sort-reversed";
pub const SMART_COLLECTION_SORT: &str = "smart-collection-sort";
pub const SMART_COLLECTION_SORT_REVERSED: &str = "smart-collection-sort-reversed";
pub const SHOW_SMART_COLLECTIONS: &str = "show-smart-collections";
pub const SHOW_TAGS: &str = "show-tags";
pub const TILE_BACKGROUND: &str = "tile-background";
pub const SHOW_FORMATS: &str = "show-formats";
pub const DOUBLE_CLICK_OPENS: &str = "double-click-opens";
pub const SEARCH_EVERYWHERE: &str = "search-everywhere";
pub const SHOW_COUNTS: &str = "show-counts";
pub const SHOW_NAMES: &str = "show-names";
pub const RESTORE_LAST_VIEW: &str = "restore-last-view";
pub const REOPEN_LAST_LIBRARY: &str = "reopen-last-library";
pub const ANIMATE_ON_HOVER: &str = "animate-on-hover";
pub const LAST_VIEW: &str = "last-view";
pub const CONFIRM_EMPTY_TRASH: &str = "confirm-empty-trash";
pub const AUTO_EMPTY_TRASH: &str = "auto-empty-trash";
pub const EXPORT_FORMAT: &str = "export-format";
pub const EXPORT_QUALITY: &str = "export-quality";
pub const EXPORT_BACKGROUND: &str = "export-background";
pub const EXPORT_KEEP_TRANSPARENCY: &str = "export-keep-transparency";
pub const EXPORT_SIZE_UNIT: &str = "export-size-unit";
pub const EXPORT_ICON_SIZES: &str = "export-icon-sizes";

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

pub fn store_value(settings: &gio::Settings, key: &str, value: &gtk::glib::Variant) {
    if let Err(error) = settings.set_value(key, value) {
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
