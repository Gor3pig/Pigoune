use gtk::gdk::DragAction;
use pigoune_core::AssetView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropOffer {
    pub allowed: DragAction,
    pub preferred: DragAction,
}

impl DropOffer {
    pub const REFUSED: Self = Self {
        allowed: DragAction::empty(),
        preferred: DragAction::empty(),
    };

    pub const COPY_ONLY: Self = Self {
        allowed: DragAction::COPY,
        preferred: DragAction::COPY,
    };

    pub const MOVE_ONLY: Self = Self {
        allowed: DragAction::MOVE,
        preferred: DragAction::MOVE,
    };

    pub const MOVE_OR_COPY: Self = Self {
        allowed: DragAction::MOVE.union(DragAction::COPY),
        preferred: DragAction::MOVE,
    };

    pub fn is_refused(self) -> bool {
        self.allowed.is_empty()
    }
}

pub fn offer_for_assets(target: AssetView, displayed: AssetView) -> DropOffer {
    match target {
        AssetView::Collection(to) => match displayed {
            AssetView::Collection(from) if from == to => DropOffer::REFUSED,
            AssetView::Collection(_) => DropOffer::MOVE_OR_COPY,
            _ => DropOffer::COPY_ONLY,
        },
        AssetView::Tag(_) | AssetView::Favorites => DropOffer::COPY_ONLY,
        AssetView::Trash => DropOffer::MOVE_ONLY,
        AssetView::All | AssetView::Unclassified | AssetView::Smart(_) => DropOffer::REFUSED,
    }
}

pub fn keeps_source(selected: DragAction, control_is_held: bool) -> bool {
    if selected.is_empty() {
        control_is_held
    } else {
        selected.contains(DragAction::COPY)
    }
}

#[cfg(test)]
mod tests {
    use pigoune_core::{CollectionId, SmartCollectionId, TagId};

    use super::*;

    fn collection(last: u8) -> AssetView {
        AssetView::Collection(
            CollectionId::parse(&format!("00000000-0000-7000-8000-0000000000{last:02x}"))
                .expect("id"),
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
    fn dropping_from_a_collection_on_another_moves_by_default_and_copies_with_control() {
        let offer = offer_for_assets(collection(2), collection(1));
        assert_eq!(offer, DropOffer::MOVE_OR_COPY);
        assert_eq!(offer.preferred, DragAction::MOVE);
        assert!(offer.allowed.contains(DragAction::COPY));
    }

    #[test]
    fn dropping_on_the_collection_being_shown_is_refused() {
        assert!(offer_for_assets(collection(1), collection(1)).is_refused());
    }

    #[test]
    fn dropping_from_a_view_that_is_not_a_collection_only_adds() {
        for displayed in [
            AssetView::All,
            AssetView::Unclassified,
            AssetView::Favorites,
            tag(),
            smart(),
        ] {
            assert_eq!(
                offer_for_assets(collection(2), displayed),
                DropOffer::COPY_ONLY
            );
        }
    }

    #[test]
    fn tags_and_favorites_are_copies_and_the_trash_is_a_move() {
        assert_eq!(offer_for_assets(tag(), collection(1)), DropOffer::COPY_ONLY);
        assert_eq!(
            offer_for_assets(AssetView::Favorites, AssetView::All),
            DropOffer::COPY_ONLY
        );
        assert_eq!(
            offer_for_assets(AssetView::Trash, collection(1)),
            DropOffer::MOVE_ONLY
        );
    }

    #[test]
    fn the_other_views_receive_nothing() {
        for target in [AssetView::All, AssetView::Unclassified, smart()] {
            assert!(offer_for_assets(target, collection(1)).is_refused());
        }
    }

    #[test]
    fn the_source_is_kept_when_the_chosen_action_is_a_copy() {
        assert!(keeps_source(DragAction::COPY, false));
        assert!(!keeps_source(DragAction::MOVE, true));
    }

    #[test]
    fn before_an_action_is_chosen_the_control_key_decides() {
        assert!(keeps_source(DragAction::empty(), true));
        assert!(!keeps_source(DragAction::empty(), false));
    }
}
