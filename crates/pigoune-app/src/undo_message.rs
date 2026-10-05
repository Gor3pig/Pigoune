use gettextrs::{gettext, ngettext};
use pigoune_core::{
    AssetCommand, AssetId, Change, CollectionCommand, CollectionId, Library,
    SmartCollectionCommand, SmartCollectionId, TagCommand, TagId, TextField,
};

pub fn describe(change: &Change, library: &Library) -> String {
    let names = Names(library);
    let described = match change {
        Change::Asset(command) => describe_asset(command, &names),
        Change::Collection(command) => describe_collection(command, &names),
        Change::Tag(command) => describe_tag(command, &names),
        Change::SmartCollection(command) => describe_smart_collection(command, &names),
    };
    described.unwrap_or_else(|| gettext("Last change undone"))
}

fn describe_asset(command: &AssetCommand, names: &Names) -> Option<String> {
    let text = match command {
        AssetCommand::SetFavorite { assets, favorite } => counted(
            assets.len(),
            &if *favorite {
                ngettext(
                    "Adding {count} resource to the favorites undone",
                    "Adding {count} resources to the favorites undone",
                    plural(assets.len()),
                )
            } else {
                ngettext(
                    "Removing {count} resource from the favorites undone",
                    "Removing {count} resources from the favorites undone",
                    plural(assets.len()),
                )
            },
        ),
        AssetCommand::SetTrashed { assets, trashed } => counted(
            assets.len(),
            &if *trashed {
                ngettext(
                    "Moving {count} resource to the trash undone",
                    "Moving {count} resources to the trash undone",
                    plural(assets.len()),
                )
            } else {
                ngettext(
                    "Restoring {count} resource undone",
                    "Restoring {count} resources undone",
                    plural(assets.len()),
                )
            },
        ),
        AssetCommand::Rename { asset, .. } => named(
            &gettext("Renaming undone, the resource is named “{name}” again"),
            &names.asset(*asset)?,
        ),
        AssetCommand::SetText { asset, field, .. } => {
            named(&text_change(*field), &names.asset(*asset)?)
        }
        AssetCommand::Batch(commands) => {
            return single(commands, |command| describe_asset(command, names));
        }
    };
    Some(text)
}

fn text_change(field: TextField) -> String {
    match field {
        TextField::Note => gettext("Note change on “{name}” undone"),
        TextField::SourceUrl => gettext("Source change on “{name}” undone"),
        TextField::License => gettext("License change on “{name}” undone"),
        TextField::Author => gettext("Author change on “{name}” undone"),
    }
}

fn describe_collection(command: &CollectionCommand, names: &Names) -> Option<String> {
    let text = match command {
        CollectionCommand::Create { collection } => named(
            &gettext("Creating the collection “{name}” undone"),
            &collection.name,
        ),
        CollectionCommand::Delete { .. } => return None,
        CollectionCommand::Rename { id, .. } => named(
            &gettext("Renaming undone, the collection is named “{name}” again"),
            &names.collection(*id)?,
        ),
        CollectionCommand::Trash { id } => named(
            &gettext("Deleting the collection “{name}” undone"),
            &names.collection(*id)?,
        ),
        CollectionCommand::AddAssets { collection, assets } => named(
            &counted(
                assets.len(),
                &ngettext(
                    "Adding {count} resource to “{name}” undone",
                    "Adding {count} resources to “{name}” undone",
                    plural(assets.len()),
                ),
            ),
            &names.collection(*collection)?,
        ),
        CollectionCommand::RemoveAssets { collection, assets } => named(
            &counted(
                assets.len(),
                &ngettext(
                    "Removing {count} resource from “{name}” undone",
                    "Removing {count} resources from “{name}” undone",
                    plural(assets.len()),
                ),
            ),
            &names.collection(*collection)?,
        ),
        CollectionCommand::MoveAssets { to, assets, .. } => named(
            &counted(
                assets.len(),
                &ngettext(
                    "Moving {count} resource to “{name}” undone",
                    "Moving {count} resources to “{name}” undone",
                    plural(assets.len()),
                ),
            ),
            &names.collection(*to)?,
        ),
        CollectionCommand::Move { .. }
        | CollectionCommand::Arrange { .. }
        | CollectionCommand::SetTrashed { .. }
        | CollectionCommand::Batch(_) => return describe_arrangement(command, names),
    };
    Some(text)
}

fn describe_arrangement(command: &CollectionCommand, names: &Names) -> Option<String> {
    match moved_collection(command) {
        Some(id) => Some(named(
            &gettext("Moving the collection “{name}” undone"),
            &names.collection(id)?,
        )),
        None if is_arrangement(command) => Some(gettext("Change of the collection order undone")),
        None => None,
    }
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

fn describe_tag(command: &TagCommand, names: &Names) -> Option<String> {
    let text = match command {
        TagCommand::Add { name, .. } => named(&gettext("Adding the tag “{name}” undone"), name),
        TagCommand::Link { tag, assets } => named(
            &counted(
                assets.len(),
                &ngettext(
                    "Adding the tag “{name}” to {count} resource undone",
                    "Adding the tag “{name}” to {count} resources undone",
                    plural(assets.len()),
                ),
            ),
            &names.tag(*tag)?,
        ),
        TagCommand::Unlink { tag, assets } => named(
            &counted(
                assets.len(),
                &ngettext(
                    "Removing the tag “{name}” from {count} resource undone",
                    "Removing the tag “{name}” from {count} resources undone",
                    plural(assets.len()),
                ),
            ),
            &names.tag(*tag)?,
        ),
        TagCommand::Rename { tag, .. } => named(
            &gettext("Renaming undone, the tag is named “{name}” again"),
            &names.tag(*tag)?,
        ),
        TagCommand::Merge { from, .. } => named(
            &gettext("Merging the tag “{name}” undone"),
            &names.tag(*from)?,
        ),
        TagCommand::Delete { tag } => named(
            &gettext("Deleting the tag “{name}” undone"),
            &names.tag(*tag)?,
        ),
        TagCommand::Recreate { .. } => return None,
        TagCommand::Batch(commands) => return describe_tag_batch(commands, names),
    };
    Some(text)
}

fn describe_smart_collection(command: &SmartCollectionCommand, names: &Names) -> Option<String> {
    let text = match command {
        SmartCollectionCommand::Create { collection } => named(
            &gettext("Creating the smart collection “{name}” undone"),
            &collection.name,
        ),
        SmartCollectionCommand::Update { collection } => named(
            &gettext("Changes to the smart collection “{name}” undone"),
            &names.smart_collection(collection.id)?,
        ),
        SmartCollectionCommand::Arrange { .. } => {
            gettext("Reordering the smart collections undone")
        }
        SmartCollectionCommand::Delete { id } => named(
            &gettext("Deleting the smart collection “{name}” undone"),
            &names.smart_collection(*id)?,
        ),
    };
    Some(text)
}

fn describe_tag_batch(commands: &[TagCommand], names: &Names) -> Option<String> {
    let added = commands
        .iter()
        .filter(|command| matches!(command, TagCommand::Add { .. }))
        .count();
    if added > 1 && added == commands.len() {
        return Some(counted(
            added,
            &ngettext(
                "Adding {count} tag undone",
                "Adding {count} tags undone",
                plural(added),
            ),
        ));
    }
    single(commands, |command| describe_tag(command, names))
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
