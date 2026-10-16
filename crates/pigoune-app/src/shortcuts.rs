use std::collections::HashMap;

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, gio};

use crate::settings;

pub type Overrides = HashMap<String, String>;

pub struct Customizable {
    pub action: &'static str,
    pub defaults: &'static [&'static str],
    pub locked: bool,
}

pub const CUSTOMIZABLE: [Customizable; 30] = [
    Customizable {
        action: "win.create-library",
        defaults: &["<Control>n"],
        locked: false,
    },
    Customizable {
        action: "win.open-library",
        defaults: &["<Control>o"],
        locked: false,
    },
    Customizable {
        action: "win.import-files",
        defaults: &["<Control>i"],
        locked: false,
    },
    Customizable {
        action: "win.search",
        defaults: &["<Control>f"],
        locked: false,
    },
    Customizable {
        action: "win.select-all",
        defaults: &["<Control>a"],
        locked: true,
    },
    Customizable {
        action: "win.copy-selected",
        defaults: &["<Control>c"],
        locked: true,
    },
    Customizable {
        action: "win.paste",
        defaults: &["<Control>v"],
        locked: true,
    },
    Customizable {
        action: "win.paste-and-annotate",
        defaults: &["<Control><Shift>v"],
        locked: false,
    },
    Customizable {
        action: "win.toggle-favorite",
        defaults: &["<Control>d"],
        locked: false,
    },
    Customizable {
        action: "win.add-tag",
        defaults: &["<Control>t"],
        locked: false,
    },
    Customizable {
        action: "win.add-to-collection",
        defaults: &["<Control>k"],
        locked: false,
    },
    Customizable {
        action: "win.move-to-collection",
        defaults: &["<Control>m"],
        locked: false,
    },
    Customizable {
        action: "win.undo",
        defaults: &["<Control>z"],
        locked: true,
    },
    Customizable {
        action: "win.redo",
        defaults: &["<Control><Shift>z", "<Control>y"],
        locked: true,
    },
    Customizable {
        action: "win.enlarge-thumbnails",
        defaults: &["<Control>plus", "<Control>equal", "<Control>KP_Add"],
        locked: false,
    },
    Customizable {
        action: "win.shrink-thumbnails",
        defaults: &["<Control>minus", "<Control>KP_Subtract"],
        locked: false,
    },
    Customizable {
        action: "win.preferences",
        defaults: &["<Control>comma"],
        locked: false,
    },
    Customizable {
        action: "app.help",
        defaults: &["F1"],
        locked: false,
    },
    Customizable {
        action: "app.quit",
        defaults: &["<Control>q"],
        locked: true,
    },
    Customizable {
        action: "win.import-folder",
        defaults: &[],
        locked: false,
    },
    Customizable {
        action: "win.export-selected",
        defaults: &[],
        locked: false,
    },
    Customizable {
        action: "win.export-selected-as",
        defaults: &[],
        locked: false,
    },
    Customizable {
        action: "win.open-with",
        defaults: &[],
        locked: false,
    },
    Customizable {
        action: "win.show-in-files",
        defaults: &[],
        locked: false,
    },
    Customizable {
        action: "win.set-wallpaper",
        defaults: &[],
        locked: false,
    },
    Customizable {
        action: "win.restore-selected",
        defaults: &[],
        locked: false,
    },
    Customizable {
        action: "win.remove-from-all-collections",
        defaults: &[],
        locked: false,
    },
    Customizable {
        action: "win.empty-trash",
        defaults: &[],
        locked: false,
    },
    Customizable {
        action: "win.new-collection",
        defaults: &[],
        locked: false,
    },
    Customizable {
        action: "win.new-smart-collection",
        defaults: &[],
        locked: false,
    },
];

const CLOSE_WINDOW_ACTION: &str = "win.close-window";
const CLOSE_WINDOW_ACCELERATOR: &str = "<Control>w";
const RESERVED: [&str; 24] = [
    "F2",
    "F10",
    "F11",
    "<Shift>F10",
    "<Control>question",
    "<Control>w",
    "<Control>x",
    "<Control>BackSpace",
    "<Control>Delete",
    "<Control>Left",
    "<Control>Right",
    "<Control>Up",
    "<Control>Down",
    "<Control>Home",
    "<Control>End",
    "<Control>Tab",
    "<Control>Page_Up",
    "<Control>Page_Down",
    "<Alt>F4",
    "<Alt>Tab",
    "<Control><Shift>Tab",
    "<Control><Shift>ISO_Left_Tab",
    "<Control>0",
    "<Control>1",
];
const FUNCTION_KEYS: std::ops::RangeInclusive<u8> = 1..=12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Unreadable,
    NeedsModifier,
    Reserved,
    Taken(&'static str),
}

pub fn stored(settings: &gio::Settings) -> Overrides {
    settings
        .value(settings::SHORTCUTS)
        .get::<Overrides>()
        .unwrap_or_default()
}

pub fn store(settings: &gio::Settings, overrides: &Overrides) {
    settings::store_value(settings, settings::SHORTCUTS, &overrides.to_variant());
}

pub fn usable_accelerators(customizable: &Customizable, overrides: &Overrides) -> Vec<String> {
    let chosen = accelerators(customizable, overrides);
    if chosen
        .iter()
        .all(|accelerator| gtk::accelerator_parse(accelerator).is_some())
    {
        chosen
    } else {
        customizable
            .defaults
            .iter()
            .map(|default| (*default).to_owned())
            .collect()
    }
}

pub fn apply(application: &impl IsA<gtk::Application>, overrides: &Overrides) {
    for customizable in &CUSTOMIZABLE {
        let accelerators = usable_accelerators(customizable, overrides);
        let accelerators: Vec<&str> = accelerators.iter().map(String::as_str).collect();
        application.set_accels_for_action(customizable.action, &accelerators);
    }
    application.set_accels_for_action(CLOSE_WINDOW_ACTION, &[CLOSE_WINDOW_ACCELERATOR]);
}

pub fn suspend(application: &impl IsA<gtk::Application>) {
    for customizable in &CUSTOMIZABLE {
        application.set_accels_for_action(customizable.action, &[]);
    }
    application.set_accels_for_action(CLOSE_WINDOW_ACTION, &[]);
}

pub fn current(action: &str) -> Option<String> {
    let application = gio::Application::default()?
        .downcast::<gtk::Application>()
        .ok()?;
    application
        .accels_for_action(action)
        .first()
        .map(ToString::to_string)
}

pub fn menu_item(label: &str, action: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), Some(action));
    if let Some(accelerator) = current(action) {
        item.set_attribute_value("accel", Some(&accelerator.to_variant()));
    }
    item
}

pub fn current_label(action: &str) -> String {
    current(action).map_or_else(String::new, |accelerator| display(&accelerator))
}

pub fn label(action: &str) -> String {
    match action {
        "win.create-library" => gettext("New Library"),
        "win.open-library" => gettext("Open a Library"),
        "win.import-files" => gettext("Import Files"),
        "win.search" => gettext("Search"),
        "win.select-all" => gettext("Select All Assets"),
        "win.copy-selected" => gettext("Copy to the Clipboard"),
        "win.paste" => gettext("Paste Files or an Image to Import"),
        "win.paste-and-annotate" => gettext("Paste an Image to Annotate"),
        "win.toggle-favorite" => gettext("Add to or Remove from Favorites"),
        "win.add-tag" => gettext("Add a Tag"),
        "win.add-to-collection" => gettext("Add to a Collection"),
        "win.move-to-collection" => gettext("Move to a Collection"),
        "win.undo" => gettext("Undo"),
        "win.redo" => gettext("Redo"),
        "win.enlarge-thumbnails" => gettext("Enlarge Thumbnails"),
        "win.shrink-thumbnails" => gettext("Shrink Thumbnails"),
        "win.preferences" => gettext("Preferences"),
        "app.help" => gettext("Help"),
        "app.quit" => gettext("Quit"),
        "win.import-folder" => gettext("Import a Folder"),
        "win.export-selected" => gettext("Export Assets"),
        "win.export-selected-as" => gettext("Convert and Export Assets"),
        "win.open-with" => gettext("Open With Another Application"),
        "win.show-in-files" => gettext("Show in Files"),
        "win.set-wallpaper" => gettext("Set as Wallpaper"),
        "win.restore-selected" => gettext("Restore Assets"),
        "win.remove-from-all-collections" => gettext("Remove from All Collections"),
        "win.empty-trash" => gettext("Empty the Trash"),
        "win.new-collection" => gettext("New Collection"),
        "win.new-smart-collection" => gettext("New Smart Collection"),
        other => other.to_owned(),
    }
}

pub fn accelerators(customizable: &Customizable, overrides: &Overrides) -> Vec<String> {
    let chosen = if customizable.locked {
        None
    } else {
        overrides
            .get(customizable.action)
            .and_then(|chosen| canonical(chosen))
    };
    match chosen {
        Some(chosen) => vec![chosen],
        None => customizable
            .defaults
            .iter()
            .map(|default| (*default).to_owned())
            .collect(),
    }
}

const MODIFIERS: [(&str, &[&str]); 6] = [
    ("Control", &["control", "ctrl", "primary"]),
    ("Shift", &["shift"]),
    ("Alt", &["alt"]),
    ("Super", &["super"]),
    ("Hyper", &["hyper"]),
    ("Meta", &["meta"]),
];

pub fn canonical(accelerator: &str) -> Option<String> {
    let mut rest = accelerator.trim();
    let mut held = [false; MODIFIERS.len()];
    while let Some(after) = rest.strip_prefix('<') {
        let (name, remaining) = after.split_once('>')?;
        let name = name.to_lowercase();
        let index = MODIFIERS
            .iter()
            .position(|(_, aliases)| aliases.contains(&name.as_str()))?;
        held[index] = true;
        rest = remaining;
    }
    if rest.is_empty() || rest.contains(['<', '>', ' ']) {
        return None;
    }
    let key = if rest.chars().count() == 1 {
        rest.to_lowercase()
    } else {
        rest.to_owned()
    };
    let mut written = String::new();
    for ((name, _), held) in MODIFIERS.iter().zip(held) {
        if held {
            written.push('<');
            written.push_str(name);
            written.push('>');
        }
    }
    written.push_str(&key);
    Some(written)
}

pub fn from_key_press(key: gdk::Key, state: gdk::ModifierType) -> Option<String> {
    if is_modifier_key(key) {
        return None;
    }
    let mut modifiers = state & gtk::accelerator_get_default_mod_mask();
    if key
        .to_unicode()
        .is_some_and(|letter| !letter.is_alphabetic())
    {
        modifiers.remove(gdk::ModifierType::SHIFT_MASK);
    }
    canonical(&gtk::accelerator_name(key.to_lower(), modifiers))
}

pub fn is_modifier_key(key: gdk::Key) -> bool {
    matches!(
        key,
        gdk::Key::Shift_L
            | gdk::Key::Shift_R
            | gdk::Key::Control_L
            | gdk::Key::Control_R
            | gdk::Key::Alt_L
            | gdk::Key::Alt_R
            | gdk::Key::Meta_L
            | gdk::Key::Meta_R
            | gdk::Key::Super_L
            | gdk::Key::Super_R
            | gdk::Key::Hyper_L
            | gdk::Key::Hyper_R
            | gdk::Key::ISO_Level3_Shift
            | gdk::Key::Caps_Lock
            | gdk::Key::Num_Lock
    )
}

pub fn validate(action: &str, accelerator: &str, overrides: &Overrides) -> Result<String, Refusal> {
    let chosen = canonical(accelerator).ok_or(Refusal::Unreadable)?;
    let has_command_modifier = chosen.contains("<Control>") || chosen.contains("<Alt>");
    if !has_command_modifier && !is_function_key(key_of(&chosen)) {
        return Err(Refusal::NeedsModifier);
    }
    if RESERVED.contains(&chosen.as_str()) {
        return Err(Refusal::Reserved);
    }
    for other in &CUSTOMIZABLE {
        if other.action == action {
            continue;
        }
        let taken = accelerators(other, overrides)
            .iter()
            .any(|held| canonical(held).as_deref() == Some(chosen.as_str()));
        if taken {
            return Err(Refusal::Taken(other.action));
        }
    }
    Ok(chosen)
}

fn key_of(canonical: &str) -> &str {
    canonical.rsplit('>').next().unwrap_or(canonical)
}

fn is_function_key(key: &str) -> bool {
    key.strip_prefix('F')
        .and_then(|digits| digits.parse::<u8>().ok())
        .is_some_and(|number| FUNCTION_KEYS.contains(&number))
}

pub fn display(accelerator: &str) -> String {
    gtk::accelerator_parse(accelerator).map_or_else(
        || accelerator.to_owned(),
        |(key, modifiers)| gtk::accelerator_get_label(key, modifiers).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Overrides {
        Overrides::new()
    }

    fn chosen(action: &str, accelerator: &str) -> Overrides {
        Overrides::from([(action.to_owned(), accelerator.to_owned())])
    }

    const SHORTCUTS_WINDOW: &str = include_str!("../ui/shortcuts-dialog.blp");

    #[test]
    fn every_customizable_action_is_listed_in_the_shortcuts_window_with_its_label() {
        for customizable in CUSTOMIZABLE
            .iter()
            .filter(|customizable| !customizable.defaults.is_empty())
        {
            let action = format!("action-name: \"{}\";", customizable.action);
            let title = format!("title: _(\"{}\");", label(customizable.action));
            let listed = SHORTCUTS_WINDOW
                .split("Adw.ShortcutsItem")
                .any(|item| item.contains(&action) && item.contains(&title));
            assert!(
                listed,
                "{} is missing from the shortcuts window or has another title",
                customizable.action
            );
        }
    }

    #[test]
    fn the_shortcuts_window_does_not_show_a_customizable_action_as_a_fixed_key() {
        for item in SHORTCUTS_WINDOW.split("Adw.ShortcutsItem").skip(1) {
            if item.contains("action-name:") {
                assert!(
                    !item.contains("accelerator:"),
                    "an item mixes an action and a fixed key: {item}"
                );
            }
        }
    }

    #[test]
    fn every_default_is_readable_and_free_of_conflicts() {
        let mut seen: Vec<String> = Vec::new();
        for customizable in &CUSTOMIZABLE {
            for default in customizable.defaults {
                let default = canonical(default).expect("default is readable");
                assert!(!seen.contains(&default), "{default} is used twice");
                seen.push(default);
            }
        }
    }

    #[test]
    fn a_free_combination_with_control_is_accepted() {
        assert_eq!(
            validate("win.search", "<Control>j", &none()),
            Ok("<Control>j".to_owned())
        );
    }

    #[test]
    fn a_function_key_alone_is_accepted() {
        assert_eq!(validate("win.search", "F5", &none()), Ok("F5".to_owned()));
    }

    #[test]
    fn a_bare_letter_or_shift_letter_is_refused() {
        assert_eq!(
            validate("win.search", "j", &none()),
            Err(Refusal::NeedsModifier)
        );
        assert_eq!(
            validate("win.search", "<Shift>j", &none()),
            Err(Refusal::NeedsModifier)
        );
        assert_eq!(
            validate("win.search", "Delete", &none()),
            Err(Refusal::NeedsModifier)
        );
    }

    #[test]
    fn the_reserved_function_keys_are_refused() {
        assert_eq!(
            validate("win.search", "F2", &none()),
            Err(Refusal::Reserved)
        );
        assert_eq!(
            validate("win.search", "F11", &none()),
            Err(Refusal::Reserved)
        );
        assert_eq!(
            validate("win.search", "<Shift>F10", &none()),
            Err(Refusal::Reserved)
        );
    }

    #[test]
    fn the_keys_of_text_editing_and_of_the_window_are_refused() {
        for reserved in [
            "<Control>x",
            "<Control>BackSpace",
            "<Control>Left",
            "<Control>w",
            "<Alt>F4",
            "<Control>Page_Down",
            "<Control><Shift>ISO_Left_Tab",
            "<Control>0",
            "<Control>1",
        ] {
            assert_eq!(
                validate("win.search", reserved, &none()),
                Err(Refusal::Reserved),
                "{reserved} should be reserved"
            );
        }
    }

    #[test]
    fn a_combination_held_by_another_action_is_refused_with_its_name() {
        assert_eq!(
            validate("win.search", "<Control>k", &none()),
            Err(Refusal::Taken("win.add-to-collection"))
        );
    }

    #[test]
    fn a_secondary_default_counts_as_taken() {
        assert_eq!(
            validate("win.search", "<Control>y", &none()),
            Err(Refusal::Taken("win.redo"))
        );
    }

    #[test]
    fn keeping_its_own_combination_is_accepted() {
        assert_eq!(
            validate("win.add-to-collection", "<Control>k", &none()),
            Ok("<Control>k".to_owned())
        );
    }

    #[test]
    fn a_released_combination_becomes_free() {
        let overrides = chosen("win.add-to-collection", "<Control>j");
        assert_eq!(
            validate("win.search", "<Control>k", &overrides),
            Ok("<Control>k".to_owned())
        );
        assert_eq!(
            validate("win.search", "<Control>j", &overrides),
            Err(Refusal::Taken("win.add-to-collection"))
        );
    }

    #[test]
    fn an_unreadable_text_is_refused() {
        assert_eq!(
            validate("win.search", "not a shortcut", &none()),
            Err(Refusal::Unreadable)
        );
    }

    #[test]
    fn a_choice_replaces_every_default_of_the_action() {
        let enlarge = CUSTOMIZABLE
            .iter()
            .find(|customizable| customizable.action == "win.enlarge-thumbnails")
            .expect("enlarge is customizable");
        assert_eq!(accelerators(enlarge, &none()).len(), 3);
        assert_eq!(
            accelerators(enlarge, &chosen("win.enlarge-thumbnails", "<Control>j")),
            vec!["<Control>j".to_owned()]
        );
    }

    #[test]
    fn the_standard_shortcuts_are_locked_and_ignore_any_stored_choice() {
        let locked: Vec<&str> = CUSTOMIZABLE
            .iter()
            .filter(|customizable| customizable.locked)
            .map(|customizable| customizable.action)
            .collect();
        assert_eq!(
            locked,
            [
                "win.select-all",
                "win.copy-selected",
                "win.paste",
                "win.undo",
                "win.redo",
                "app.quit"
            ]
        );
        let copy = CUSTOMIZABLE
            .iter()
            .find(|customizable| customizable.action == "win.copy-selected")
            .expect("copy is listed");
        assert_eq!(
            accelerators(copy, &chosen("win.copy-selected", "<Control>j")),
            vec!["<Control>c".to_owned()]
        );
    }

    #[test]
    fn an_unreadable_stored_choice_falls_back_to_the_defaults() {
        let search = CUSTOMIZABLE
            .iter()
            .find(|customizable| customizable.action == "win.search")
            .expect("search is customizable");
        assert_eq!(
            accelerators(search, &chosen("win.search", "garbage text")),
            vec!["<Control>f".to_owned()]
        );
    }

    #[test]
    fn the_modifier_keys_alone_are_not_shortcuts() {
        assert!(is_modifier_key(gdk::Key::Control_L));
        assert!(is_modifier_key(gdk::Key::Shift_R));
        assert!(!is_modifier_key(gdk::Key::k));
    }

    #[test]
    fn spellings_of_the_same_combination_are_written_the_same_way() {
        let expected = Some("<Control><Shift>z".to_owned());
        assert_eq!(canonical("<Shift><Control>Z"), expected);
        assert_eq!(canonical("<Primary><Shift>z"), expected);
        assert_eq!(canonical("<ctrl><shift>z"), expected);
        assert_eq!(canonical("F5"), Some("F5".to_owned()));
        assert_eq!(
            canonical("<Control>KP_Add"),
            Some("<Control>KP_Add".to_owned())
        );
    }

    #[test]
    fn text_that_is_not_a_combination_is_not_read() {
        assert_eq!(canonical(""), None);
        assert_eq!(canonical("<Control>"), None);
        assert_eq!(canonical("<Nothing>k"), None);
        assert_eq!(canonical("two words"), None);
    }
}
