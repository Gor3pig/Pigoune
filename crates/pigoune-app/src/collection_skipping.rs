use gettextrs::gettext;
use pigoune_core::Collection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuplicateSkipping {
    Included,
    Skipped,
    SkippedBy(String),
}

#[must_use]
pub fn state_of(collection: &Collection, skipped_by: Option<&str>) -> DuplicateSkipping {
    if collection.skip_duplicates {
        DuplicateSkipping::Skipped
    } else {
        skipped_by.map_or(DuplicateSkipping::Included, |name| {
            DuplicateSkipping::SkippedBy(name.to_owned())
        })
    }
}

#[must_use]
pub fn skipped_by_for_children(
    collection: &Collection,
    skipped_by: Option<&str>,
) -> Option<String> {
    if collection.skip_duplicates {
        Some(collection.name.clone())
    } else {
        skipped_by.map(str::to_owned)
    }
}

#[must_use]
pub fn menu_label(state: &DuplicateSkipping) -> String {
    match state {
        DuplicateSkipping::Included => gettext("Leave Out of Duplicate Search"),
        DuplicateSkipping::Skipped => gettext("Include in Duplicate Search"),
        DuplicateSkipping::SkippedBy(parent) => {
            gettext("Left Out Through “{name}”").replace("{name}", parent)
        }
    }
}

#[must_use]
pub fn mark_tooltip(state: &DuplicateSkipping) -> Option<String> {
    match state {
        DuplicateSkipping::Included => None,
        DuplicateSkipping::Skipped => Some(gettext("Left out of the duplicate search")),
        DuplicateSkipping::SkippedBy(parent) => Some(
            gettext("Left out of the duplicate search through “{name}”").replace("{name}", parent),
        ),
    }
}

#[must_use]
pub fn confirmation_title(name: &str, leaving_out: bool) -> String {
    if leaving_out {
        gettext("Leave “{name}” out of the duplicate search?")
    } else {
        gettext("Include “{name}” in the duplicate search again?")
    }
    .replace("{name}", name)
}

#[must_use]
pub fn confirmation_body(leaving_out: bool) -> String {
    if leaving_out {
        gettext(
            "This collection and its sub-collections will no longer be checked for duplicates. An asset that is also in another collection stays in the search. Nothing is deleted, and you can undo this.",
        )
    } else {
        gettext(
            "This collection and its sub-collections will be checked again the next time you look for duplicates.",
        )
    }
}

#[must_use]
pub fn confirmation_button(leaving_out: bool) -> String {
    if leaving_out {
        gettext("_Leave Out")
    } else {
        gettext("_Include")
    }
}

#[cfg(test)]
mod tests {
    use pigoune_core::{CollectionId, CollectionLook};

    use super::*;

    fn collection(name: &str, skip: bool) -> Collection {
        Collection {
            id: CollectionId::parse("00000000-0000-7000-8000-000000000001").expect("valid id"),
            name: name.to_owned(),
            parent: None,
            position: 0,
            created_at_unix_ms: 0,
            look: CollectionLook::default(),
            skip_duplicates: skip,
        }
    }

    #[test]
    fn a_collection_without_the_setting_or_a_skipped_parent_is_included() {
        assert_eq!(
            state_of(&collection("Logos", false), None),
            DuplicateSkipping::Included
        );
    }

    #[test]
    fn a_collection_with_the_setting_is_skipped() {
        assert_eq!(
            state_of(&collection("Logos", true), None),
            DuplicateSkipping::Skipped
        );
    }

    #[test]
    fn a_collection_under_a_skipped_parent_is_skipped_by_it() {
        assert_eq!(
            state_of(&collection("2025", false), Some("Logos")),
            DuplicateSkipping::SkippedBy("Logos".to_owned())
        );
    }

    #[test]
    fn a_collection_with_its_own_setting_under_a_skipped_parent_is_skipped_on_its_own() {
        assert_eq!(
            state_of(&collection("2025", true), Some("Logos")),
            DuplicateSkipping::Skipped
        );
    }

    #[test]
    fn children_are_skipped_by_the_nearest_ancestor_that_has_the_setting() {
        let parent = collection("Logos", true);
        assert_eq!(
            skipped_by_for_children(&parent, None),
            Some("Logos".to_owned())
        );
        let middle = collection("2025", false);
        assert_eq!(
            skipped_by_for_children(&middle, Some("Logos")),
            Some("Logos".to_owned())
        );
        let own = collection("Spring", true);
        assert_eq!(
            skipped_by_for_children(&own, Some("Logos")),
            Some("Spring".to_owned())
        );
        assert_eq!(
            skipped_by_for_children(&collection("Free", false), None),
            None
        );
    }

    #[test]
    fn the_menu_offers_to_leave_out_then_to_restore_and_names_the_parent_when_inherited() {
        assert!(menu_label(&DuplicateSkipping::Included).starts_with("Leave Out"));
        assert!(menu_label(&DuplicateSkipping::Skipped).starts_with("Include"));
        let inherited = menu_label(&DuplicateSkipping::SkippedBy("Logos".to_owned()));
        assert!(inherited.starts_with("Left Out Through") && inherited.contains("“Logos”"));
    }

    #[test]
    fn only_a_skipped_collection_has_a_mark_and_the_inherited_one_names_its_parent() {
        assert_eq!(mark_tooltip(&DuplicateSkipping::Included), None);
        assert!(mark_tooltip(&DuplicateSkipping::Skipped).is_some());
        let inherited = mark_tooltip(&DuplicateSkipping::SkippedBy("Logos".to_owned()));
        assert!(inherited.is_some_and(|text| text.contains("“Logos”")));
    }

    #[test]
    fn both_confirmations_name_the_collection_and_say_what_the_button_does() {
        assert!(confirmation_title("Logos", true).contains("“Logos”"));
        assert!(confirmation_title("Logos", false).contains("“Logos”"));
        assert_ne!(
            confirmation_title("Logos", true),
            confirmation_title("Logos", false)
        );
        assert!(confirmation_body(true).contains("undo"));
        assert!(confirmation_body(false).contains("checked again"));
        assert_eq!(confirmation_button(true), "_Leave Out");
        assert_eq!(confirmation_button(false), "_Include");
    }
}
