use std::collections::HashMap;

use gettextrs::{gettext, ngettext};
use pigoune_core::{Collection, CollectionId};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftOutEntry {
    pub id: CollectionId,
    pub name: String,
    pub parent_name: Option<String>,
    pub covered_below: usize,
}

#[must_use]
pub fn left_out_entries(collections: &[Collection]) -> Vec<LeftOutEntry> {
    let by_id: HashMap<CollectionId, &Collection> = collections
        .iter()
        .map(|collection| (collection.id, collection))
        .collect();
    collections
        .iter()
        .filter(|collection| collection.skip_duplicates)
        .map(|collection| LeftOutEntry {
            id: collection.id,
            name: collection.name.clone(),
            parent_name: collection
                .parent
                .and_then(|parent| by_id.get(&parent))
                .map(|parent| parent.name.clone()),
            covered_below: collections
                .iter()
                .filter(|other| !other.skip_duplicates)
                .filter(|other| nearest_skipped_ancestor(other, &by_id) == Some(collection.id))
                .count(),
        })
        .collect()
}

fn nearest_skipped_ancestor(
    collection: &Collection,
    by_id: &HashMap<CollectionId, &Collection>,
) -> Option<CollectionId> {
    let mut current = collection.parent;
    while let Some(id) = current {
        let ancestor = by_id.get(&id)?;
        if ancestor.skip_duplicates {
            return Some(id);
        }
        current = ancestor.parent;
    }
    None
}

#[must_use]
pub fn left_out_subtitle(entry: &LeftOutEntry) -> String {
    let place = match &entry.parent_name {
        Some(parent) => gettext("In “{name}”").replace("{name}", parent),
        None => gettext("At the top level"),
    };
    if entry.covered_below == 0 {
        return place;
    }
    let covered = ngettext(
        "{count} sub-collection is left out with it",
        "{count} sub-collections are left out with it",
        u32::try_from(entry.covered_below).unwrap_or(u32::MAX),
    )
    .replace("{count}", &entry.covered_below.to_string());
    format!("{place} · {covered}")
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

    fn node(number: u8, name: &str, parent: Option<u8>, skip: bool) -> Collection {
        let id = |number: u8| {
            CollectionId::parse(&format!("00000000-0000-7000-8000-0000000000{number:02}"))
                .expect("valid id")
        };
        Collection {
            id: id(number),
            parent: parent.map(id),
            ..collection(name, skip)
        }
    }

    #[test]
    fn only_the_collections_with_the_setting_are_listed_with_their_parent() {
        let collections = [
            node(1, "Photos", None, false),
            node(2, "Drafts", Some(1), true),
            node(3, "Archive", None, true),
        ];
        let entries = left_out_entries(&collections);
        let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["Drafts", "Archive"]);
        assert_eq!(entries[0].parent_name.as_deref(), Some("Photos"));
        assert_eq!(entries[1].parent_name, None);
    }

    #[test]
    fn sub_collections_are_counted_under_the_nearest_collection_left_out() {
        let collections = [
            node(1, "Logos", None, true),
            node(2, "Old", Some(1), false),
            node(3, "2020", Some(2), false),
            node(4, "Icons", Some(1), true),
            node(5, "Round", Some(4), false),
        ];
        let entries = left_out_entries(&collections);
        assert_eq!(entries[0].covered_below, 2);
        assert_eq!(entries[1].covered_below, 1);
    }

    #[test]
    fn the_subtitle_names_the_place_and_the_sub_collections_left_out_with_it() {
        let entry = |parent: Option<&str>, covered_below| LeftOutEntry {
            id: node(1, "Logos", None, true).id,
            name: "Logos".to_owned(),
            parent_name: parent.map(str::to_owned),
            covered_below,
        };
        assert_eq!(left_out_subtitle(&entry(None, 0)), "At the top level");
        assert_eq!(left_out_subtitle(&entry(Some("Photos"), 0)), "In “Photos”");
        assert_eq!(
            left_out_subtitle(&entry(None, 1)),
            "At the top level · 1 sub-collection is left out with it"
        );
        assert_eq!(
            left_out_subtitle(&entry(Some("Photos"), 3)),
            "In “Photos” · 3 sub-collections are left out with it"
        );
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
