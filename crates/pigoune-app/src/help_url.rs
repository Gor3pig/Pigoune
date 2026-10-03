const HELP_BASE: &str = "https://github.com/Gor3pig/Pigoune/blob/main/help";
const SOURCE_LANGUAGE: &str = "en";
const TRANSLATED_GUIDES: [&str; 1] = ["fr"];

pub fn help_url(languages: &[&str]) -> String {
    let language = languages
        .iter()
        .filter_map(|language| language.split(['_', '.', '@']).next())
        .find(|code| *code == SOURCE_LANGUAGE || *code == "C" || TRANSLATED_GUIDES.contains(code))
        .filter(|code| TRANSLATED_GUIDES.contains(code))
        .unwrap_or(SOURCE_LANGUAGE);
    format!("{HELP_BASE}/{language}/README.md")
}

#[cfg(test)]
mod tests {
    use super::help_url;

    #[test]
    fn a_french_desktop_opens_the_french_guide() {
        assert_eq!(
            help_url(&["fr_FR.UTF-8", "fr_FR", "fr", "C"]),
            "https://github.com/Gor3pig/Pigoune/blob/main/help/fr/README.md"
        );
    }

    #[test]
    fn english_comes_first_when_the_user_prefers_it() {
        assert_eq!(
            help_url(&["en_US", "en", "fr", "C"]),
            "https://github.com/Gor3pig/Pigoune/blob/main/help/en/README.md"
        );
    }

    #[test]
    fn a_language_without_a_guide_falls_back_to_english() {
        assert_eq!(
            help_url(&["de_DE.UTF-8", "de", "C"]),
            "https://github.com/Gor3pig/Pigoune/blob/main/help/en/README.md"
        );
    }

    #[test]
    fn a_language_without_a_guide_lets_a_later_translated_one_win() {
        assert_eq!(
            help_url(&["br_FR", "br", "fr_FR", "fr", "C"]),
            "https://github.com/Gor3pig/Pigoune/blob/main/help/fr/README.md"
        );
    }
}
