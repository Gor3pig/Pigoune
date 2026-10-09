use pigoune_core::AssetView;

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
}
