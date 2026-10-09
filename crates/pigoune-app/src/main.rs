mod animation;
mod animation_player;
mod application;
mod asset_colors;
mod asset_details;
mod asset_facts;
mod asset_grid;
mod asset_object;
mod asset_preview;
mod asset_sort;
mod asset_tile;
mod background_import;
mod chart_slices;
mod clipboard_content;
mod collection_choice;
mod collection_chooser;
mod collection_drop;
mod collection_look_dialog;
mod collection_looks;
mod collection_name_dialog;
mod collection_places;
mod collection_sort;
mod color_chips;
mod config;
mod conversion_memory;
mod conversion_report;
mod custom_color_swatch;
mod desktop_bars;
mod desktop_frame;
mod desktop_panels;
mod displayed_view;
mod drag_content;
mod drag_icon;
mod drop_action;
mod drop_message;
mod drop_places;
mod dropped_content;
mod error_messages;
mod export_as_dialog;
mod export_size;
mod filter_choices;
mod filter_popover;
mod first_launch;
mod flatpak_updates;
mod found_flash;
mod frame_cache;
mod grid_columns;
mod grid_header;
mod group_mosaic;
mod health_page;
mod help_url;
mod host_path;
mod icon_sides;
mod icon_theme;
mod image_check;
mod image_conversion;
mod import_progress_dialog;
mod import_report;
mod languages;
mod library_info_dialog;
mod load_slots;
mod new_library_dialog;
mod pasted_content;
mod preferences_dialog;
mod preview_flight;
mod preview_strip;
mod query_pills;
mod recent_libraries;
mod relaunch;
mod release_notes;
mod removable_pill;
mod ring_chart;
mod screen_size;
mod search_space;
mod search_width;
mod settings;
mod shortened_label;
mod sidebar;
mod sidebar_item;
mod sidebar_row;
mod sidebar_tag_cloud;
mod smart_collection_dialog;
mod smart_collection_sort;
mod square_space;
mod stacked_bar;
mod swipe_steps;
mod tag_chooser;
mod tag_cloud;
mod tag_editor;
mod tag_input;
mod tag_summary;
mod tag_tree;
mod thumbnails;
mod toasts;
mod undo_message;
mod update_banner;
mod update_news;
mod view_setting;
mod wallpaper;
mod wallpaper_dialog;
mod wallpaper_framing;
mod wallpaper_progress_dialog;
mod wallpaper_stage;
mod wallpaper_swatches;
mod whats_new;
mod window;
mod zoom_math;
mod zoom_view;

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};
use gtk::prelude::*;
use gtk::{gio, glib};

use crate::config::{GETTEXT_PACKAGE, LOCALEDIR};

fn main() -> glib::ExitCode {
    languages::apply_chosen_language();
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
