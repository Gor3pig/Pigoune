use std::collections::HashSet;

use pigoune_core::{LONGEST_TAG_NAME, Tag, TagId, comparable};

const MOST_SUGGESTIONS: usize = 8;

pub fn is_separator(character: char) -> bool {
    character == ',' || character.is_whitespace()
}

fn shortened(name: &str) -> String {
    name.chars().take(LONGEST_TAG_NAME).collect()
}

pub fn names_in(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.split(is_separator)
        .filter(|name| !name.is_empty())
        .map(shortened)
        .filter(|name| seen.insert(name.to_lowercase()))
        .collect()
}

pub fn split_finished(text: &str) -> (Vec<String>, String) {
    match text.rfind(is_separator) {
        Some(position) => {
            let separator_length = text[position..].chars().next().map_or(1, char::len_utf8);
            (
                names_in(&text[..position]),
                shortened(&text[position + separator_length..]),
            )
        }
        None => (Vec::new(), shortened(text)),
    }
}

pub fn fragment_being_typed(text: &str) -> &str {
    text.rsplit(is_separator).next().unwrap_or_default()
}

pub fn suggestions<'a>(all: &'a [Tag], typed: &str, already: &[TagId]) -> Vec<&'a Tag> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Vec::new();
    }
    let wanted = comparable(typed);
    let candidates = all.iter().filter(|tag| !already.contains(&tag.id));
    let (mut starting, mut containing): (Vec<&Tag>, Vec<&Tag>) = candidates
        .filter(|tag| comparable(&tag.name).contains(&wanted))
        .partition(|tag| comparable(&tag.name).starts_with(&wanted));
    starting.append(&mut containing);
    starting.retain(|tag| tag.name.to_lowercase() != typed.to_lowercase());
    starting.truncate(MOST_SUGGESTIONS);
    starting
}

#[cfg(test)]
mod tests {
    use pigoune_core::{Tag, TagId};

    use super::{fragment_being_typed, names_in, split_finished, suggestions};

    fn tag(number: u8, name: &str) -> Tag {
        Tag {
            id: TagId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("id"),
            name: name.to_owned(),
            parent: None,
        }
    }

    #[test]
    fn commas_separate_several_names_without_blanks_or_repeats() {
        assert_eq!(
            names_in(" logo, social ,, Logo,bleu "),
            ["logo", "social", "bleu"]
        );
        assert!(names_in(" , ").is_empty());
    }

    #[test]
    fn a_comma_finishes_the_names_before_it_and_keeps_what_follows() {
        assert_eq!(
            split_finished("logo,"),
            (vec!["logo".to_owned()], String::new())
        );
        assert_eq!(
            split_finished("logo, bleu, so"),
            (vec!["logo".to_owned(), "bleu".to_owned()], "so".to_owned())
        );
        assert_eq!(split_finished("logo"), (Vec::new(), "logo".to_owned()));
        assert_eq!(split_finished(" , "), (Vec::new(), String::new()));
    }

    #[test]
    fn the_fragment_being_typed_is_the_last_one() {
        assert_eq!(fragment_being_typed("logo, so"), "so");
        assert_eq!(fragment_being_typed("lo"), "lo");
        assert_eq!(fragment_being_typed("logo bleu fl"), "fl");
        assert_eq!(fragment_being_typed("logo "), "");
    }

    #[test]
    fn a_space_ends_a_tag_like_a_comma() {
        assert_eq!(
            names_in("logo bleu  flat,rouge"),
            ["logo", "bleu", "flat", "rouge"]
        );
        assert_eq!(
            split_finished("logo bleu fl"),
            (vec!["logo".to_owned(), "bleu".to_owned()], "fl".to_owned())
        );
        assert_eq!(
            split_finished("logo "),
            (vec!["logo".to_owned()], String::new())
        );
        assert_eq!(split_finished(" "), (Vec::new(), String::new()));
        assert_eq!(
            split_finished("logo\u{a0}bleu"),
            (vec!["logo".to_owned()], "bleu".to_owned())
        );
    }

    #[test]
    fn a_name_is_cut_at_twenty_characters() {
        let long = "abcdefghijklmnopqrstuvwxyz";
        assert_eq!(names_in(long), ["abcdefghijklmnopqrst"]);
        assert_eq!(
            split_finished(&format!("logo {long}")),
            (vec!["logo".to_owned()], "abcdefghijklmnopqrst".to_owned())
        );
        assert_eq!(names_in(&"é".repeat(25))[0].chars().count(), 20);
    }

    #[test]
    fn suggestions_put_beginnings_first_and_skip_known_tags() {
        let all = [
            tag(1, "Animaux"),
            tag(2, "logo"),
            tag(3, "Logos animés"),
            tag(4, "blog"),
        ];
        let names = |found: Vec<&Tag>| found.iter().map(|tag| tag.name.clone()).collect::<Vec<_>>();
        assert_eq!(
            names(suggestions(&all, "lo", &[])),
            ["logo", "Logos animés", "blog"]
        );
        assert_eq!(
            names(suggestions(&all, "lo", &[all[1].id])),
            ["Logos animés", "blog"]
        );
        assert_eq!(names(suggestions(&all, "LOGO", &[])), ["Logos animés"]);
        assert!(suggestions(&all, "  ", &[]).is_empty());
    }

    #[test]
    fn suggestions_ignore_accents_and_ligatures_but_keep_distinct_names() {
        let all = [tag(1, "Étoile"), tag(2, "Cœur"), tag(3, "etoile")];
        let names = |found: Vec<&Tag>| found.iter().map(|tag| tag.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(suggestions(&all, "eto", &[])), ["Étoile", "etoile"]);
        assert_eq!(names(suggestions(&all, "etoile", &[])), ["Étoile"]);
        assert_eq!(names(suggestions(&all, "coeur", &[])), ["Cœur"]);
        assert_eq!(names(suggestions(&all, "cœ", &[])), ["Cœur"]);
    }
}
