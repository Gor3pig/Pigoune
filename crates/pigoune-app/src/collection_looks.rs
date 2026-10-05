use gettextrs::pgettext;
use pigoune_core::CollectionLook;

pub const DEFAULT_ICON_KEY: &str = "folder";
pub const DEFAULT_ICON_NAME: &str = "folder-symbolic";

pub struct LookChoice {
    pub key: &'static str,
    pub label: String,
}

const ICON_KEYS: [&str; 40] = [
    "folder",
    "folder-pictures",
    "folder-music",
    "folder-videos",
    "folder-templates",
    "user-home",
    "user-bookmarks",
    "starred",
    "emote-love",
    "camera-photo",
    "audio-headphones",
    "font-x-generic",
    "x-office-document",
    "x-office-drawing",
    "x-office-presentation",
    "x-office-spreadsheet",
    "x-office-calendar",
    "mail-attachment",
    "package-x-generic",
    "applications-graphics",
    "applications-games",
    "applications-science",
    "applications-engineering",
    "dialog-password",
    "emoji-nature",
    "emoji-food",
    "emoji-travel",
    "airplane-mode",
    "emoji-objects",
    "emoji-symbols",
    "emoji-flags",
    "emoji-activities",
    "emoji-people",
    "face-smile",
    "weather-clear",
    "weather-clear-night",
    "computer",
    "phone",
    "input-tablet",
    "mark-location",
];

const ICON_NAMES: [&str; 40] = [
    "folder-symbolic",
    "folder-pictures-symbolic",
    "folder-music-symbolic",
    "folder-videos-symbolic",
    "folder-templates-symbolic",
    "user-home-symbolic",
    "user-bookmarks-symbolic",
    "starred-symbolic",
    "emote-love-symbolic",
    "camera-photo-symbolic",
    "audio-headphones-symbolic",
    "font-x-generic-symbolic",
    "x-office-document-symbolic",
    "x-office-drawing-symbolic",
    "x-office-presentation-symbolic",
    "x-office-spreadsheet-symbolic",
    "x-office-calendar-symbolic",
    "mail-attachment-symbolic",
    "package-x-generic-symbolic",
    "applications-graphics-symbolic",
    "applications-games-symbolic",
    "applications-science-symbolic",
    "applications-engineering-symbolic",
    "dialog-password-symbolic",
    "emoji-nature-symbolic",
    "emoji-food-symbolic",
    "emoji-travel-symbolic",
    "airplane-mode-symbolic",
    "emoji-objects-symbolic",
    "emoji-symbols-symbolic",
    "emoji-flags-symbolic",
    "emoji-activities-symbolic",
    "emoji-people-symbolic",
    "face-smile-symbolic",
    "weather-clear-symbolic",
    "weather-clear-night-symbolic",
    "computer-symbolic",
    "phone-symbolic",
    "input-tablet-symbolic",
    "mark-location-symbolic",
];

const COLOR_KEYS: [&str; 9] = [
    "blue", "teal", "green", "yellow", "orange", "red", "pink", "purple", "slate",
];

pub const COLOR_CLASSES: [&str; 9] = [
    "collection-blue",
    "collection-teal",
    "collection-green",
    "collection-yellow",
    "collection-orange",
    "collection-red",
    "collection-pink",
    "collection-purple",
    "collection-slate",
];

#[must_use]
pub fn icon_name(look: &CollectionLook) -> &'static str {
    icon_name_of_key(known_icon_key(look))
}

#[must_use]
pub fn icon_name_of_key(key: &str) -> &'static str {
    ICON_KEYS
        .iter()
        .position(|known| *known == key)
        .map_or(DEFAULT_ICON_NAME, |index| ICON_NAMES[index])
}

#[must_use]
pub fn known_icon_key(look: &CollectionLook) -> &'static str {
    look.icon
        .as_deref()
        .and_then(|key| ICON_KEYS.iter().find(|known| **known == key))
        .copied()
        .unwrap_or(DEFAULT_ICON_KEY)
}

#[must_use]
pub fn known_color_key(look: &CollectionLook) -> Option<&'static str> {
    let key = look.color.as_deref()?;
    COLOR_KEYS.iter().find(|known| **known == key).copied()
}

#[must_use]
pub fn color_class(look: &CollectionLook) -> Option<&'static str> {
    let key = known_color_key(look)?;
    COLOR_KEYS
        .iter()
        .position(|known| *known == key)
        .map(|index| COLOR_CLASSES[index])
}

#[must_use]
pub fn icon_choices() -> Vec<LookChoice> {
    ICON_KEYS
        .iter()
        .map(|key| LookChoice {
            key,
            label: icon_label(key),
        })
        .collect()
}

#[must_use]
pub fn color_choices() -> Vec<LookChoice> {
    COLOR_KEYS
        .iter()
        .map(|key| LookChoice {
            key,
            label: color_label(key),
        })
        .collect()
}

#[must_use]
pub fn default_color_label() -> String {
    pgettext("collection color", "Default")
}

fn icon_label(key: &str) -> String {
    match key {
        "folder-pictures" => pgettext("collection icon", "Pictures"),
        "folder-music" => pgettext("collection icon", "Music"),
        "folder-videos" => pgettext("collection icon", "Videos"),
        "folder-templates" => pgettext("collection icon", "Templates"),
        "user-home" => pgettext("collection icon", "Home"),
        "user-bookmarks" => pgettext("collection icon", "Bookmark"),
        "starred" => pgettext("collection icon", "Star"),
        "emote-love" => pgettext("collection icon", "Heart"),
        "camera-photo" => pgettext("collection icon", "Camera"),
        "font-x-generic" => pgettext("collection icon", "Font"),
        "x-office-document" => pgettext("collection icon", "Document"),
        "x-office-drawing" => pgettext("collection icon", "Drawing"),
        "x-office-presentation" => pgettext("collection icon", "Presentation"),
        "x-office-spreadsheet" => pgettext("collection icon", "Spreadsheet"),
        "package-x-generic" => pgettext("collection icon", "Package"),
        "applications-graphics" => pgettext("collection icon", "Graphics"),
        "applications-games" => pgettext("collection icon", "Games"),
        "applications-science" => pgettext("collection icon", "Science"),
        "applications-engineering" => pgettext("collection icon", "Tools"),
        "emoji-nature" => pgettext("collection icon", "Nature"),
        "emoji-food" => pgettext("collection icon", "Food"),
        "emoji-travel" => pgettext("collection icon", "Travel"),
        "emoji-objects" => pgettext("collection icon", "Light Bulb"),
        "emoji-symbols" => pgettext("collection icon", "Symbols"),
        "emoji-flags" => pgettext("collection icon", "Flag"),
        "emoji-activities" => pgettext("collection icon", "Sports"),
        "emoji-people" => pgettext("collection icon", "People"),
        "face-smile" => pgettext("collection icon", "Smile"),
        "weather-clear" => pgettext("collection icon", "Sun"),
        "weather-clear-night" => pgettext("collection icon", "Moon"),
        "computer" => pgettext("collection icon", "Computer"),
        "phone" => pgettext("collection icon", "Phone"),
        "input-tablet" => pgettext("collection icon", "Graphics Tablet"),
        "mark-location" => pgettext("collection icon", "Location"),
        "audio-headphones" => pgettext("collection icon", "Headphones"),
        "x-office-calendar" => pgettext("collection icon", "Calendar"),
        "mail-attachment" => pgettext("collection icon", "Paper Clip"),
        "dialog-password" => pgettext("collection icon", "Key"),
        "airplane-mode" => pgettext("collection icon", "Airplane"),
        _ => pgettext("collection icon", "Folder"),
    }
}

fn color_label(key: &str) -> String {
    match key {
        "teal" => pgettext("collection color", "Teal"),
        "green" => pgettext("collection color", "Green"),
        "yellow" => pgettext("collection color", "Yellow"),
        "orange" => pgettext("collection color", "Orange"),
        "red" => pgettext("collection color", "Red"),
        "pink" => pgettext("collection color", "Pink"),
        "purple" => pgettext("collection color", "Purple"),
        "slate" => pgettext("collection color", "Slate"),
        _ => pgettext("collection color", "Blue"),
    }
}

#[cfg(test)]
mod tests {
    use pigoune_core::CollectionLook;

    use super::{COLOR_KEYS, DEFAULT_ICON_NAME, ICON_KEYS, color_class, icon_name};

    #[test]
    fn every_icon_key_names_its_symbolic_icon() {
        for key in ICON_KEYS {
            let look = CollectionLook {
                icon: Some(key.to_owned()),
                color: None,
            };
            assert_eq!(icon_name(&look), format!("{key}-symbolic"));
            assert!(look.is_valid());
        }
    }

    #[test]
    fn every_color_key_has_its_own_class() {
        for key in COLOR_KEYS {
            let look = CollectionLook {
                icon: None,
                color: Some(key.to_owned()),
            };
            assert_eq!(
                color_class(&look),
                Some(format!("collection-{key}").as_str())
            );
            assert!(look.is_valid());
        }
    }

    #[test]
    fn an_unknown_look_falls_back_to_the_default_folder() {
        let look = CollectionLook {
            icon: Some("rocket".to_owned()),
            color: Some("gold".to_owned()),
        };

        assert_eq!(icon_name(&look), DEFAULT_ICON_NAME);
        assert_eq!(color_class(&look), None);
        assert_eq!(icon_name(&CollectionLook::default()), DEFAULT_ICON_NAME);
    }
}
