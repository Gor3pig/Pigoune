use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::config::{APP_ID, VERSION};
use crate::settings;

const DIALOG_WIDTH: i32 = 560;
const APP_ICON_PIXELS: i32 = 96;

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Nothing,
    RememberOnly,
    Show,
}

struct Point {
    icon: &'static str,
    title: String,
    text: String,
}

pub fn decide(
    last_seen: &str,
    running: &str,
    used_before: bool,
    enabled: bool,
    has_points: bool,
) -> Decision {
    if last_seen == running {
        Decision::Nothing
    } else if (last_seen.is_empty() && !used_before) || !enabled || !has_points {
        Decision::RememberOnly
    } else {
        Decision::Show
    }
}

pub fn short_version(version: &str) -> String {
    version.split('.').take(2).collect::<Vec<_>>().join(".")
}

pub fn show_after_update(parent: &impl IsA<gtk::Widget>, settings: &gio::Settings) {
    let points = points_of(VERSION);
    let used_before = !settings.string(settings::LAST_LIBRARY_PATH).is_empty()
        || !settings.strv(settings::RECENT_LIBRARIES).is_empty();
    let decision = decide(
        &settings.string(settings::LAST_SEEN_VERSION),
        VERSION,
        used_before,
        settings.boolean(settings::SHOW_WHATS_NEW),
        !points.is_empty(),
    );
    if decision == Decision::Nothing {
        return;
    }
    settings::store_string(settings, settings::LAST_SEEN_VERSION, VERSION);
    if decision == Decision::Show {
        present(parent, &points);
    }
}

fn points_of(version: &str) -> Vec<Point> {
    match version {
        "2.4.0" => vec![
            Point {
                icon: "dialog-information-symbolic",
                title: gettext("News After Each Update"),
                text: gettext(
                    "This window sums up the main news the first time a new version starts. You can turn it off in the Preferences.",
                ),
            },
            Point {
                icon: "drive-harddisk-symbolic",
                title: gettext("Thumbnails Moved"),
                text: gettext(
                    "Clearing the thumbnails is now done in the library information, in the Storage tab, next to the space they take.",
                ),
            },
        ],
        _ => Vec::new(),
    }
}

fn present(parent: &impl IsA<gtk::Widget>, points: &[Point]) {
    let heading =
        gettext("What’s New in Pigoune {version}").replace("{version}", &short_version(VERSION));
    let icon = gtk::Image::builder()
        .icon_name(APP_ID)
        .pixel_size(APP_ICON_PIXELS)
        .halign(gtk::Align::Center)
        .build();
    let title = gtk::Label::builder()
        .label(heading)
        .css_classes(["title-1"])
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    for point in points {
        let row = adw::ActionRow::builder()
            .title(&point.title)
            .subtitle(&point.text)
            .build();
        let icon = gtk::Image::builder()
            .icon_name(point.icon)
            .css_classes(["accent"])
            .build();
        row.add_prefix(&icon);
        list.append(&row);
    }
    let button = gtk::Button::builder()
        .label(gettext("_Continue"))
        .use_underline(true)
        .halign(gtk::Align::Center)
        .css_classes(["pill", "suggested-action"])
        .build();
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    for child in [
        icon.upcast_ref::<gtk::Widget>(),
        title.upcast_ref(),
        list.upcast_ref(),
        button.upcast_ref(),
    ] {
        content.append(child);
    }
    let scrolled = gtk::ScrolledWindow::builder()
        .child(&content)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .build();
    let dialog = adw::Dialog::builder()
        .child(&scrolled)
        .content_width(DIALOG_WIDTH)
        .build();
    button.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| {
            dialog.close();
        }
    ));
    dialog.present(Some(parent));
}

#[cfg(test)]
mod tests {
    use super::{Decision, decide, short_version};

    #[test]
    fn a_first_installation_only_remembers_the_version() {
        assert_eq!(
            decide("", "2.4.0", false, true, true),
            Decision::RememberOnly
        );
    }

    #[test]
    fn an_existing_user_without_a_remembered_version_sees_the_news_once() {
        assert_eq!(decide("", "2.4.0", true, true, true), Decision::Show);
    }

    #[test]
    fn a_new_version_shows_the_news() {
        assert_eq!(decide("2.3.0", "2.4.0", true, true, true), Decision::Show);
    }

    #[test]
    fn the_same_version_shows_nothing_again() {
        assert_eq!(
            decide("2.4.0", "2.4.0", true, true, true),
            Decision::Nothing
        );
    }

    #[test]
    fn a_disabled_preference_remembers_the_version_without_showing() {
        assert_eq!(
            decide("2.3.0", "2.4.0", true, false, true),
            Decision::RememberOnly
        );
    }

    #[test]
    fn a_version_without_news_is_remembered_without_showing() {
        assert_eq!(
            decide("2.3.0", "2.3.1", true, true, false),
            Decision::RememberOnly
        );
    }

    #[test]
    fn only_the_major_and_minor_numbers_are_shown() {
        assert_eq!(short_version("2.4.0"), "2.4");
        assert_eq!(short_version("2.4"), "2.4");
        assert_eq!(short_version("3"), "3");
    }
}
