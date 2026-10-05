use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, params};

use super::collection::{self, Collection, ensure_name_is_free, is_usable_collection, valid_name};
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
