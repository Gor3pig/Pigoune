use pigoune_core::AssetView;

pub fn accepts_files(view: AssetView) -> bool {
    matches!(
        view,
        AssetView::All | AssetView::Unclassified | AssetView::Collection(_)
    )
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

    fn tag() -> AssetView {
        AssetView::Tag(TagId::parse("00000000-0000-7000-8000-000000000009").expect("id"))
    }

    fn smart() -> AssetView {
        AssetView::Smart(
            SmartCollectionId::parse("00000000-0000-7000-8000-00000000000b").expect("id"),
        )
    }

    #[test]
    fn files_enter_only_places() {
        assert!(accepts_files(AssetView::All));
        assert!(accepts_files(AssetView::Unclassified));
        assert!(accepts_files(collection()));
    }

    #[test]
    fn files_are_refused_by_lenses_and_the_trash() {
        assert!(!accepts_files(AssetView::Favorites));
        assert!(!accepts_files(tag()));
        assert!(!accepts_files(smart()));
        assert!(!accepts_files(AssetView::Trash));
    }
}
