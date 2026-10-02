use pigoune_core::{CollectionCommand, CollectionId};

use crate::collection_sort::CollectionTree;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropZone {
    Before,
    #[default]
    Into,
    After,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionDrop {
    Into(Option<CollectionId>),
    Before(CollectionId),
    After(CollectionId),
}

#[derive(Debug, PartialEq, Eq)]
pub struct DropPlan {
    pub commands: Vec<CollectionCommand>,
    pub switches_to_custom_order: bool,
}

const EDGE_SHARE: f64 = 0.25;

pub fn zone_at(y: f64, height: f64) -> DropZone {
    if y < height * EDGE_SHARE {
        DropZone::Before
    } else if y > height * (1.0 - EDGE_SHARE) {
        DropZone::After
    } else {
        DropZone::Into
    }
}

pub fn plan(
    tree: &CollectionTree,
    dragged: CollectionId,
    drop: CollectionDrop,
    custom_order_shown: bool,
) -> Option<DropPlan> {
    let current_parent = tree.find(dragged)?.parent;
    match drop {
        CollectionDrop::Into(parent) => {
            if parent == Some(dragged) || parent == current_parent {
                return None;
            }
            Some(DropPlan {
                commands: vec![CollectionCommand::Move {
                    id: dragged,
                    parent,
                }],
                switches_to_custom_order: false,
            })
        }
        CollectionDrop::Before(target) | CollectionDrop::After(target) => {
            if target == dragged {
                return None;
            }
            let parent = tree.find(target)?.parent;
            let mut commands = if custom_order_shown {
                Vec::new()
            } else {
                tree.arrangements()
            };
            if parent != current_parent {
                commands.push(CollectionCommand::Move {
                    id: dragged,
                    parent,
                });
            }
            let siblings: Vec<CollectionId> = tree
                .children_of(parent)
                .iter()
                .map(|collection| collection.id)
                .collect();
            let after = matches!(drop, CollectionDrop::After(_));
            commands.push(CollectionCommand::Arrange {
                parent,
                order: placed(&siblings, dragged, target, after),
            });
            Some(DropPlan {
                commands,
                switches_to_custom_order: !custom_order_shown,
            })
        }
    }
}

fn placed(
    siblings: &[CollectionId],
    dragged: CollectionId,
    target: CollectionId,
    after: bool,
) -> Vec<CollectionId> {
    let mut order: Vec<CollectionId> = siblings
        .iter()
        .copied()
        .filter(|id| *id != dragged)
        .collect();
    let index = order
        .iter()
        .position(|id| *id == target)
        .map_or(order.len(), |index| index + usize::from(after));
    order.insert(index, dragged);
    order
}

#[cfg(test)]
mod tests {
    use pigoune_core::{Collection, CollectionCommand, CollectionId};

    use super::{CollectionDrop, DropPlan, DropZone, plan, zone_at};
    use crate::collection_sort::{CollectionCriterion, CollectionOrder, CollectionTree};

    fn id(number: u8) -> CollectionId {
        CollectionId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("id")
    }

    fn collection(number: u8, name: &str, parent: Option<u8>) -> Collection {
        Collection {
            id: id(number),
            name: name.to_owned(),
            parent: parent.map(id),
            position: 0,
            created_at_unix_ms: i64::from(number),
        }
    }

    fn tree() -> CollectionTree {
        let collections = vec![
            collection(1, "Avion", None),
            collection(2, "Marques", None),
            collection(3, "Zèbre", None),
            collection(4, "Audio", Some(2)),
            collection(5, "Tech", Some(2)),
        ];
        let order = CollectionOrder {
            criterion: CollectionCriterion::Name,
            reversed: false,
        };
        CollectionTree::new(collections, order, |name: &str| name.to_lowercase())
    }

    #[test]
    fn the_edges_of_a_row_place_and_its_middle_nests() {
        assert_eq!(zone_at(2.0, 40.0), DropZone::Before);
        assert_eq!(zone_at(20.0, 40.0), DropZone::Into);
        assert_eq!(zone_at(38.0, 40.0), DropZone::After);
    }

    #[test]
    fn dropping_into_a_collection_moves_without_changing_the_sort() {
        assert_eq!(
            plan(&tree(), id(3), CollectionDrop::Into(Some(id(2))), false),
            Some(DropPlan {
                commands: vec![CollectionCommand::Move {
                    id: id(3),
                    parent: Some(id(2)),
                }],
                switches_to_custom_order: false,
            })
        );
        assert_eq!(
            plan(&tree(), id(5), CollectionDrop::Into(None), false).map(|plan| plan.commands),
            Some(vec![CollectionCommand::Move {
                id: id(5),
                parent: None,
            }])
        );
    }

    #[test]
    fn dropping_where_it_already_is_does_nothing() {
        assert_eq!(
            plan(&tree(), id(5), CollectionDrop::Into(Some(id(2))), true),
            None
        );
        assert_eq!(
            plan(&tree(), id(2), CollectionDrop::Into(Some(id(2))), true),
            None
        );
        assert_eq!(plan(&tree(), id(1), CollectionDrop::Into(None), true), None);
        assert_eq!(
            plan(&tree(), id(1), CollectionDrop::After(id(1)), true),
            None
        );
    }

    #[test]
    fn placing_between_siblings_rearranges_them() {
        assert_eq!(
            plan(&tree(), id(3), CollectionDrop::Before(id(1)), true),
            Some(DropPlan {
                commands: vec![CollectionCommand::Arrange {
                    parent: None,
                    order: vec![id(3), id(1), id(2)],
                }],
                switches_to_custom_order: false,
            })
        );
        assert_eq!(
            plan(&tree(), id(1), CollectionDrop::After(id(2)), true).map(|plan| plan.commands),
            Some(vec![CollectionCommand::Arrange {
                parent: None,
                order: vec![id(2), id(1), id(3)],
            }])
        );
    }

    #[test]
    fn placing_next_to_another_parent_moves_then_rearranges() {
        assert_eq!(
            plan(&tree(), id(1), CollectionDrop::After(id(4)), true).map(|plan| plan.commands),
            Some(vec![
                CollectionCommand::Move {
                    id: id(1),
                    parent: Some(id(2)),
                },
                CollectionCommand::Arrange {
                    parent: Some(id(2)),
                    order: vec![id(4), id(1), id(5)],
                },
            ])
        );
    }

    #[test]
    fn placing_by_hand_keeps_the_displayed_order_when_switching_to_custom() {
        let plan = plan(&tree(), id(3), CollectionDrop::Before(id(1)), false).expect("a plan");
        assert!(plan.switches_to_custom_order);
        assert_eq!(
            plan.commands,
            [
                CollectionCommand::Arrange {
                    parent: None,
                    order: vec![id(1), id(2), id(3)],
                },
                CollectionCommand::Arrange {
                    parent: Some(id(2)),
                    order: vec![id(4), id(5)],
                },
                CollectionCommand::Arrange {
                    parent: None,
                    order: vec![id(3), id(1), id(2)],
                },
            ]
        );
    }
}
