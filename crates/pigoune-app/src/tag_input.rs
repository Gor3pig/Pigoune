use std::collections::HashSet;

use pigoune_core::{Tag, TagId};

const SEPARATOR: char = ',';
const MOST_SUGGESTIONS: usize = 8;

pub fn names_in(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.split(SEPARATOR)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|name| seen.insert(name.to_lowercase()))
        .map(str::to_owned)
        .collect()
}

pub fn split_finished(text: &str) -> (Vec<String>, String) {
    match text.rfind(SEPARATOR) {
        Some(position) => (
            names_in(&text[..position]),
            text[position + SEPARATOR.len_utf8()..]
                .trim_start()
                .to_owned(),
        ),
        None => (Vec::new(), text.to_owned()),
    }
}

pub fn fragment_being_typed(text: &str) -> &str {
    text.rsplit(SEPARATOR).next().unwrap_or_default().trim()
}

pub fn with_last_fragment_replaced(text: &str, name: &str) -> String {
    match text.rfind(SEPARATOR) {
        Some(position) => format!("{}{SEPARATOR} {name}", &text[..position]),
        None => name.to_owned(),
    }
}

pub fn suggestions<'a>(all: &'a [Tag], typed: &str, already: &[TagId]) -> Vec<&'a Tag> {
    let typed = typed.trim().to_lowercase();
    if typed.is_empty() {
        return Vec::new();
    }
    let candidates = all.iter().filter(|tag| !already.contains(&tag.id));
    let (mut starting, mut containing): (Vec<&Tag>, Vec<&Tag>) = candidates
        .filter(|tag| tag.name.to_lowercase().contains(&typed))
        .partition(|tag| tag.name.to_lowercase().starts_with(&typed));
    starting.append(&mut containing);
    starting.retain(|tag| tag.name.to_lowercase() != typed);
    starting.truncate(MOST_SUGGESTIONS);
    starting
}

#[cfg(test)]
mod tests {
    use pigoune_core::{Tag, TagId};

    use super::{
        fragment_being_typed, names_in, split_finished, suggestions, with_last_fragment_replaced,
    };

    fn tag(number: u8, name: &str) -> Tag {
        Tag {
            id: TagId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("id"),
            name: name.to_owned(),
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
        assert_eq!(
            with_last_fragment_replaced("logo, so", "social"),
            "logo, social"
        );
        assert_eq!(with_last_fragment_replaced("so", "social"), "social");
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
}
