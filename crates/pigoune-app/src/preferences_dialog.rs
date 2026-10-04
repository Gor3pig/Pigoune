use adw::prelude::*;
use gtk::gio;

use crate::settings;

const RESOURCE: &str = "/io/github/gor3pig/Pigoune/ui/preferences-dialog.ui";

pub fn present(parent: &impl IsA<gtk::Widget>, settings: &gio::Settings) {
    let builder = gtk::Builder::from_resource(RESOURCE);
    for (row, key) in [
        ("reopen_last_library_row", settings::REOPEN_LAST_LIBRARY),
        ("restore_last_view_row", settings::RESTORE_LAST_VIEW),
        ("show_names_row", settings::SHOW_NAMES),
        ("animate_on_hover_row", settings::ANIMATE_ON_HOVER),
        ("show_counts_row", settings::SHOW_COUNTS),
        ("confirm_empty_trash_row", settings::CONFIRM_EMPTY_TRASH),
        ("auto_empty_trash_row", settings::AUTO_EMPTY_TRASH),
    ] {
        if let Some(row) = builder.object::<adw::SwitchRow>(row) {
            settings.bind(key, &row, "active").build();
        }
    }
    let (Some(dialog), Some(page)) = (
        builder.object::<adw::PreferencesDialog>("dialog"),
        builder.object::<adw::PreferencesPage>("page"),
    ) else {
        return;
    };
    dialog.connect_map(move |dialog| fit_to_content(dialog, &page));
    dialog.present(Some(parent));
}

fn fit_to_content(dialog: &adw::PreferencesDialog, page: &adw::PreferencesPage) {
    let width = dialog.content_width();
    let content = descendant::<gtk::Viewport>(page.upcast_ref()).map_or(0, |viewport| {
        viewport.measure(gtk::Orientation::Vertical, width).1
    });
    let header = descendant::<adw::HeaderBar>(dialog.upcast_ref()).map_or(0, |header| {
        header.measure(gtk::Orientation::Vertical, width).1
    });
    if content > 0 {
        dialog.set_content_height(content + header);
    }
}

fn descendant<T: IsA<gtk::Widget>>(widget: &gtk::Widget) -> Option<T> {
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Ok(found) = current.clone().downcast::<T>() {
            return Some(found);
        }
        if let Some(found) = descendant::<T>(&current) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}
