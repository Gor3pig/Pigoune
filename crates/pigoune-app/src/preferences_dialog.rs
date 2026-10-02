use adw::prelude::*;
use gtk::gio;

use crate::settings;

const RESOURCE: &str = "/io/github/gor3pig/Pigoune/ui/preferences-dialog.ui";

pub fn present(parent: &impl IsA<gtk::Widget>, settings: &gio::Settings) {
    let builder = gtk::Builder::from_resource(RESOURCE);
    for (row, key) in [
        ("show_counts_row", settings::SHOW_COUNTS),
        ("restore_last_view_row", settings::RESTORE_LAST_VIEW),
        ("confirm_empty_trash_row", settings::CONFIRM_EMPTY_TRASH),
        ("auto_empty_trash_row", settings::AUTO_EMPTY_TRASH),
    ] {
        if let Some(row) = builder.object::<adw::SwitchRow>(row) {
            settings.bind(key, &row, "active").build();
        }
    }
    if let Some(dialog) = builder.object::<adw::PreferencesDialog>("dialog") {
        dialog.present(Some(parent));
    }
}
