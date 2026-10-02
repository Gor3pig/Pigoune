use pigoune_core::{AssetView, CollectionId, TagId};

const ALL: &str = "all";
const UNCLASSIFIED: &str = "unclassified";
const FAVORITES: &str = "favorites";
const TRASH: &str = "trash";
const COLLECTION_PREFIX: &str = "collection:";
const TAG_PREFIX: &str = "tag:";

pub fn to_setting(view: AssetView) -> String {
    match view {
        AssetView::All => ALL.to_owned(),
        AssetView::Unclassified => UNCLASSIFIED.to_owned(),
        AssetView::Favorites => FAVORITES.to_owned(),
        AssetView::Trash => TRASH.to_owned(),
        AssetView::Collection(id) => format!("{COLLECTION_PREFIX}{id}"),
        AssetView::Tag(id) => format!("{TAG_PREFIX}{id}"),
    }
}

pub fn from_setting(value: &str) -> AssetView {
    if value == UNCLASSIFIED {
        return AssetView::Unclassified;
    }
    if value == FAVORITES {
        return AssetView::Favorites;
    }
    if value == TRASH {
        return AssetView::Trash;
    }
    if let Some(tag) = value.strip_prefix(TAG_PREFIX).and_then(TagId::parse) {
        return AssetView::Tag(tag);
    }
    value
        .strip_prefix(COLLECTION_PREFIX)
        .and_then(CollectionId::parse)
        .map_or(AssetView::All, AssetView::Collection)
}

#[cfg(test)]
mod tests {
    use pigoune_core::{AssetView, CollectionId, TagId};

    use super::{from_setting, to_setting};

    #[test]
    fn every_view_survives_a_round_trip_through_the_setting() {
        let collection = CollectionId::parse("00000000-0000-7000-8000-000000000007").expect("id");
        for view in [
            AssetView::All,
            AssetView::Unclassified,
            AssetView::Favorites,
            AssetView::Trash,
            AssetView::Collection(collection),
            AssetView::Tag(TagId::parse("00000000-0000-7000-8000-000000000009").expect("id")),
        ] {
            assert_eq!(from_setting(&to_setting(view)), view);
        }
    }

    #[test]
    fn anything_unexpected_falls_back_to_all() {
        assert_eq!(from_setting(""), AssetView::All);
        assert_eq!(from_setting("collection:nonsense"), AssetView::All);
    }
}
