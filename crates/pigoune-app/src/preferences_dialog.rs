use adw::prelude::*;
use gtk::gio;

use crate::settings;

const RESOURCE: &str = "/io/github/gor3pig/Pigoune/ui/preferences-dialog.ui";
const BAR_POSITIONS: [&str; 2] = [settings::BAR_AT_TOP, settings::BAR_AT_BOTTOM];

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
    if let Some(row) = builder.object::<adw::ComboRow>("bar_position_row") {
        bind_bar_position(settings, &row);
    }
    if let Some(dialog) = builder.object::<adw::PreferencesDialog>("dialog") {
        dialog.present(Some(parent));
    }
}

fn bind_bar_position(settings: &gio::Settings, row: &adw::ComboRow) {
    settings
        .bind(settings::BAR_POSITION, row, "selected")
        .mapping(|variant, _| {
            let position = variant.str()?;
            let index = BAR_POSITIONS.iter().position(|known| *known == position)?;
            Some(u32::try_from(index).ok()?.to_value())
        })
        .set_mapping(|value, _| {
            let index = usize::try_from(value.get::<u32>().ok()?).ok()?;
            Some(BAR_POSITIONS.get(index)?.to_variant())
        })
        .build();
}
