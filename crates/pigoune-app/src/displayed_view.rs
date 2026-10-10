use pigoune_core::{AssetView, CollectionId, SmartCollectionId, TagId};

pub fn can_widen(view: AssetView) -> bool {
    matches!(
        view,
        AssetView::All
            | AssetView::Favorites
            | AssetView::Unclassified
            | AssetView::Collection(_)
            | AssetView::Tag(_)
    )
}

pub fn view_after_tag_removal(
    current: AssetView,
    removed: &[TagId],
    parent: Option<TagId>,
) -> Option<AssetView> {
    match current {
        AssetView::Tag(shown) if removed.contains(&shown) => {
            Some(parent.map_or(AssetView::All, AssetView::Tag))
        }
        _ => None,
    }
}

pub fn still_exists(
    view: AssetView,
    collections: &[CollectionId],
    tags: &[TagId],
    smart_collections: &[SmartCollectionId],
) -> bool {
    match view {
        AssetView::Collection(id) => collections.contains(&id),
        AssetView::Tag(id) => tags.contains(&id),
        AssetView::Smart(id) => smart_collections.contains(&id),
        AssetView::All | AssetView::Favorites | AssetView::Unclassified | AssetView::Trash => true,
    }
}

pub fn everywhere(setting: bool, chosen_by_click: bool) -> bool {
    setting && !chosen_by_click
}

pub fn narrows_by_click(setting: bool, searching: bool) -> bool {
    setting && searching
}

pub fn shown(chosen: AssetView, search_everywhere: bool, searching: bool) -> AssetView {
    if search_everywhere && searching && can_widen(chosen) {
        AssetView::All
    } else {
        chosen
    }
}

#[cfg(test)]
mod tests {
    use pigoune_core::{CollectionId, SmartCollectionId, TagId};

    use super::*;

    fn collection() -> AssetView {
        AssetView::Collection(
            CollectionId::parse("00000000-0000-7000-8000-000000000007").expect("id"),
        )
    }

    #[test]
    fn a_search_everywhere_shows_the_whole_library() {
        assert_eq!(shown(collection(), true, true), AssetView::All);
        assert_eq!(shown(AssetView::Favorites, true, true), AssetView::All);
        assert_eq!(
            shown(
                AssetView::Tag(TagId::parse("00000000-0000-7000-8000-000000000009").expect("id")),
                true,
                true
            ),
            AssetView::All
        );
    }

    #[test]
    fn without_a_search_the_chosen_view_is_shown() {
        assert_eq!(shown(collection(), true, false), collection());
        assert_eq!(
            shown(AssetView::Favorites, true, false),
            AssetView::Favorites
        );
    }

    #[test]
    fn without_the_setting_the_chosen_view_is_shown() {
        assert_eq!(shown(collection(), false, true), collection());
    }

    #[test]
    fn smart_collections_and_the_trash_are_never_widened() {
        let smart = AssetView::Smart(
            SmartCollectionId::parse("00000000-0000-7000-8000-00000000000b").expect("id"),
        );
        assert_eq!(shown(smart, true, true), smart);
        assert_eq!(shown(AssetView::Trash, true, true), AssetView::Trash);
    }

    #[test]
    fn a_view_chosen_by_a_click_wins_over_the_everywhere_setting() {
        assert!(everywhere(true, false));
        assert!(!everywhere(true, true));
        assert!(!everywhere(false, false));
        assert!(!everywhere(false, true));
    }

    #[test]
    fn a_click_narrows_the_search_only_when_it_would_have_been_widened() {
        assert!(narrows_by_click(true, true));
        assert!(!narrows_by_click(true, false));
        assert!(!narrows_by_click(false, true));
    }

    #[test]
    fn a_view_exists_while_its_item_is_still_listed() {
        let collection_id =
            CollectionId::parse("00000000-0000-7000-8000-000000000007").expect("id");
        let tag_id = TagId::parse("00000000-0000-7000-8000-000000000009").expect("id");
        let smart_id =
            SmartCollectionId::parse("00000000-0000-7000-8000-00000000000b").expect("id");

        assert!(still_exists(
            AssetView::Collection(collection_id),
            &[collection_id],
            &[],
            &[]
        ));
        assert!(!still_exists(
            AssetView::Collection(collection_id),
            &[],
            &[tag_id],
            &[smart_id]
        ));
        assert!(!still_exists(
            AssetView::Tag(tag_id),
            &[collection_id],
            &[],
            &[smart_id]
        ));
        assert!(!still_exists(
            AssetView::Smart(smart_id),
            &[collection_id],
            &[tag_id],
            &[]
        ));
        assert!(still_exists(
            AssetView::Smart(smart_id),
            &[],
            &[],
            &[smart_id]
        ));
    }

    fn tag_id(number: u8) -> TagId {
        TagId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("id")
    }

    #[test]
    fn a_removed_tag_that_is_shown_falls_back_to_its_parent() {
        let (animals, birds, hawks) = (tag_id(1), tag_id(2), tag_id(3));
        assert_eq!(
            view_after_tag_removal(AssetView::Tag(hawks), &[hawks], Some(birds)),
            Some(AssetView::Tag(birds))
        );
        assert_eq!(
            view_after_tag_removal(AssetView::Tag(hawks), &[birds, hawks], Some(animals)),
            Some(AssetView::Tag(animals))
        );
    }

    #[test]
    fn a_removed_top_level_tag_falls_back_to_the_whole_library() {
        let animals = tag_id(1);
        assert_eq!(
            view_after_tag_removal(AssetView::Tag(animals), &[animals], None),
            Some(AssetView::All)
        );
    }

    #[test]
    fn a_view_that_survives_the_removal_is_left_alone() {
        let (animals, birds) = (tag_id(1), tag_id(2));
        assert_eq!(
            view_after_tag_removal(AssetView::Tag(birds), &[animals], None),
            None
        );
        assert_eq!(
            view_after_tag_removal(AssetView::Favorites, &[animals], None),
            None
        );
    }

    #[test]
    fn the_fixed_views_always_exist() {
        for view in [
            AssetView::All,
            AssetView::Favorites,
            AssetView::Unclassified,
            AssetView::Trash,
        ] {
            assert!(still_exists(view, &[], &[], &[]));
        }
    }
}
