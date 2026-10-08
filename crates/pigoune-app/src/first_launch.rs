use gtk::gdk;
use gtk::gio;
use gtk::prelude::*;

use crate::settings;

const TOP_BAR_ROOM: i32 = 64;

#[derive(Clone, Copy)]
enum Choice {
    On(bool),
    Width(i32),
    Height(i32),
}

const PROFILE: [(&str, Choice); 5] = [
    (settings::WINDOW_WIDTH, Choice::Width(1400)),
    (settings::WINDOW_HEIGHT, Choice::Height(800)),
    (settings::PREVIEW_BOUNDS, Choice::On(false)),
    (settings::PREVIEW_DETAILS, Choice::On(true)),
    (settings::SEARCH_EVERYWHERE, Choice::On(true)),
];

pub fn apply(settings: &gio::Settings) {
    apply_on_screen(settings, first_monitor_size());
}

fn apply_on_screen(settings: &gio::Settings, screen: Option<(i32, i32)>) {
    if !is_untouched(settings) {
        return;
    }
    for (key, choice) in PROFILE {
        match choice {
            Choice::On(value) => settings::store_bool(settings, key, value),
            Choice::Width(wanted) => {
                settings::store_int(settings, key, fitted(wanted, screen.map(|size| size.0), 0));
            }
            Choice::Height(wanted) => {
                settings::store_int(
                    settings,
                    key,
                    fitted(wanted, screen.map(|size| size.1), TOP_BAR_ROOM),
                );
            }
        }
    }
}

fn is_untouched(settings: &gio::Settings) -> bool {
    settings.settings_schema().is_some_and(|schema| {
        schema
            .list_keys()
            .iter()
            .all(|key| settings.user_value(key).is_none())
    })
}

fn fitted(wanted: i32, available: Option<i32>, reserved: i32) -> i32 {
    available.map_or(wanted, |room| wanted.min(room - reserved))
}

fn first_monitor_size() -> Option<(i32, i32)> {
    let monitor = gdk::Display::default()?
        .monitors()
        .item(0)?
        .downcast::<gdk::Monitor>()
        .ok()?;
    let geometry = monitor.geometry();
    Some((geometry.width(), geometry.height()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA: &str = include_str!("../../../data/io.github.gor3pig.Pigoune.gschema.xml");

    fn stored(settings: &gio::Settings, key: &str) -> Option<String> {
        settings
            .user_value(key)
            .map(|value| value.print(false).to_string())
    }

    #[test]
    fn a_new_installation_gets_the_first_launch_profile() {
        let settings = settings::load_in_memory();

        apply_on_screen(&settings, Some((1920, 1080)));

        assert_eq!(
            stored(&settings, settings::PREVIEW_DETAILS).as_deref(),
            Some("true")
        );
        assert_eq!(
            stored(&settings, settings::PREVIEW_BOUNDS).as_deref(),
            Some("false")
        );
        assert_eq!(
            stored(&settings, settings::SEARCH_EVERYWHERE).as_deref(),
            Some("true")
        );
        assert_eq!(
            stored(&settings, settings::WINDOW_WIDTH).as_deref(),
            Some("1400")
        );
        assert_eq!(
            stored(&settings, settings::WINDOW_HEIGHT).as_deref(),
            Some("800")
        );
    }

    #[test]
    fn the_window_never_exceeds_a_small_screen() {
        let settings = settings::load_in_memory();

        apply_on_screen(&settings, Some((1280, 720)));

        assert_eq!(
            stored(&settings, settings::WINDOW_WIDTH).as_deref(),
            Some("1280")
        );
        assert_eq!(
            stored(&settings, settings::WINDOW_HEIGHT).as_deref(),
            Some("656")
        );
    }

    #[test]
    fn someone_who_already_customized_anything_sees_no_change() {
        let settings = settings::load_in_memory();
        settings::store_int(&settings, settings::WINDOW_WIDTH, 1000);

        apply_on_screen(&settings, Some((1920, 1080)));

        assert_eq!(
            stored(&settings, settings::WINDOW_WIDTH).as_deref(),
            Some("1000")
        );
        assert_eq!(stored(&settings, settings::WINDOW_HEIGHT), None);
        assert_eq!(stored(&settings, settings::PREVIEW_DETAILS), None);
        assert_eq!(stored(&settings, settings::SEARCH_EVERYWHERE), None);
    }

    #[test]
    fn the_profile_never_overrides_a_later_choice() {
        let settings = settings::load_in_memory();
        apply_on_screen(&settings, Some((1920, 1080)));
        settings::store_bool(&settings, settings::PREVIEW_DETAILS, false);

        apply_on_screen(&settings, Some((1920, 1080)));

        assert_eq!(
            stored(&settings, settings::PREVIEW_DETAILS).as_deref(),
            Some("false")
        );
    }

    #[test]
    fn the_profile_only_names_real_settings_that_it_changes() {
        let settings = settings::load_in_memory();
        let schema = settings.settings_schema().expect("the schema is known");
        for (key, _) in PROFILE {
            assert!(schema.has_key(key), "{key} is not in the schema");
            assert!(
                SCHEMA.contains(&format!("name=\"{key}\"")),
                "{key} is not declared"
            );
        }
        apply_on_screen(&settings, None);
        for (key, _) in PROFILE {
            let default = settings.default_value(key).expect("the key has a default");
            let chosen = settings.value(key);
            assert_ne!(default, chosen, "{key} keeps its default value");
        }
    }

    #[test]
    fn the_internal_state_is_never_part_of_the_profile() {
        let internal = [
            settings::LAST_SEEN_VERSION,
            settings::LAST_LIBRARY_PATH,
            settings::RECENT_LIBRARIES,
            settings::LAST_VIEW,
            settings::LANGUAGE,
        ];
        for (key, _) in PROFILE {
            assert!(!internal.contains(&key), "{key} is internal state");
        }
    }
}
