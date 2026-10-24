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

fn points_of_version_3() -> Vec<Point> {
    vec![
        Point {
            icon: "document-edit-symbolic",
            title: gettext("Annotate Without Touching the Original"),
            text: gettext(
                "Draw on screenshots and on any image of your library: the original file is never changed, and your annotations stay editable.",
            ),
        },
        Point {
            icon: "view-list-symbolic",
            title: gettext("Layers You Control"),
            text: gettext(
                "Reorder, hide, lock, rename and duplicate annotations from the Layers panel, and hide a blur or pixelation without losing it.",
            ),
        },
        Point {
            icon: "color-select-symbolic",
            title: gettext("More Ways to Draw"),
            text: gettext(
                "Dashed or dotted strokes, a precise, sketchy or very sketchy look, curved arrows, numbered steps, a spotlight, a blur and a pixelation.",
            ),
        },
        Point {
            icon: "sidebar-show-right-symbolic",
            title: gettext("Your Workspace, Your Way"),
            text: gettext(
                "Fold Tools, Properties and Layers, or hide the whole panel with F9. Pigoune remembers your choice.",
            ),
        },
        Point {
            icon: "view-reveal-symbolic",
            title: gettext("Compare With the Original"),
            text: gettext(
                "Hold the eye button in the preview to see the original image, and release it to see your annotations again.",
            ),
        },
    ]
}

fn points_of_version_3_1() -> Vec<Point> {
    vec![
        Point {
            icon: "object-rotate-right-symbolic",
            title: gettext("Turn Your Annotations"),
            text: gettext(
                "Turn rectangles, ellipses, texts, lines, arrows, strokes, blurs and pixelations with the round handle above them, or set an exact angle in the panel.",
            ),
        },
        Point {
            icon: "font-select-symbolic",
            title: gettext("Resize a Text With Its Handles"),
            text: gettext(
                "Drag a handle of a selected text to make the letters bigger or smaller, like any other annotation.",
            ),
        },
        Point {
            icon: "view-conceal-symbolic",
            title: gettext("Leave a Collection Out of Duplicates"),
            text: gettext(
                "Right-click a collection and choose Leave Out of Duplicate Search, for archives you do not want compared. Undo brings it back.",
            ),
        },
        Point {
            icon: "color-select-symbolic",
            title: gettext("A Few More Choices"),
            text: gettext(
                "A rounded spotlight, bigger steps, a blur that goes up to a radius of 150, and layers renamed with a double click.",
            ),
        },
        Point {
            icon: "security-high-symbolic",
            title: gettext("Safer and Quicker"),
            text: gettext(
                "A blur now hides everything up to the edge of the image, and files shared with other applications are cleaned up after a while. Big libraries open faster, the grid and the sidebar refresh only where something changed, and stored thumbnails appear sooner.",
            ),
        },
    ]
}

fn points_of_version_3_2() -> Vec<Point> {
    vec![
        Point {
            icon: "weather-clear-night-symbolic",
            title: gettext("Choose Your Theme"),
            text: gettext(
                "Three round buttons at the top of the main menu follow the system, or always use the light or the dark theme.",
            ),
        },
        Point {
            icon: "view-list-symbolic",
            title: gettext("See What Is Left Out of Duplicates"),
            text: gettext(
                "The Left Out button of the Duplicates tab lists the collections left out of the search, and Include brings one back. The sidebar no longer shows a mark.",
            ),
        },
        Point {
            icon: "image-x-generic-symbolic",
            title: gettext("Thumbnails Come Back by Themselves"),
            text: gettext(
                "After Clear in Manage Library, Pigoune makes the thumbnails again in the background, while you are not using it.",
            ),
        },
    ]
}

fn points_of(version: &str) -> Vec<Point> {
    match version {
        "3.2.0" => points_of_version_3_2(),
        "3.1.0" => points_of_version_3_1(),
        "3.0.0" => points_of_version_3(),
        "2.12.0" => vec![
            Point {
                icon: "view-conceal-symbolic",
                title: gettext("Ignore What Is Not a Duplicate"),
                text: gettext(
                    "Ignore a whole group, or a single asset, when look-alikes are not duplicates. The Ignored button lists them and brings them back, and Undo works too.",
                ),
            },
            Point {
                icon: "view-list-symbolic",
                title: gettext("A Lighter Duplicates Tab"),
                text: gettext(
                    "The trash button only appears once you tick something, and the detailed analysis is kept in its own file so the library stays small.",
                ),
            },
        ],
        "2.11.0" => vec![
            Point {
                icon: "user-trash-symbolic",
                title: gettext("Safer Duplicates"),
                text: gettext(
                    "Nothing goes to the trash unless you tick it yourself, and Pigoune checks every asset again just before moving it.",
                ),
            },
            Point {
                icon: "color-select-symbolic",
                title: gettext("Finer Comparison"),
                text: gettext(
                    "Pigoune now compares colors and details, so assets that only share an outline are no longer grouped. It analyzes your library again in the background.",
                ),
            },
        ],
        "2.10.0" => vec![
            Point {
                icon: "edit-find-symbolic",
                title: gettext("Find Look-Alikes"),
                text: gettext(
                    "Right-click an asset and choose Find Similar to see the assets that look like it, closest first.",
                ),
            },
            Point {
                icon: "edit-copy-symbolic",
                title: gettext("Duplicates"),
                text: gettext(
                    "The new Duplicates tab of Manage Library groups assets that look alike, such as the same logo in two formats. Keep one, and the others go to the trash.",
                ),
            },
        ],
        "2.9.0" => vec![
            Point {
                icon: "input-mouse-symbolic",
                title: gettext("Precise Clicks"),
                text: gettext(
                    "Clicking next to a thumbnail no longer selects it: the clickable area is exactly the thumbnail and its name.",
                ),
            },
            Point {
                icon: "font-select-symbolic",
                title: gettext("Names as Plain Text"),
                text: gettext(
                    "A new option in Preferences, under Display, makes asset names plain text, so a selection rectangle can start from them.",
                ),
            },
        ],
        "2.8.0" => vec![
            Point {
                icon: "edit-undo-symbolic",
                title: gettext("Undo and Redo"),
                text: gettext(
                    "Two buttons at the top of the sidebar undo and redo your changes, and the message after an undo offers to redo it.",
                ),
            },
            Point {
                icon: "input-keyboard-symbolic",
                title: gettext("Your Own Shortcuts"),
                text: gettext(
                    "Change the keyboard shortcuts in Preferences, on the new Shortcuts page, and give a key to actions that had none.",
                ),
            },
            Point {
                icon: "folder-symbolic",
                title: gettext("Clearer Moves"),
                text: gettext(
                    "Dragging assets from All into a collection now moves them and Ctrl adds them, and every action tells you what changed with an Undo button.",
                ),
            },
            Point {
                icon: "drive-harddisk-symbolic",
                title: gettext("Manage Library, Instantly"),
                text: gettext(
                    "The Manage Library window opens at once, even for a library on a slow disk.",
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
    use super::{Decision, decide, points_of, short_version};

    #[test]
    fn the_news_of_the_current_major_version_are_between_three_and_five_complete_points() {
        let points = points_of("3.2.0");
        assert!((3..=5).contains(&points.len()), "{} points", points.len());
        for point in &points {
            assert!(!point.icon.is_empty() && !point.title.is_empty() && !point.text.is_empty());
        }
    }

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
