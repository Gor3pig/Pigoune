use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use gtk::gio::prelude::*;
use gtk::glib;

use crate::config::{GETTEXT_PACKAGE, LOCALEDIR};
use crate::settings;

const SOURCE_LANGUAGE: &str = "en";
const LANGUAGE_VARIABLE: &str = "LANGUAGE";
const ORIGINAL_VARIABLE: &str = "PIGOUNE_SYSTEM_LANGUAGE";

const NATIVE_NAMES: [(&str, &str); 24] = [
    ("ca", "Català"),
    ("cs", "Čeština"),
    ("da", "Dansk"),
    ("de", "Deutsch"),
    ("el", "Ελληνικά"),
    ("en", "English"),
    ("es", "Español"),
    ("eu", "Euskara"),
    ("fi", "Suomi"),
    ("fr", "Français"),
    ("hu", "Magyar"),
    ("it", "Italiano"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("nb", "Norsk bokmål"),
    ("nl", "Nederlands"),
    ("oc", "Occitan"),
    ("pl", "Polski"),
    ("pt", "Português"),
    ("pt_BR", "Português do Brasil"),
    ("ru", "Русский"),
    ("sv", "Svenska"),
    ("tr", "Türkçe"),
    ("uk", "Українська"),
];

pub fn apply_chosen_language() {
    let chosen = settings::load().string(settings::LANGUAGE);
    let original = env::var_os(ORIGINAL_VARIABLE);
    let mut command = match (chosen.is_empty(), original) {
        (true, None) => return,
        (true, Some(original)) => {
            let mut command = this_program_again();
            command.env_remove(ORIGINAL_VARIABLE);
            if original.is_empty() {
                command.env_remove(LANGUAGE_VARIABLE);
            } else {
                command.env(LANGUAGE_VARIABLE, original);
            }
            command
        }
        (false, original) => {
            if original.is_some()
                && env::var(LANGUAGE_VARIABLE).is_ok_and(|current| current == chosen)
            {
                return;
            }
            let mut command = this_program_again();
            let system_language = original
                .or_else(|| env::var_os(LANGUAGE_VARIABLE))
                .unwrap_or_default();
            command
                .env(ORIGINAL_VARIABLE, system_language)
                .env(LANGUAGE_VARIABLE, chosen.as_str());
            command
        }
    };
    let error = command.exec();
    glib::g_warning!("pigoune", "Unable to choose the language: {error}");
}

fn this_program_again() -> Command {
    let program = env::current_exe().unwrap_or_else(|_| PathBuf::from(GETTEXT_PACKAGE));
    let mut command = Command::new(program);
    command.args(env::args_os().skip(1));
    command
}

pub fn available() -> Vec<String> {
    let translated = fs::read_dir(LOCALEDIR)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| has_translation(&entry.path()))
        .filter_map(|entry| entry.file_name().into_string().ok());
    with_source_language(translated)
}

pub fn native_name(code: &str) -> String {
    NATIVE_NAMES
        .iter()
        .find(|(known, _)| *known == code)
        .map_or_else(|| code.to_owned(), |(_, name)| (*name).to_owned())
}

fn has_translation(language_dir: &Path) -> bool {
    language_dir
        .join("LC_MESSAGES")
        .join(format!("{GETTEXT_PACKAGE}.mo"))
        .is_file()
}

fn with_source_language(translated: impl Iterator<Item = String>) -> Vec<String> {
    let mut codes: Vec<String> = translated.collect();
    codes.push(SOURCE_LANGUAGE.to_owned());
    codes.sort_by_key(|code| native_name(code).to_lowercase());
    codes.dedup();
    codes
}

#[cfg(test)]
mod tests {
    use super::{native_name, with_source_language};

    #[test]
    fn known_languages_are_named_in_their_own_language() {
        assert_eq!(native_name("fr"), "Français");
        assert_eq!(native_name("de"), "Deutsch");
    }

    #[test]
    fn an_unknown_language_shows_its_code() {
        assert_eq!(native_name("xx_YY"), "xx_YY");
    }

    #[test]
    fn english_is_always_offered_and_the_list_is_sorted_by_name() {
        let codes = with_source_language(["fr".to_owned(), "de".to_owned()].into_iter());
        assert_eq!(codes, ["de", "en", "fr"]);
    }

    #[test]
    fn english_is_offered_once() {
        let codes = with_source_language(["en".to_owned()].into_iter());
        assert_eq!(codes, ["en"]);
    }
}
