use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, params};

use super::collection::{
    self, Collection, CollectionLook, ensure_name_is_free, find_child_named, is_usable_collection,
    valid_name,
};
use super::{AssetId, Change, CollectionError, CollectionId, Library, LibraryError, clock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionCommand {
    Create {
        collection: Collection,
    },
    Delete {
        id: CollectionId,
    },
    Rename {
        id: CollectionId,
        name: String,
    },
    Restyle {
        id: CollectionId,
        look: CollectionLook,
    },
    SkipDuplicates {
        id: CollectionId,
        skip: bool,
    },
    Move {
        id: CollectionId,
        parent: Option<CollectionId>,
    },
    Arrange {
        parent: Option<CollectionId>,
        order: Vec<CollectionId>,
    },
    Trash {
        id: CollectionId,
    },
    SetTrashed {
        collections: Vec<CollectionId>,
        assets: Vec<AssetId>,
        trashed: bool,
    },
    AddAssets {
        collection: CollectionId,
        assets: Vec<AssetId>,
    },
    RemoveAssets {
        collection: CollectionId,
        assets: Vec<AssetId>,
    },
    MoveAssets {
        from: CollectionId,
        to: CollectionId,
        assets: Vec<AssetId>,
    },
    Batch(Vec<CollectionCommand>),
}

impl CollectionCommand {
    #[must_use]
    pub fn affected_assets(&self) -> usize {
        let mut affected = HashSet::new();
        self.collect_assets(&mut affected);
        affected.len()
    }

    fn collect_assets(&self, affected: &mut HashSet<AssetId>) {
        match self {
            Self::AddAssets { assets, .. }
            | Self::RemoveAssets { assets, .. }
            | Self::MoveAssets { assets, .. } => affected.extend(assets.iter().copied()),
            Self::Batch(commands) => {
                for command in commands {
                    command.collect_assets(affected);
                }
            }
            _ => {}
        }
    }

    pub fn removes_assets(&self) -> bool {
        match self {
            Self::RemoveAssets { .. } | Self::MoveAssets { .. } => true,
            Self::Batch(commands) => commands.iter().any(Self::removes_assets),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionRemoval {
    pub sub_collections: usize,
    pub trashed_assets: usize,
}

impl Library {
    pub fn removal_of(&self, id: CollectionId) -> Result<CollectionRemoval, CollectionError> {
        usable(&self.connection, id)?;
        let collections = subtree(&self.connection, id)?;
        let trashed_assets = assets_only_in(&self.connection, &collections)?.len();
        Ok(CollectionRemoval {
            sub_collections: collections.len().saturating_sub(1),
            trashed_assets,
        })
    }

    pub fn unclassify_command(
        &self,
        assets: &[AssetId],
    ) -> Result<Option<CollectionCommand>, LibraryError> {
        let held = self.collections_held_by(assets)?;
        let removals = removals_outside(assets, &held, None);
        if removals.is_empty() {
            return Ok(None);
        }
        Ok(Some(CollectionCommand::Batch(removals)))
    }

    pub fn move_to_collection_command(
        &self,
        assets: &[AssetId],
        target: CollectionId,
    ) -> Result<Option<CollectionCommand>, LibraryError> {
        let held = self.collections_held_by(assets)?;
        let mut commands = removals_outside(assets, &held, Some(target));
        let mut newcomers = Vec::new();
        for asset in assets {
            if !held
                .get(asset)
                .is_some_and(|collections| collections.contains(&target))
            {
                newcomers.push(*asset);
            }
        }
        if commands.is_empty() && newcomers.is_empty() {
            return Ok(None);
        }
        commands.push(CollectionCommand::AddAssets {
            collection: target,
            assets: newcomers,
        });
        Ok(Some(CollectionCommand::Batch(commands)))
    }

    pub fn apply_collection_command(
        &mut self,
        command: &CollectionCommand,
    ) -> Result<CollectionCommand, CollectionError> {
        let inverse = self.run_collection_command(command)?;
        self.history.record(
            Change::Collection(command.clone()),
            Change::Collection(inverse.clone()),
        );
        Ok(inverse)
    }

    pub(super) fn run_collection_command(
        &mut self,
        command: &CollectionCommand,
    ) -> Result<CollectionCommand, CollectionError> {
        if changes_only_memberships(command) {
            self.keeping_queries_cached(|library| library.write_collection_command(command))
        } else {
            self.write_collection_command(command)
        }
    }

    fn write_collection_command(
        &mut self,
        command: &CollectionCommand,
    ) -> Result<CollectionCommand, CollectionError> {
        let transaction = self.connection.transaction()?;
        let inverse = apply(&transaction, command)?;
        transaction.commit()?;
        Ok(inverse)
    }

    pub fn sibling_order(
        &self,
        parent: Option<CollectionId>,
    ) -> Result<Vec<CollectionId>, LibraryError> {
        current_order(&self.connection, parent)
    }
}

fn apply(
    connection: &Connection,
    command: &CollectionCommand,
) -> Result<CollectionCommand, CollectionError> {
    match command {
        CollectionCommand::Create { collection } => create(connection, collection),
        CollectionCommand::Delete { id } => delete(connection, *id),
        CollectionCommand::Rename { id, name } => rename(connection, *id, name),
        CollectionCommand::Restyle { id, look } => restyle(connection, *id, look),
        CollectionCommand::SkipDuplicates { id, skip } => skip_duplicates(connection, *id, *skip),
        CollectionCommand::Move { id, parent } => move_into(connection, *id, *parent),
        CollectionCommand::Arrange { parent, order } => arrange(connection, *parent, order),
        CollectionCommand::Trash { id } => trash(connection, *id),
        CollectionCommand::SetTrashed {
            collections,
            assets,
            trashed,
        } => set_trashed(connection, collections, assets, *trashed),
        CollectionCommand::AddAssets { collection, assets } => {
            add_assets(connection, *collection, assets)
        }
        CollectionCommand::RemoveAssets { collection, assets } => {
            remove_assets(connection, *collection, assets)
        }
        CollectionCommand::MoveAssets { from, to, assets } => {
            move_assets(connection, *from, *to, assets)
        }
        CollectionCommand::Batch(commands) => {
            let mut inverses = commands
                .iter()
                .map(|command| apply(connection, command))
                .collect::<Result<Vec<_>, _>>()?;
            inverses.reverse();
            Ok(CollectionCommand::Batch(inverses))
        }
    }
}

fn create(
    connection: &Connection,
    collection: &Collection,
) -> Result<CollectionCommand, CollectionError> {
    let name = valid_name(&collection.name)?;
    if let Some(parent) = collection.parent
        && !is_usable_collection(connection, parent)?
    {
        return Err(CollectionError::NotFound(parent));
    }
    ensure_name_is_free(connection, collection.parent, name, None)?;
    collection::insert(
        connection,
        &Collection {
            name: name.to_owned(),
            ..collection.clone()
        },
    )?;
    Ok(CollectionCommand::Delete { id: collection.id })
}

fn delete(connection: &Connection, id: CollectionId) -> Result<CollectionCommand, CollectionError> {
    usable(connection, id)?;
    let existing = connection.query_row(
        &format!(
            "SELECT {} FROM collections WHERE id = ?1",
            collection::COLLECTION_COLUMNS
        ),
        [id],
        collection::collection_from_row,
    )?;
    if holds_anything(connection, id)? {
        return Err(CollectionError::NotEmpty(existing.name));
    }
    connection.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    Ok(CollectionCommand::Create {
        collection: existing,
    })
}

fn holds_anything(connection: &Connection, id: CollectionId) -> Result<bool, LibraryError> {
    Ok(connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM asset_collections WHERE collection_id = ?1)
             OR EXISTS (SELECT 1 FROM collections WHERE parent_id = ?1)",
        [id],
        |row| row.get(0),
    )?)
}

fn rename(
    connection: &Connection,
    id: CollectionId,
    name: &str,
) -> Result<CollectionCommand, CollectionError> {
    let (old_name, parent) = usable(connection, id)?;
    let name = valid_name(name)?;
    ensure_name_is_free(connection, parent, name, Some(id))?;
    connection.execute(
        "UPDATE collections SET name = ?2 WHERE id = ?1",
        params![id, name],
    )?;
    Ok(CollectionCommand::Rename { id, name: old_name })
}

fn restyle(
    connection: &Connection,
    id: CollectionId,
    look: &CollectionLook,
) -> Result<CollectionCommand, CollectionError> {
    usable(connection, id)?;
    if !look.is_valid() {
        return Err(CollectionError::InvalidLook);
    }
    let previous = connection.query_row(
        "SELECT icon, color FROM collections WHERE id = ?1",
        [id],
        |row| {
            Ok(CollectionLook {
                icon: row.get(0)?,
                color: row.get(1)?,
            })
        },
    )?;
    connection.execute(
        "UPDATE collections SET icon = ?2, color = ?3 WHERE id = ?1",
        params![id, look.icon, look.color],
    )?;
    Ok(CollectionCommand::Restyle { id, look: previous })
}

fn skip_duplicates(
    connection: &Connection,
    id: CollectionId,
    skip: bool,
) -> Result<CollectionCommand, CollectionError> {
    usable(connection, id)?;
    let previous: bool = connection.query_row(
        "SELECT skip_duplicates FROM collections WHERE id = ?1",
        [id],
        |row| row.get(0),
    )?;
    connection.execute(
        "UPDATE collections SET skip_duplicates = ?2 WHERE id = ?1",
        params![id, skip],
    )?;
    Ok(CollectionCommand::SkipDuplicates { id, skip: previous })
}

fn move_into(
    connection: &Connection,
    id: CollectionId,
    parent: Option<CollectionId>,
) -> Result<CollectionCommand, CollectionError> {
    let (name, old_parent) = usable(connection, id)?;
    if let Some(parent) = parent {
        if !is_usable_collection(connection, parent)? {
            return Err(CollectionError::NotFound(parent));
        }
        if subtree(connection, id)?.contains(&parent) {
            return Err(CollectionError::WouldContainItself);
        }
    }
    ensure_name_is_free(connection, parent, &name, Some(id))?;
    let old_order = current_order(connection, old_parent)?;
    let position = collection::next_position(connection, parent)?;
    connection.execute(
        "UPDATE collections SET parent_id = ?2, position = ?3 WHERE id = ?1",
        params![id, parent, position],
    )?;
    Ok(CollectionCommand::Batch(vec![
        CollectionCommand::Move {
            id,
            parent: old_parent,
        },
        CollectionCommand::Arrange {
            parent: old_parent,
            order: old_order,
        },
    ]))
}

fn arrange(
    connection: &Connection,
    parent: Option<CollectionId>,
    order: &[CollectionId],
) -> Result<CollectionCommand, CollectionError> {
    let previous = current_order(connection, parent)?;
    let wanted: HashSet<_> = order.iter().collect();
    let present: HashSet<_> = previous.iter().collect();
    if wanted != present || order.len() != previous.len() {
        return Err(CollectionError::OutdatedOrder);
    }
    for (position, id) in (0_i64..).zip(order) {
        connection.execute(
            "UPDATE collections SET position = ?2 WHERE id = ?1",
            params![id, position],
        )?;
    }
    Ok(CollectionCommand::Arrange {
        parent,
        order: previous,
    })
}

fn trash(connection: &Connection, id: CollectionId) -> Result<CollectionCommand, CollectionError> {
    usable(connection, id)?;
    let collections = subtree(connection, id)?;
    let assets = assets_only_in(connection, &collections)?;
    set_trashed(connection, &collections, &assets, true)
}

fn set_trashed(
    connection: &Connection,
    collections: &[CollectionId],
    assets: &[AssetId],
    trashed: bool,
) -> Result<CollectionCommand, CollectionError> {
    let moment = trashed.then(clock::now_unix_ms);
    for id in collections {
        if !trashed {
            free_the_name_of(connection, *id)?;
        }
        connection.execute(
            "UPDATE collections SET trashed_at_unix_ms = ?2 WHERE id = ?1",
            params![id, moment],
        )?;
    }
    for id in assets {
        connection.execute(
            "UPDATE assets SET trashed_at_unix_ms = ?2 WHERE id = ?1",
            params![id, moment],
        )?;
    }
    Ok(CollectionCommand::SetTrashed {
        collections: collections.to_vec(),
        assets: assets.to_vec(),
        trashed: !trashed,
    })
}

fn free_the_name_of(connection: &Connection, id: CollectionId) -> Result<(), CollectionError> {
    let (name, parent): (String, Option<CollectionId>) = connection.query_row(
        "SELECT name, parent_id FROM collections WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if find_child_named(connection, parent, &name)?.is_none() {
        return Ok(());
    }
    let mut number = 2_u32;
    let free = loop {
        let candidate = format!("{name} ({number})");
        if find_child_named(connection, parent, &candidate)?.is_none() {
            break candidate;
        }
        number += 1;
    };
    connection.execute(
        "UPDATE collections SET name = ?2 WHERE id = ?1",
        params![id, free],
    )?;
    Ok(())
}

fn add_assets(
    connection: &Connection,
    collection: CollectionId,
    assets: &[AssetId],
) -> Result<CollectionCommand, CollectionError> {
    usable(connection, collection)?;
    let mut added = Vec::new();
    for asset in assets {
        ensure_asset_is_visible(connection, *asset)?;
        let changed = connection.execute(
            "INSERT OR IGNORE INTO asset_collections (asset_id, collection_id) VALUES (?1, ?2)",
            params![asset, collection],
        )?;
        if changed > 0 {
            added.push(*asset);
        }
    }
    Ok(CollectionCommand::RemoveAssets {
        collection,
        assets: added,
    })
}

fn remove_assets(
    connection: &Connection,
    collection: CollectionId,
    assets: &[AssetId],
) -> Result<CollectionCommand, CollectionError> {
    usable(connection, collection)?;
    let mut removed = Vec::new();
    for asset in assets {
        let changed = connection.execute(
            "DELETE FROM asset_collections WHERE asset_id = ?1 AND collection_id = ?2",
            params![asset, collection],
        )?;
        if changed > 0 {
            removed.push(*asset);
        }
    }
    Ok(CollectionCommand::AddAssets {
        collection,
        assets: removed,
    })
}

fn move_assets(
    connection: &Connection,
    from: CollectionId,
    to: CollectionId,
    assets: &[AssetId],
) -> Result<CollectionCommand, CollectionError> {
    usable(connection, from)?;
    usable(connection, to)?;
    let mut inverses = Vec::new();
    for collection in subtree(connection, from)? {
        inverses.push(remove_assets(connection, collection, assets)?);
    }
    inverses.push(add_assets(connection, to, assets)?);
    inverses.reverse();
    Ok(CollectionCommand::Batch(inverses))
}

fn ensure_asset_is_visible(connection: &Connection, asset: AssetId) -> Result<(), CollectionError> {
    connection
        .query_row(
            "SELECT 1 FROM assets WHERE id = ?1 AND trashed_at_unix_ms IS NULL",
            [asset],
            |_| Ok(()),
        )
        .optional()?
        .ok_or(CollectionError::AssetNotFound(asset))
}

fn usable(
    connection: &Connection,
    id: CollectionId,
) -> Result<(String, Option<CollectionId>), CollectionError> {
    if !is_usable_collection(connection, id)? {
        return Err(CollectionError::NotFound(id));
    }
    Ok(connection.query_row(
        "SELECT name, parent_id FROM collections WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?)
}

fn current_order(
    connection: &Connection,
    parent: Option<CollectionId>,
) -> Result<Vec<CollectionId>, LibraryError> {
    let mut statement = connection.prepare(
        "SELECT id FROM collections
         WHERE parent_id IS ?1 AND trashed_at_unix_ms IS NULL
         ORDER BY position, created_at_unix_ms, id",
    )?;
    let order = statement
        .query_map([parent], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(order)
}

fn subtree(connection: &Connection, root: CollectionId) -> Result<Vec<CollectionId>, LibraryError> {
    let mut statement = connection.prepare(
        "WITH RECURSIVE subtree(id) AS (
             SELECT ?1
             UNION
             SELECT child.id FROM collections child
             JOIN subtree ON child.parent_id = subtree.id
             WHERE child.trashed_at_unix_ms IS NULL
         )
         SELECT id FROM subtree",
    )?;
    let ids = statement
        .query_map([root], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

fn changes_only_memberships(command: &CollectionCommand) -> bool {
    match command {
        CollectionCommand::AddAssets { .. }
        | CollectionCommand::RemoveAssets { .. }
        | CollectionCommand::MoveAssets { .. } => true,
        CollectionCommand::Batch(commands) => commands.iter().all(changes_only_memberships),
        CollectionCommand::Create { .. }
        | CollectionCommand::Delete { .. }
        | CollectionCommand::Rename { .. }
        | CollectionCommand::Restyle { .. }
        | CollectionCommand::SkipDuplicates { .. }
        | CollectionCommand::Move { .. }
        | CollectionCommand::Arrange { .. }
        | CollectionCommand::Trash { .. }
        | CollectionCommand::SetTrashed { .. } => false,
    }
}

fn removals_outside(
    assets: &[AssetId],
    held: &HashMap<AssetId, Vec<CollectionId>>,
    kept: Option<CollectionId>,
) -> Vec<CollectionCommand> {
    let mut by_collection: BTreeMap<CollectionId, Vec<AssetId>> = BTreeMap::new();
    for asset in assets {
        for collection in held.get(asset).into_iter().flatten() {
            if Some(*collection) != kept {
                by_collection.entry(*collection).or_default().push(*asset);
            }
        }
    }
    by_collection
        .into_iter()
        .map(|(collection, assets)| CollectionCommand::RemoveAssets { collection, assets })
        .collect()
}

fn assets_only_in(
    connection: &Connection,
    collections: &[CollectionId],
) -> Result<Vec<AssetId>, LibraryError> {
    let inside: HashSet<CollectionId> = collections.iter().copied().collect();
    let mut linked = connection.prepare(
        "SELECT asset_collections.asset_id FROM asset_collections
         JOIN assets ON assets.id = asset_collections.asset_id
         WHERE asset_collections.collection_id = ?1 AND assets.trashed_at_unix_ms IS NULL",
    )?;
    let mut elsewhere = connection.prepare(
        "SELECT asset_collections.collection_id FROM asset_collections
         JOIN collections ON collections.id = asset_collections.collection_id
         WHERE asset_collections.asset_id = ?1 AND collections.trashed_at_unix_ms IS NULL",
    )?;
    let mut trashed = Vec::new();
    let mut seen = HashSet::new();
    for collection in collections {
        let assets: Vec<AssetId> = linked
            .query_map([collection], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        for asset in assets {
            if !seen.insert(asset) {
                continue;
            }
            let homes: Vec<CollectionId> = elsewhere
                .query_map([asset], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            if homes.iter().all(|home| inside.contains(home)) {
                trashed.push(asset);
            }
        }
    }
    Ok(trashed)
}

#[cfg(test)]
mod cache_tests {
    use std::path::Path;
    use std::sync::Arc;

    use super::{CollectionCommand, CollectionError};
    use crate::library::{
        Asset, AssetCommand, AssetId, AssetView, Change, CollectionId, ImportOutcome, Library,
        TagCommand,
    };

    type Step = Box<dyn Fn(&mut Library)>;

    struct Fixture {
        _workspace: tempfile::TempDir,
        library: Library,
        assets: Vec<AssetId>,
        first: CollectionId,
        second: CollectionId,
        child: CollectionId,
    }

    fn fixture() -> Fixture {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
        let mut assets = Vec::new();
        for name in ["red-dot.png", "dark-circle.svg", "blue-photo.jpg"] {
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name);
            match library.import_file(&source, None).expect("import succeeds") {
                ImportOutcome::Imported(id) => assets.push(id),
                outcome => panic!("unexpected outcome {outcome:?}"),
            }
        }
        let first = library
            .create_collection("First", None)
            .expect("collection");
        let second = library
            .create_collection("Second", None)
            .expect("collection");
        let child = library
            .create_collection("Child", Some(first))
            .expect("collection");
        library
            .apply_collection_command(&CollectionCommand::AddAssets {
                collection: first,
                assets: vec![assets[0]],
            })
            .expect("an asset is filed");
        library
            .apply_tag_command(&TagCommand::Add {
                assets: assets.clone(),
                name: "Brands/Logo".to_owned(),
            })
            .expect("assets are tagged");
        Fixture {
            _workspace: workspace,
            library,
            assets,
            first,
            second,
            child,
        }
    }

    fn warm(library: &Library) -> Arc<Vec<Asset>> {
        let all = library.shared_assets(AssetView::All).expect("listing");
        library.shared_assets(AssetView::Trash).expect("trash");
        library.tag_names_by_asset().expect("tag names");
        all
    }

    fn still_cached(library: &Library, all: &Arc<Vec<Asset>>) -> bool {
        Arc::ptr_eq(
            &library.shared_assets(AssetView::All).expect("listing"),
            all,
        )
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("folder is created");
        for entry in std::fs::read_dir(from).expect("folder is read") {
            let entry = entry.expect("entry is read");
            let target = to.join(entry.file_name());
            if entry.file_type().expect("type is read").is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).expect("file is copied");
            }
        }
    }

    fn reopened_listing(library: &Library) -> Vec<Asset> {
        let copy = tempfile::tempdir().expect("temporary directory");
        let root = copy.path().join("copy.pigoune");
        copy_tree(library.root(), &root);
        Library::open(&root)
            .expect("the copy opens")
            .visible_assets_in(AssetView::All)
            .expect("listing")
    }

    #[test]
    fn adding_removing_and_moving_assets_keep_a_valid_cache() {
        let mut fixture = fixture();
        let [first_asset, second_asset, third_asset] = fixture.assets[..] else {
            panic!("three assets");
        };
        let all = warm(&fixture.library);
        let tags = fixture.library.tag_names_by_asset().expect("tag names");
        let commands = [
            CollectionCommand::AddAssets {
                collection: fixture.second,
                assets: vec![first_asset, second_asset, second_asset],
            },
            CollectionCommand::AddAssets {
                collection: fixture.second,
                assets: vec![first_asset],
            },
            CollectionCommand::AddAssets {
                collection: fixture.second,
                assets: Vec::new(),
            },
            CollectionCommand::MoveAssets {
                from: fixture.second,
                to: fixture.child,
                assets: vec![first_asset, third_asset],
            },
            CollectionCommand::RemoveAssets {
                collection: fixture.child,
                assets: vec![first_asset],
            },
            CollectionCommand::Batch(vec![
                CollectionCommand::AddAssets {
                    collection: fixture.first,
                    assets: vec![second_asset],
                },
                CollectionCommand::RemoveAssets {
                    collection: fixture.first,
                    assets: vec![first_asset],
                },
            ]),
        ];

        let mut recorded = 0;
        for command in &commands {
            let before = fixture.library.latest_change();
            fixture
                .library
                .apply_collection_command(command)
                .expect("the command applies");
            recorded += usize::from(fixture.library.latest_change() != before);
            assert!(
                still_cached(&fixture.library, &all),
                "{command:?} kept the cache"
            );
        }
        assert!(recorded >= 4, "most commands are recorded");
        for round in 0..recorded {
            let undone = fixture.library.undo().expect("undo runs");
            assert!(
                matches!(undone, Some(Change::Collection(_))),
                "undo {round} undoes a collection change"
            );
            assert!(still_cached(&fixture.library, &all), "undo {round} kept it");
        }
        for round in 0..recorded {
            fixture.library.redo().expect("redo runs");
            assert!(still_cached(&fixture.library, &all), "redo {round} kept it");
        }
        assert!(Arc::ptr_eq(
            &fixture.library.tag_names_by_asset().expect("tag names"),
            &tags
        ));
        assert_eq!(*all, reopened_listing(&fixture.library));
    }

    #[test]
    fn every_other_collection_command_still_drops_the_cache() {
        let mut fixture = fixture();
        let asset = fixture.assets[1];
        let spare = fixture
            .library
            .create_collection("Spare", None)
            .expect("collection");
        let commands = [
            CollectionCommand::Rename {
                id: fixture.second,
                name: "Renamed".to_owned(),
            },
            CollectionCommand::Move {
                id: fixture.child,
                parent: Some(fixture.second),
            },
            CollectionCommand::Arrange {
                parent: None,
                order: {
                    let mut order = vec![fixture.first, fixture.second, spare];
                    order.reverse();
                    order
                },
            },
            CollectionCommand::SetTrashed {
                collections: Vec::new(),
                assets: vec![asset],
                trashed: true,
            },
            CollectionCommand::Trash { id: fixture.second },
            CollectionCommand::Delete { id: spare },
            CollectionCommand::Batch(vec![
                CollectionCommand::AddAssets {
                    collection: fixture.first,
                    assets: vec![fixture.assets[2]],
                },
                CollectionCommand::Rename {
                    id: fixture.first,
                    name: "Mixed".to_owned(),
                },
            ]),
        ];
        for command in &commands {
            let all = warm(&fixture.library);
            fixture
                .library
                .apply_collection_command(command)
                .expect("the command applies");
            assert!(
                !still_cached(&fixture.library, &all),
                "{command:?} dropped the cache"
            );
        }
        let all = warm(&fixture.library);
        fixture
            .library
            .create_collection("Created", None)
            .expect("collection");
        assert!(!still_cached(&fixture.library, &all));
    }

    #[test]
    fn trash_restore_and_tag_commands_still_drop_the_cache() {
        let mut fixture = fixture();
        let asset = fixture.assets[0];
        let tag = fixture
            .library
            .tags()
            .expect("tags")
            .into_iter()
            .find(|tag| tag.name == "Logo")
            .expect("the leaf tag")
            .id;
        let parent = fixture
            .library
            .tags()
            .expect("tags")
            .into_iter()
            .find(|tag| tag.name == "Brands")
            .expect("the parent tag")
            .id;
        let other = fixture
            .library
            .apply_tag_command(&TagCommand::Add {
                assets: vec![asset],
                name: "Other".to_owned(),
            })
            .map(|_| {
                fixture
                    .library
                    .tags()
                    .expect("tags")
                    .into_iter()
                    .find(|tag| tag.name == "Other")
                    .expect("the new tag")
                    .id
            })
            .expect("a tag is added");
        let steps: Vec<Step> = vec![
            Box::new(move |library| {
                library
                    .apply_asset_command(&AssetCommand::SetTrashed {
                        assets: vec![asset],
                        trashed: true,
                    })
                    .expect("trashed");
            }),
            Box::new(move |library| {
                library
                    .apply_asset_command(&AssetCommand::SetTrashed {
                        assets: vec![asset],
                        trashed: false,
                    })
                    .expect("restored");
            }),
            Box::new(move |library| {
                library
                    .apply_tag_command(&TagCommand::Merge {
                        from: other,
                        into: tag,
                    })
                    .expect("merged");
            }),
            Box::new(move |library| {
                library
                    .apply_tag_command(&TagCommand::Dissolve { tag: parent })
                    .expect("dissolved");
            }),
            Box::new(move |library| {
                library
                    .apply_tag_command(&TagCommand::Delete { tag })
                    .expect("deleted");
            }),
        ];
        for step in steps {
            let all = warm(&fixture.library);
            step(&mut fixture.library);
            assert!(!still_cached(&fixture.library, &all));
        }
    }

    #[test]
    fn a_failed_membership_command_leaves_a_coherent_cache_and_the_error() {
        let mut fixture = fixture();
        let unknown = AssetId::parse("00000000-0000-7000-8000-000000000999").expect("valid id");
        let all = warm(&fixture.library);

        let failed = fixture
            .library
            .apply_collection_command(&CollectionCommand::AddAssets {
                collection: fixture.second,
                assets: vec![fixture.assets[1], unknown],
            });

        assert!(matches!(failed, Err(CollectionError::AssetNotFound(id)) if id == unknown));
        let shown = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert_eq!(*shown, reopened_listing(&fixture.library));
        assert_eq!(shown.len(), all.len());
        assert!(
            !fixture
                .library
                .collections_of(fixture.assets[1])
                .expect("memberships")
                .contains(&fixture.second),
            "nothing of a failed command stays"
        );
    }

    #[test]
    fn a_cache_already_stale_is_not_revived_by_a_membership_command() {
        let mut fixture = fixture();
        let asset = fixture.assets[0];
        let all = warm(&fixture.library);
        fixture
            .library
            .apply_asset_command(&AssetCommand::SetFavorite {
                assets: vec![asset],
                favorite: true,
            })
            .expect("a favorite is set");

        fixture
            .library
            .apply_collection_command(&CollectionCommand::AddAssets {
                collection: fixture.second,
                assets: vec![asset],
            })
            .expect("the asset is filed");

        assert!(!still_cached(&fixture.library, &all));
        let shown = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert!(
            shown
                .iter()
                .any(|candidate| candidate.id == asset && candidate.is_favorite)
        );
    }
}
