use std::cmp::Ordering;

use pigoune_core::{SmartCollection, SmartCollectionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartCollectionCriterion {
    Name,
    Created,
    Custom,
}

impl SmartCollectionCriterion {
    #[must_use]
    pub fn from_setting(value: &str) -> Self {
        match value {
            "created" => Self::Created,
            "custom" => Self::Custom,
            _ => Self::Name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartCollectionOrder {
    pub criterion: SmartCollectionCriterion,
    pub reversed: bool,
}

impl SmartCollectionOrder {
    pub fn sort<Key: Ord>(
        self,
        collections: &mut [SmartCollection],
        collation_key: impl Fn(&str) -> Key,
    ) {
        collections.sort_by_cached_key(|collection| {
            (
                collation_key(&collection.name),
                collection.created_at_unix_ms,
            )
        });
        let natural = |first: &SmartCollection, second: &SmartCollection| match self.criterion {
            SmartCollectionCriterion::Name => Ordering::Equal,
            SmartCollectionCriterion::Custom => first.position.cmp(&second.position),
            SmartCollectionCriterion::Created => {
                second.created_at_unix_ms.cmp(&first.created_at_unix_ms)
            }
        };
        collections.sort_by(natural);
        if self.reversed {
            collections.reverse();
        }
    }
}

#[must_use]
pub fn reordered(
    shown: &[SmartCollectionId],
    dragged: SmartCollectionId,
    target: SmartCollectionId,
    after: bool,
) -> Vec<SmartCollectionId> {
    if dragged == target {
        return shown.to_vec();
    }
    let mut order: Vec<SmartCollectionId> =
        shown.iter().copied().filter(|id| *id != dragged).collect();
    let index = order
        .iter()
        .position(|id| *id == target)
        .map_or(order.len(), |index| index + usize::from(after));
    order.insert(index, dragged);
    order
}

#[cfg(test)]
mod tests {
    use pigoune_core::{AssetFilter, SmartCollection, SmartCollectionId};

    use super::{SmartCollectionCriterion, SmartCollectionOrder, reordered};

    fn collection(name: &str, created_at_unix_ms: i64) -> SmartCollection {
        SmartCollection {
            id: SmartCollectionId::parse(&format!(
                "00000000-0000-7000-8000-{created_at_unix_ms:012}"
            ))
            .expect("id"),
            name: name.to_owned(),
            filter: AssetFilter::default(),
            position: 10 - created_at_unix_ms,
            created_at_unix_ms,
        }
    }

    fn names(order: SmartCollectionOrder) -> Vec<String> {
        let mut collections = vec![
            collection("bravo", 1),
            collection("Alpha", 3),
            collection("charlie", 2),
        ];
        order.sort(&mut collections, str::to_lowercase);
        collections
            .into_iter()
            .map(|collection| collection.name)
            .collect()
    }

    #[test]
    fn by_name_follows_the_collation_key() {
        let order = SmartCollectionOrder {
            criterion: SmartCollectionCriterion::Name,
            reversed: false,
        };
        assert_eq!(names(order), ["Alpha", "bravo", "charlie"]);
    }

    #[test]
    fn by_date_shows_the_newest_first() {
        let order = SmartCollectionOrder {
            criterion: SmartCollectionCriterion::Created,
            reversed: false,
        };
        assert_eq!(names(order), ["Alpha", "charlie", "bravo"]);
    }

    #[test]
    fn reversing_turns_the_order_around() {
        let order = SmartCollectionOrder {
            criterion: SmartCollectionCriterion::Name,
            reversed: true,
        };
        assert_eq!(names(order), ["charlie", "bravo", "Alpha"]);
    }

    #[test]
    fn anything_unexpected_sorts_by_name() {
        assert_eq!(
            SmartCollectionCriterion::from_setting("nonsense"),
            SmartCollectionCriterion::Name
        );
        assert_eq!(
            SmartCollectionCriterion::from_setting("created"),
            SmartCollectionCriterion::Created
        );
    }

    #[test]
    fn the_custom_order_follows_the_positions() {
        let order = SmartCollectionOrder {
            criterion: SmartCollectionCriterion::Custom,
            reversed: false,
        };
        assert_eq!(names(order), ["Alpha", "charlie", "bravo"]);
    }

    #[test]
    fn a_dragged_smart_collection_lands_before_or_after_its_target() {
        let ids: Vec<SmartCollectionId> =
            (1..=3).map(|number| collection("x", number).id).collect();
        assert_eq!(
            reordered(&ids, ids[2], ids[0], false),
            [ids[2], ids[0], ids[1]]
        );
        assert_eq!(
            reordered(&ids, ids[0], ids[2], true),
            [ids[1], ids[2], ids[0]]
        );
        assert_eq!(reordered(&ids, ids[1], ids[1], true), ids);
    }
}
