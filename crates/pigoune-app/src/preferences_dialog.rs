use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::settings;

const RESOURCE: &str = "/io/github/gor3pig/Pigoune/ui/preferences-dialog.ui";

pub struct ThumbnailStorage {
    pub library_name: String,
    pub bytes: u64,
    pub clear: Rc<dyn Fn() -> Result<(), String>>,
}

const TILE_BACKGROUNDS: [&str; 5] = ["transparent", "white", "grey", "black", "checkerboard"];

pub fn present(
    parent: &impl IsA<gtk::Widget>,
    settings: &gio::Settings,
    thumbnails: Option<ThumbnailStorage>,
) {
    let builder = gtk::Builder::from_resource(RESOURCE);
    for (row, key) in [
        ("reopen_last_library_row", settings::REOPEN_LAST_LIBRARY),
        ("restore_last_view_row", settings::RESTORE_LAST_VIEW),
        ("show_names_row", settings::SHOW_NAMES),
        ("animate_on_hover_row", settings::ANIMATE_ON_HOVER),
        ("show_counts_row", settings::SHOW_COUNTS),
        ("show_tags_row", settings::SHOW_TAGS),
        ("show_formats_row", settings::SHOW_FORMATS),
        ("double_click_opens_row", settings::DOUBLE_CLICK_OPENS),
        ("search_everywhere_row", settings::SEARCH_EVERYWHERE),
        (
            "show_smart_collections_row",
            settings::SHOW_SMART_COLLECTIONS,
        ),
        ("confirm_empty_trash_row", settings::CONFIRM_EMPTY_TRASH),
        ("auto_empty_trash_row", settings::AUTO_EMPTY_TRASH),
    ] {
        if let Some(row) = builder.object::<adw::SwitchRow>(row) {
            settings.bind(key, &row, "active").build();
        }
    }
    if let Some(row) = builder.object::<adw::SpinRow>("recent_libraries_row") {
        follow_recent_libraries_limit(&row, settings);
    }
    if let Some(row) = builder.object::<adw::ComboRow>("tile_background_row") {
        follow_tile_background(&row, settings);
    }
    let (Some(dialog), Some(page)) = (
        builder.object::<adw::PreferencesDialog>("dialog"),
        builder.object::<adw::PreferencesPage>("page"),
    ) else {
        return;
    };
    if let Some(thumbnails) = thumbnails {
        offer_thumbnail_cleaning(&builder, &dialog, thumbnails);
    }
    dialog.connect_map(move |dialog| fit_to_content(dialog, &page));
    dialog.present(Some(parent));
}

fn follow_recent_libraries_limit(row: &adw::SpinRow, settings: &gio::Settings) {
    row.set_value(f64::from(settings.int(settings::RECENT_LIBRARIES_LIMIT)));
    row.connect_value_notify(glib::clone!(
        #[strong]
        settings,
        move |row| {
            settings::store_int(
                &settings,
                settings::RECENT_LIBRARIES_LIMIT,
                whole_number(row.value()),
            );
        }
    ));
}

fn follow_tile_background(row: &adw::ComboRow, settings: &gio::Settings) {
    let current = settings.string(settings::TILE_BACKGROUND);
    let position = TILE_BACKGROUNDS
        .iter()
        .position(|background| *background == current.as_str())
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0);
    row.set_selected(position);
    row.connect_selected_notify(glib::clone!(
        #[strong]
        settings,
        move |row| {
            let chosen = usize::try_from(row.selected())
                .ok()
                .and_then(|index| TILE_BACKGROUNDS.get(index))
                .copied()
                .unwrap_or(TILE_BACKGROUNDS[0]);
            settings::store_string(&settings, settings::TILE_BACKGROUND, chosen);
        }
    ));
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the spin row holds small whole numbers"
)]
fn whole_number(value: f64) -> i32 {
    value.round() as i32
}

fn offer_thumbnail_cleaning(
    builder: &gtk::Builder,
    dialog: &adw::PreferencesDialog,
    thumbnails: ThumbnailStorage,
) {
    let (Some(group), Some(row), Some(button)) = (
        builder.object::<adw::PreferencesGroup>("storage_group"),
        builder.object::<adw::ActionRow>("thumbnails_row"),
        builder.object::<gtk::Button>("clear_thumbnails_button"),
    ) else {
        return;
    };
    group.set_visible(true);
    show_thumbnail_bytes(&row, &button, &thumbnails.library_name, thumbnails.bytes);
    button.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        #[weak]
        row,
        move |button| {
            let message = match (thumbnails.clear)() {
                Ok(()) => {
                    show_thumbnail_bytes(&row, button, &thumbnails.library_name, 0);
                    freed_text(thumbnails.bytes)
                }
                Err(reason) => failure_text(&reason),
            };
            dialog.add_toast(adw::Toast::new(&message));
        }
    ));
}

fn freed_text(bytes: u64) -> String {
    gettext("{size} freed, thumbnails are made again when needed")
        .replace("{size}", &glib::format_size(bytes))
}

fn failure_text(reason: &str) -> String {
    gettext("Unable to clear the thumbnails: {reason}").replace("{reason}", reason)
}

fn show_thumbnail_bytes(
    row: &adw::ActionRow,
    button: &gtk::Button,
    library_name: &str,
    bytes: u64,
) {
    row.set_subtitle(
        &gettext("{size} used by “{name}”")
            .replace("{size}", &glib::format_size(bytes))
            .replace("{name}", library_name),
    );
    button.set_sensitive(bytes > 0);
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
