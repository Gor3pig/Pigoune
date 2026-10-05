use std::cmp::Ordering;
use std::collections::HashMap;

use pigoune_core::{Collection, CollectionCommand, CollectionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollectionCriterion {
    #[default]
    Name,
    Created,
    Custom,
}

impl CollectionCriterion {
    pub fn from_setting(value: &str) -> Self {
        match value {
            "created" => Self::Created,
            "custom" => Self::Custom,
            _ => Self::Name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollectionOrder {
    pub criterion: CollectionCriterion,
    pub reversed: bool,
}

impl CollectionOrder {
    pub fn compare<Key: Ord + ?Sized>(
        self,
        first: (&Collection, &Key),
        second: (&Collection, &Key),
    ) -> Ordering {
        let (first, first_key) = first;
        let (second, second_key) = second;
        let natural = match self.criterion {
            CollectionCriterion::Name => first_key.cmp(second_key),
            CollectionCriterion::Created => {
                second.created_at_unix_ms.cmp(&first.created_at_unix_ms)
            }
            CollectionCriterion::Custom => first.position.cmp(&second.position),
        }
        .then_with(|| first.created_at_unix_ms.cmp(&second.created_at_unix_ms))
        .then_with(|| first.id.cmp(&second.id));
        if self.reversed {
            natural.reverse()
        } else {
            natural
        }
    }
}

pub struct CollectionTree {
    children: HashMap<Option<CollectionId>, Vec<Collection>>,
}

impl CollectionTree {
    pub fn new<Key: Ord>(
        collections: Vec<Collection>,
        order: CollectionOrder,
        name_key: impl Fn(&str) -> Key,
    ) -> Self {
        let mut children: HashMap<Option<CollectionId>, Vec<(Collection, Key)>> = HashMap::new();
        for collection in collections {
            let key = name_key(&collection.name);
            children
                .entry(collection.parent)
                .or_default()
                .push((collection, key));
        }
        let children = children
            .into_iter()
            .map(|(parent, mut siblings)| {
                siblings.sort_by(|first, second| {
                    order.compare((&first.0, &first.1), (&second.0, &second.1))
                });
                (
                    parent,
                    siblings
                        .into_iter()
                        .map(|(collection, _)| collection)
                        .collect(),
                )
            })
            .collect();
        Self { children }
    }

    pub fn children_of(&self, parent: Option<CollectionId>) -> &[Collection] {
        self.children.get(&parent).map_or(&[], Vec::as_slice)
    }

    pub fn find(&self, id: CollectionId) -> Option<&Collection> {
        self.children
            .values()
            .flatten()
            .find(|collection| collection.id == id)
    }

    pub fn arrangements(&self) -> Vec<CollectionCommand> {
        let mut parents: Vec<Option<CollectionId>> = self.children.keys().copied().collect();
        parents.sort();
        parents
            .into_iter()
            .map(|parent| CollectionCommand::Arrange {
                parent,
                order: self
                    .children_of(parent)
                    .iter()
                    .map(|collection| collection.id)
                    .collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use pigoune_core::{Collection, CollectionId, CollectionLook};

    use super::{CollectionCriterion, CollectionOrder, CollectionTree};

    fn collection(
        number: u8,
        name: &str,
        parent: Option<CollectionId>,
        position: i64,
    ) -> Collection {
        Collection {
            id: CollectionId::parse(&format!("00000000-0000-7000-8000-{number:012}"))
                .expect("valid id"),
            name: name.to_owned(),
            parent,
            position,
            created_at_unix_ms: i64::from(number),
            look: CollectionLook::default(),
        }
    }

    fn names(tree: &CollectionTree, parent: Option<CollectionId>) -> Vec<&str> {
        tree.children_of(parent)
            .iter()
            .map(|collection| collection.name.as_str())
            .collect()
    }

    fn tree(criterion: CollectionCriterion, reversed: bool) -> (CollectionTree, CollectionId) {
        let root = collection(1, "Marques", None, 2);
        let root_id = root.id;
        let collections = vec![
            root,
            collection(2, "Avion", None, 1),
            collection(3, "Zèbre", None, 0),
            collection(4, "Tech", Some(root_id), 0),
        ];
        let order = CollectionOrder {
            criterion,
            reversed,
        };
        (
            CollectionTree::new(collections, order, |name: &str| name.to_lowercase()),
            root_id,
        )
    }

    #[test]
    fn collections_are_grouped_under_their_parent() {
        let (tree, root) = tree(CollectionCriterion::Name, false);
        assert_eq!(names(&tree, Some(root)), ["Tech"]);
        assert!(
            tree.children_of(Some(
                CollectionId::parse("00000000-0000-7000-8000-000000000099").expect("id")
            ))
            .is_empty()
        );
    }

    #[test]
    fn each_criterion_orders_siblings() {
        assert_eq!(
            names(&tree(CollectionCriterion::Name, false).0, None),
            ["Avion", "Marques", "Zèbre"]
        );
        assert_eq!(
            names(&tree(CollectionCriterion::Created, false).0, None),
            ["Zèbre", "Avion", "Marques"]
        );
        assert_eq!(
            names(&tree(CollectionCriterion::Custom, false).0, None),
            ["Zèbre", "Avion", "Marques"]
        );
        assert_eq!(
            names(&tree(CollectionCriterion::Name, true).0, None),
            ["Zèbre", "Marques", "Avion"]
        );
    }

    #[test]
    fn unknown_settings_fall_back_to_the_name() {
        assert_eq!(
            CollectionCriterion::from_setting("bogus"),
            CollectionCriterion::Name
        );
        assert_eq!(
            CollectionCriterion::from_setting("custom"),
            CollectionCriterion::Custom
        );
    }
}
