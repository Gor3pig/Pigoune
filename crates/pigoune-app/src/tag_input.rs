use std::collections::HashSet;

use pigoune_core::{LONGEST_TAG_NAME, Tag, TagId, comparable, same_tag_name};

use crate::tag_tree;

const MOST_SUGGESTIONS: usize = 8;

pub fn is_separator(character: char) -> bool {
    character == ',' || character.is_whitespace()
}

const LEVEL_SEPARATOR: char = '/';

fn shortened(path: &str) -> String {
    path.split(LEVEL_SEPARATOR)
        .filter(|name| !name.is_empty())
        .map(|name| name.chars().take(LONGEST_TAG_NAME).collect::<String>())
        .collect::<Vec<_>>()
        .join("/")
}

fn shortened_fragment(fragment: &str) -> String {
    let mut path = shortened(fragment);
    if !path.is_empty() && fragment.ends_with(LEVEL_SEPARATOR) {
        path.push(LEVEL_SEPARATOR);
    }
    path
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
                shortened_fragment(&text[position + separator_length..]),
            )
        }
        None => (Vec::new(), shortened_fragment(text)),
    }
}

pub fn fragment_being_typed(text: &str) -> &str {
    text.rsplit(is_separator).next().unwrap_or_default()
}

pub struct CreateOffer {
    pub path: String,
    pub name: String,
    pub parents: String,
}

pub fn path_of(all: &[Tag], tag: &Tag) -> String {
    let mut names = ancestors_of(all, tag);
    names.push(tag.name.as_str());
    names.join("/")
}

pub fn ancestors_of<'a>(all: &'a [Tag], tag: &Tag) -> Vec<&'a str> {
    tag_tree::ancestors(all, tag)
        .into_iter()
        .map(|parent| parent.name.as_str())
        .collect()
}

fn child_named<'a>(all: &'a [Tag], parent: Option<TagId>, name: &str) -> Option<&'a Tag> {
    all.iter()
        .find(|tag| tag.parent == parent && same_tag_name(&tag.name, name))
}

struct UnknownParent;

fn resolved_parent(all: &[Tag], parents: &str) -> Result<Option<TagId>, UnknownParent> {
    let mut current = None;
    for name in parents
        .split(LEVEL_SEPARATOR)
        .filter(|name| !name.is_empty())
    {
        current = Some(child_named(all, current, name).ok_or(UnknownParent)?.id);
    }
    Ok(current)
}

pub fn suggestions<'a>(all: &'a [Tag], typed: &str, already: &[TagId]) -> Vec<&'a Tag> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Vec::new();
    }
    let (parents, wanted_text) = typed.rsplit_once(LEVEL_SEPARATOR).unwrap_or(("", typed));
    let Ok(parent) = resolved_parent(all, parents) else {
        return Vec::new();
    };
    let nested = typed.contains(LEVEL_SEPARATOR);
    let wanted = comparable(wanted_text);
    let candidates = all
        .iter()
        .filter(|tag| !already.contains(&tag.id))
        .filter(|tag| !nested || tag.parent == parent);
    let (mut starting, mut containing): (Vec<&Tag>, Vec<&Tag>) = candidates
        .filter(|tag| comparable(&tag.name).contains(&wanted))
        .partition(|tag| comparable(&tag.name).starts_with(&wanted));
    starting.append(&mut containing);
    starting.retain(|tag| {
        let known_by_enter = nested || tag.parent.is_none();
        !(known_by_enter && tag.name.to_lowercase() == wanted_text.to_lowercase())
    });
    starting.truncate(MOST_SUGGESTIONS);
    starting
}

pub enum EnterTarget {
    Use(String),
    Ambiguous,
}

pub fn enter_target(all: &[Tag], typed: &str) -> EnterTarget {
    if typed.contains(LEVEL_SEPARATOR) || child_named(all, None, typed).is_some() {
        return EnterTarget::Use(typed.to_owned());
    }
    let mut same_name = all.iter().filter(|tag| same_tag_name(&tag.name, typed));
    match (same_name.next(), same_name.next()) {
        (None, _) => EnterTarget::Use(typed.to_owned()),
        (Some(only), None) => EnterTarget::Use(path_of(all, only)),
        (Some(_), Some(_)) => EnterTarget::Ambiguous,
    }
}

pub fn create_offer(all: &[Tag], typed: &str) -> Option<CreateOffer> {
    let path = shortened(typed.trim());
    let (parents, name) = path.rsplit_once(LEVEL_SEPARATOR)?;
    if name.is_empty() || typed.trim().ends_with(LEVEL_SEPARATOR) {
        return None;
    }
    let exists =
        resolved_parent(all, parents).is_ok_and(|parent| child_named(all, parent, name).is_some());
    if exists {
        return None;
    }
    Some(CreateOffer {
        name: name.to_owned(),
        parents: parents.replace('/', " › "),
        path,
    })
}

#[cfg(test)]
mod tests {
    use pigoune_core::{Tag, TagId};

    use super::{
        EnterTarget, create_offer, enter_target, fragment_being_typed, names_in, path_of,
        split_finished, suggestions,
    };

    fn tag(number: u8, name: &str) -> Tag {
        Tag {
            id: TagId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("id"),
            name: name.to_owned(),
            parent: None,
        }
    }

    #[test]
    fn a_parent_typed_with_another_accent_is_not_the_existing_tag() {
        let subject = tag(1, "Sujet");
        let all = vec![subject.clone(), child(2, "Animaux", &subject)];

        assert!(suggestions(&all, "Sujét/", &[]).is_empty());
        assert_eq!(
            suggestions(&all, "sujet/", &[])
                .iter()
                .map(|found| found.name.as_str())
                .collect::<Vec<_>>(),
            ["Animaux"]
        );
    }

    #[test]
    fn a_name_differing_only_by_an_accent_is_offered_for_creation() {
        let animals = tag(1, "animaux");
        let all = vec![animals.clone(), child(2, "chèvre", &animals)];

        let offer = create_offer(&all, "animaux/chevre").expect("offer");

        assert_eq!(offer.name, "chevre");
        assert!(create_offer(&all, "Animaux/Chèvre").is_none());
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

    fn child(number: u8, name: &str, parent: &Tag) -> Tag {
        Tag {
            parent: Some(parent.id),
            ..tag(number, name)
        }
    }

    #[test]
    fn each_level_of_a_path_is_cut_at_twenty_characters() {
        let long = "abcdefghijklmnopqrstuvwxyz";
        assert_eq!(
            names_in(&format!("{long}/{long}")),
            ["abcdefghijklmnopqrst/abcdefghijklmnopqrst"]
        );
        assert_eq!(names_in("/a//b/"), ["a/b"]);
    }

    #[test]
    fn a_slash_that_ends_the_fragment_is_kept_while_typing() {
        assert_eq!(
            split_finished("logo animaux/"),
            (vec!["logo".to_owned()], "animaux/".to_owned())
        );
        assert_eq!(
            split_finished("animaux/ch"),
            (Vec::new(), "animaux/ch".to_owned())
        );
        assert_eq!(split_finished("/"), (Vec::new(), String::new()));
    }

    #[test]
    fn a_path_names_a_tag_with_its_parents() {
        let subject = tag(1, "Sujet");
        let animals = child(2, "Animaux", &subject);
        let goat = child(3, "chèvre", &animals);
        let all = [subject, animals, goat.clone()];
        assert_eq!(path_of(&all, &goat), "Sujet/Animaux/chèvre");
    }

    #[test]
    fn typing_a_parent_then_a_slash_suggests_its_children_only() {
        let subject = tag(1, "Sujet");
        let animals = child(2, "Animaux", &subject);
        let goat = child(3, "chèvre", &animals);
        let cat = child(4, "chat", &animals);
        let tree = child(5, "arbre", &subject);
        let loose = tag(6, "chèvre");
        let all = [subject, animals, goat, cat, tree, loose];
        let names = |found: Vec<&Tag>| found.iter().map(|tag| tag.name.clone()).collect::<Vec<_>>();

        assert_eq!(
            names(suggestions(&all, "sujet/animaux/", &[])),
            ["chèvre", "chat"]
        );
        assert_eq!(
            names(suggestions(&all, "Sujet/Animaux/ch", &[])),
            ["chèvre", "chat"]
        );
        assert_eq!(names(suggestions(&all, "sujet/animaux/cha", &[])), ["chat"]);
        assert!(suggestions(&all, "inconnu/ch", &[]).is_empty());
        assert_eq!(names(suggestions(&all, "chè", &[])), ["chèvre", "chèvre"]);
    }

    #[test]
    fn a_nested_tag_with_the_typed_name_is_still_suggested() {
        let subject = tag(1, "Sujet");
        let goat = child(2, "chèvre", &subject);
        let all = [subject, goat];
        let names = |found: Vec<&Tag>| found.iter().map(|tag| tag.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(suggestions(&all, "chèvre", &[])), ["chèvre"]);
    }

    #[test]
    fn creating_is_offered_for_a_new_name_under_a_typed_parent() {
        let subject = tag(1, "Sujet");
        let animals = child(2, "Animaux", &subject);
        let all = [subject, animals];

        let offer = create_offer(&all, "sujet/animaux/chouette").expect("offer");
        assert_eq!(offer.name, "chouette");
        assert_eq!(offer.parents, "sujet › animaux");
        assert_eq!(offer.path, "sujet/animaux/chouette");

        assert!(create_offer(&all, "chouette").is_none());
        assert!(create_offer(&all, "sujet/animaux/").is_none());
        assert!(create_offer(&all, "sujet/animaux").is_none());
        let deeper = create_offer(&all, "couleur/rouge").expect("offer");
        assert_eq!(deeper.parents, "couleur");
    }

    fn used(target: EnterTarget) -> String {
        match target {
            EnterTarget::Use(path) => path,
            EnterTarget::Ambiguous => "ambiguous".to_owned(),
        }
    }

    #[test]
    fn enter_reuses_the_only_tag_with_the_typed_name_whatever_its_level() {
        let animals = tag(1, "Animals");
        let all = [animals.clone(), child(2, "Birds", &animals)];

        assert_eq!(used(enter_target(&all, "birds")), "Animals/Birds");
        assert_eq!(used(enter_target(&all, "Bírds")), "Bírds");
    }

    #[test]
    fn enter_prefers_the_first_level_tag_with_the_typed_name() {
        let animals = tag(1, "Animals");
        let all = [
            animals.clone(),
            child(2, "Birds", &animals),
            tag(3, "Birds"),
        ];

        assert_eq!(used(enter_target(&all, "Birds")), "Birds");
    }

    #[test]
    fn enter_does_not_guess_between_several_nested_tags() {
        let animals = tag(1, "Animals");
        let games = tag(2, "Games");
        let all = [
            animals.clone(),
            games.clone(),
            child(3, "Birds", &animals),
            child(4, "Birds", &games),
        ];

        assert_eq!(used(enter_target(&all, "Birds")), "ambiguous");
    }

    #[test]
    fn enter_creates_a_new_tag_for_an_unknown_name_and_keeps_typed_paths() {
        let animals = tag(1, "Animals");
        let all = [animals.clone(), child(2, "Birds", &animals)];

        assert_eq!(used(enter_target(&all, "Fish")), "Fish");
        assert_eq!(used(enter_target(&all, "Birds/Owls")), "Birds/Owls");
    }
}
