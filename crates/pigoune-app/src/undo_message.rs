use std::collections::HashSet;

use gettextrs::{gettext, ngettext};
use pigoune_core::{
    AssetCommand, AssetId, Change, CollectionCommand, CollectionId, Library,
    SmartCollectionCommand, SmartCollectionId, TagCommand, TagId, TextField,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Undone,
    Redone,
}

fn pick(
    outcome: Outcome,
    undone: impl FnOnce() -> String,
    redone: impl FnOnce() -> String,
) -> String {
    match outcome {
        Outcome::Undone => undone(),
        Outcome::Redone => redone(),
    }
}

pub fn describe(change: &Change, library: &Library, outcome: Outcome) -> String {
    let names = Names(library);
    let described = match change {
        Change::Asset(command) => describe_asset(command, &names, outcome),
        Change::Collection(command) => describe_collection(command, &names, outcome),
        Change::Tag(command) => describe_tag(command, &names, outcome),
        Change::SmartCollection(command) => describe_smart_collection(command, &names, outcome),
    };
    described.unwrap_or_else(|| {
        pick(
            outcome,
            || gettext("Last change undone"),
            || gettext("Last change redone"),
        )
    })
}

fn favorite_text(favorite: bool, count: usize, outcome: Outcome) -> String {
    if favorite {
        pick(
            outcome,
            || {
                ngettext(
                    "Adding {count} asset to the favorites undone",
                    "Adding {count} assets to the favorites undone",
                    plural(count),
                )
            },
            || {
                ngettext(
                    "Adding {count} asset to the favorites redone",
                    "Adding {count} assets to the favorites redone",
                    plural(count),
                )
            },
        )
    } else {
        pick(
            outcome,
            || {
                ngettext(
                    "Removing {count} asset from the favorites undone",
                    "Removing {count} assets from the favorites undone",
                    plural(count),
                )
            },
            || {
                ngettext(
                    "Removing {count} asset from the favorites redone",
                    "Removing {count} assets from the favorites redone",
                    plural(count),
                )
            },
        )
    }
}

fn trashed_text(trashed: bool, count: usize, outcome: Outcome) -> String {
    if trashed {
        pick(
            outcome,
            || {
                ngettext(
                    "Moving {count} asset to the trash undone",
                    "Moving {count} assets to the trash undone",
                    plural(count),
                )
            },
            || {
                ngettext(
                    "Moving {count} asset to the trash redone",
                    "Moving {count} assets to the trash redone",
                    plural(count),
                )
            },
        )
    } else {
        pick(
            outcome,
            || {
                ngettext(
                    "Restoring {count} asset undone",
                    "Restoring {count} assets undone",
                    plural(count),
                )
            },
            || {
                ngettext(
                    "Restoring {count} asset redone",
                    "Restoring {count} assets redone",
                    plural(count),
                )
            },
        )
    }
}

fn describe_asset(command: &AssetCommand, names: &Names, outcome: Outcome) -> Option<String> {
    let text = match command {
        AssetCommand::SetFavorite { assets, favorite } => counted(
            assets.len(),
            &favorite_text(*favorite, assets.len(), outcome),
        ),
        AssetCommand::SetTrashed { assets, trashed } => {
            counted(assets.len(), &trashed_text(*trashed, assets.len(), outcome))
        }
        AssetCommand::Rename { asset, .. } => named(
            &pick(
                outcome,
                || gettext("Renaming undone, the asset is named “{name}” again"),
                || gettext("Renaming redone, the asset is named “{name}”"),
            ),
            &names.asset(*asset)?,
        ),
        AssetCommand::SetText { asset, field, .. } => {
            named(&text_change(*field, outcome), &names.asset(*asset)?)
        }
        AssetCommand::Batch(commands) => {
            return single(commands, |command| describe_asset(command, names, outcome));
        }
    };
    Some(text)
}

fn text_change(field: TextField, outcome: Outcome) -> String {
    match field {
        TextField::Note => pick(
            outcome,
            || gettext("Note change on “{name}” undone"),
            || gettext("Note change on “{name}” redone"),
        ),
        TextField::SourceUrl => pick(
            outcome,
            || gettext("Source change on “{name}” undone"),
            || gettext("Source change on “{name}” redone"),
        ),
        TextField::License => pick(
            outcome,
            || gettext("License change on “{name}” undone"),
            || gettext("License change on “{name}” redone"),
        ),
        TextField::Author => pick(
            outcome,
            || gettext("Author change on “{name}” undone"),
            || gettext("Author change on “{name}” redone"),
        ),
    }
}

fn describe_collection(
    command: &CollectionCommand,
    names: &Names,
    outcome: Outcome,
) -> Option<String> {
    let text = match command {
        CollectionCommand::Create { collection } => named(
            &pick(
                outcome,
                || gettext("Creating the collection “{name}” undone"),
                || gettext("Creating the collection “{name}” redone"),
            ),
            &collection.name,
        ),
        CollectionCommand::Delete { .. } => return None,
        CollectionCommand::Restyle { id, .. } => named(
            &pick(
                outcome,
                || gettext("Customizing the collection “{name}” undone"),
                || gettext("Customizing the collection “{name}” redone"),
            ),
            &names.collection(*id)?,
        ),
        CollectionCommand::Rename { id, .. } => named(
            &pick(
                outcome,
                || gettext("Renaming undone, the collection is named “{name}” again"),
                || gettext("Renaming redone, the collection is named “{name}”"),
            ),
            &names.collection(*id)?,
        ),
        CollectionCommand::Trash { id } => named(
            &pick(
                outcome,
                || gettext("Deleting the collection “{name}” undone"),
                || gettext("Deleting the collection “{name}” redone"),
            ),
            &names.collection(*id)?,
        ),
        CollectionCommand::AddAssets { collection, assets } => {
            let count = assets.len();
            named(
                &counted(
                    count,
                    &pick(
                        outcome,
                        || {
                            ngettext(
                                "Adding {count} asset to “{name}” undone",
                                "Adding {count} assets to “{name}” undone",
                                plural(count),
                            )
                        },
                        || {
                            ngettext(
                                "Adding {count} asset to “{name}” redone",
                                "Adding {count} assets to “{name}” redone",
                                plural(count),
                            )
                        },
                    ),
                ),
                &names.collection(*collection)?,
            )
        }
        CollectionCommand::RemoveAssets { collection, assets } => {
            let count = assets.len();
            named(
                &counted(
                    count,
                    &pick(
                        outcome,
                        || {
                            ngettext(
                                "Removing {count} asset from “{name}” undone",
                                "Removing {count} assets from “{name}” undone",
                                plural(count),
                            )
                        },
                        || {
                            ngettext(
                                "Removing {count} asset from “{name}” redone",
                                "Removing {count} assets from “{name}” redone",
                                plural(count),
                            )
                        },
                    ),
                ),
                &names.collection(*collection)?,
            )
        }
        CollectionCommand::MoveAssets { to, assets, .. } => {
            named(&moved_text(assets.len(), outcome), &names.collection(*to)?)
        }
        CollectionCommand::Move { .. }
        | CollectionCommand::Arrange { .. }
        | CollectionCommand::SetTrashed { .. }
        | CollectionCommand::Batch(_) => return describe_arrangement(command, names, outcome),
    };
    Some(text)
}

fn moved_text(count: usize, outcome: Outcome) -> String {
    counted(
        count,
        &pick(
            outcome,
            || {
                ngettext(
                    "Moving {count} asset to “{name}” undone",
                    "Moving {count} assets to “{name}” undone",
                    plural(count),
                )
            },
            || {
                ngettext(
                    "Moving {count} asset to “{name}” redone",
                    "Moving {count} assets to “{name}” redone",
                    plural(count),
                )
            },
        ),
    )
}

fn describe_arrangement(
    command: &CollectionCommand,
    names: &Names,
    outcome: Outcome,
) -> Option<String> {
    if let Some(count) = unclassified_count(command) {
        return Some(counted(
            count,
            &pick(
                outcome,
                || {
                    ngettext(
                        "Removing {count} asset from all its collections undone",
                        "Removing {count} assets from all their collections undone",
                        plural(count),
                    )
                },
                || {
                    ngettext(
                        "Removing {count} asset from all its collections redone",
                        "Removing {count} assets from all their collections redone",
                        plural(count),
                    )
                },
            ),
        ));
    }
    if let Some((count, target)) = moved_into(command) {
        return Some(named(
            &moved_text(count, outcome),
            &names.collection(target)?,
        ));
    }
    match moved_collection(command) {
        Some(id) => Some(named(
            &pick(
                outcome,
                || gettext("Moving the collection “{name}” undone"),
                || gettext("Moving the collection “{name}” redone"),
            ),
            &names.collection(id)?,
        )),
        None if is_arrangement(command) => Some(pick(
            outcome,
            || gettext("Change of the collection order undone"),
            || gettext("Change of the collection order redone"),
        )),
        None => None,
    }
}

fn unclassified_count(command: &CollectionCommand) -> Option<usize> {
    let CollectionCommand::Batch(commands) = command else {
        return None;
    };
    let mut distinct = HashSet::new();
    for command in commands {
        let CollectionCommand::RemoveAssets { assets, .. } = command else {
            return None;
        };
        distinct.extend(assets.iter().copied());
    }
    (!distinct.is_empty()).then_some(distinct.len())
}

fn moved_into(command: &CollectionCommand) -> Option<(usize, CollectionId)> {
    let CollectionCommand::Batch(commands) = command else {
        return None;
    };
    let mut distinct: HashSet<AssetId> = HashSet::new();
    let mut target = None;
    for command in commands {
        match command {
            CollectionCommand::RemoveAssets { assets, .. } => {
                distinct.extend(assets.iter().copied());
            }
            CollectionCommand::AddAssets { collection, assets } if target.is_none() => {
                target = Some(*collection);
                distinct.extend(assets.iter().copied());
            }
            _ => return None,
        }
    }
    Some((distinct.len(), target?))
}

fn moved_collection(command: &CollectionCommand) -> Option<CollectionId> {
    match command {
        CollectionCommand::Move { id, .. } => Some(*id),
        CollectionCommand::Batch(commands) => commands.iter().find_map(moved_collection),
        _ => None,
    }
}

fn is_arrangement(command: &CollectionCommand) -> bool {
    match command {
        CollectionCommand::Arrange { .. } => true,
        CollectionCommand::Batch(commands) => {
            !commands.is_empty() && commands.iter().all(is_arrangement)
        }
        _ => false,
    }
}

fn describe_tag(command: &TagCommand, names: &Names, outcome: Outcome) -> Option<String> {
    let text = match command {
        TagCommand::Add { name, .. } => named(
            &pick(
                outcome,
                || gettext("Adding the tag “{name}” undone"),
                || gettext("Adding the tag “{name}” redone"),
            ),
            name,
        ),
        TagCommand::Link { tag, assets } => {
            let count = assets.len();
            named(
                &counted(
                    count,
                    &pick(
                        outcome,
                        || {
                            ngettext(
                                "Adding the tag “{name}” to {count} asset undone",
                                "Adding the tag “{name}” to {count} assets undone",
                                plural(count),
                            )
                        },
                        || {
                            ngettext(
                                "Adding the tag “{name}” to {count} asset redone",
                                "Adding the tag “{name}” to {count} assets redone",
                                plural(count),
                            )
                        },
                    ),
                ),
                &names.tag(*tag)?,
            )
        }
        TagCommand::Unlink { tag, assets } => {
            let count = assets.len();
            named(
                &counted(
                    count,
                    &pick(
                        outcome,
                        || {
                            ngettext(
                                "Removing the tag “{name}” from {count} asset undone",
                                "Removing the tag “{name}” from {count} assets undone",
                                plural(count),
                            )
                        },
                        || {
                            ngettext(
                                "Removing the tag “{name}” from {count} asset redone",
                                "Removing the tag “{name}” from {count} assets redone",
                                plural(count),
                            )
                        },
                    ),
                ),
                &names.tag(*tag)?,
            )
        }
        TagCommand::Rename { tag, .. } => named(
            &pick(
                outcome,
                || gettext("Renaming undone, the tag is named “{name}” again"),
                || gettext("Renaming redone, the tag is named “{name}”"),
            ),
            &names.tag(*tag)?,
        ),
        TagCommand::Merge { from, .. } => named(
            &pick(
                outcome,
                || gettext("Merging the tag “{name}” undone"),
                || gettext("Merging the tag “{name}” redone"),
            ),
            &names.tag(*from)?,
        ),
        TagCommand::Delete { tag } | TagCommand::Dissolve { tag } => named(
            &pick(
                outcome,
                || gettext("Deleting the tag “{name}” undone"),
                || gettext("Deleting the tag “{name}” redone"),
            ),
            &names.tag(*tag)?,
        ),
        TagCommand::Move { tag, .. } => named(
            &pick(
                outcome,
                || gettext("Moving the tag “{name}” undone"),
                || gettext("Moving the tag “{name}” redone"),
            ),
            &names.tag(*tag)?,
        ),
        TagCommand::Recreate { .. } => return None,
        TagCommand::Batch(commands) => return describe_tag_batch(commands, names, outcome),
    };
    Some(text)
}

fn describe_smart_collection(
    command: &SmartCollectionCommand,
    names: &Names,
    outcome: Outcome,
) -> Option<String> {
    let text = match command {
        SmartCollectionCommand::Create { collection } => named(
            &pick(
                outcome,
                || gettext("Creating the smart collection “{name}” undone"),
                || gettext("Creating the smart collection “{name}” redone"),
            ),
            &collection.name,
        ),
        SmartCollectionCommand::Update { collection } => named(
            &pick(
                outcome,
                || gettext("Changes to the smart collection “{name}” undone"),
                || gettext("Changes to the smart collection “{name}” redone"),
            ),
            &names.smart_collection(collection.id)?,
        ),
        SmartCollectionCommand::Arrange { .. } => pick(
            outcome,
            || gettext("Reordering the smart collections undone"),
            || gettext("Reordering the smart collections redone"),
        ),
        SmartCollectionCommand::Delete { id } => named(
            &pick(
                outcome,
                || gettext("Deleting the smart collection “{name}” undone"),
                || gettext("Deleting the smart collection “{name}” redone"),
            ),
            &names.smart_collection(*id)?,
        ),
    };
    Some(text)
}

fn describe_tag_batch(commands: &[TagCommand], names: &Names, outcome: Outcome) -> Option<String> {
    let added = commands
        .iter()
        .filter(|command| matches!(command, TagCommand::Add { .. }))
        .count();
    if added > 1 && added == commands.len() {
        return Some(counted(
            added,
            &pick(
                outcome,
                || {
                    ngettext(
                        "Adding {count} tag undone",
                        "Adding {count} tags undone",
                        plural(added),
                    )
                },
                || {
                    ngettext(
                        "Adding {count} tag redone",
                        "Adding {count} tags redone",
                        plural(added),
                    )
                },
            ),
        ));
    }
    single(commands, |command| describe_tag(command, names, outcome))
}

fn single<T>(commands: &[T], describe: impl Fn(&T) -> Option<String>) -> Option<String> {
    match commands {
        [only] => describe(only),
        _ => None,
    }
}

fn counted(count: usize, text: &str) -> String {
    text.replace("{count}", &count.to_string())
}

fn named(text: &str, name: &str) -> String {
    text.replace("{name}", name)
}

fn plural(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

struct Names<'a>(&'a Library);

impl Names<'_> {
    fn asset(&self, id: AssetId) -> Option<String> {
        Some(self.0.asset(id).ok()??.display_name)
    }

    fn collection(&self, id: CollectionId) -> Option<String> {
        Some(self.0.collection(id).ok()??.name)
    }

    fn smart_collection(&self, id: SmartCollectionId) -> Option<String> {
        Some(self.0.smart_collection(id).ok()??.name)
    }

    fn tag(&self, id: TagId) -> Option<String> {
        self.0
            .tags()
            .ok()?
            .into_iter()
            .find(|tag| tag.id == id)
            .map(|tag| tag.name)
    }
}
