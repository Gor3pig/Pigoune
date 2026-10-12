use std::collections::HashMap;

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, gio};

use crate::settings;

pub type Overrides = HashMap<String, String>;

pub struct Customizable {
    pub action: &'static str,
    pub defaults: &'static [&'static str],
}

pub const CUSTOMIZABLE: [Customizable; 18] = [
    Customizable {
        action: "win.create-library",
        defaults: &["<Control>n"],
    },
    Customizable {
        action: "win.open-library",
        defaults: &["<Control>o"],
    },
    Customizable {
        action: "win.import-files",
        defaults: &["<Control>i"],
    },
    Customizable {
        action: "win.search",
        defaults: &["<Control>f"],
    },
    Customizable {
        action: "win.select-all",
        defaults: &["<Control>a"],
    },
    Customizable {
        action: "win.copy-selected",
        defaults: &["<Control>c"],
    },
    Customizable {
        action: "win.paste",
        defaults: &["<Control>v"],
    },
    Customizable {
        action: "win.toggle-favorite",
        defaults: &["<Control>d"],
    },
    Customizable {
        action: "win.add-tag",
        defaults: &["<Control>t"],
    },
    Customizable {
        action: "win.add-to-collection",
        defaults: &["<Control>k"],
    },
    Customizable {
        action: "win.move-to-collection",
        defaults: &["<Control>m"],
    },
    Customizable {
        action: "win.undo",
        defaults: &["<Control>z"],
    },
    Customizable {
        action: "win.redo",
        defaults: &["<Control><Shift>z", "<Control>y"],
    },
    Customizable {
        action: "win.enlarge-thumbnails",
        defaults: &["<Control>plus", "<Control>equal", "<Control>KP_Add"],
    },
    Customizable {
        action: "win.shrink-thumbnails",
        defaults: &["<Control>minus", "<Control>KP_Subtract"],
    },
    Customizable {
        action: "win.preferences",
        defaults: &["<Control>comma"],
    },
    Customizable {
        action: "app.help",
        defaults: &["F1"],
    },
    Customizable {
        action: "app.quit",
        defaults: &["<Control>q"],
    },
];

const RESERVED: [&str; 5] = ["F2", "F10", "F11", "<Shift>F10", "<Control>question"];
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
        other => other.to_owned(),
    }
}

pub fn accelerators(customizable: &Customizable, overrides: &Overrides) -> Vec<String> {
    match overrides
        .get(customizable.action)
        .and_then(|chosen| canonical(chosen))
    {
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
        let redo = CUSTOMIZABLE
            .iter()
            .find(|customizable| customizable.action == "win.redo")
            .expect("redo is customizable");
        assert_eq!(accelerators(redo, &none()).len(), 2);
        assert_eq!(
            accelerators(redo, &chosen("win.redo", "<Control>j")),
            vec!["<Control>j".to_owned()]
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
