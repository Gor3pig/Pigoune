use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{AssetId, CollectionError, CollectionId, Library, LibraryError, clock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    pub id: CollectionId,
    pub name: String,
    pub parent: Option<CollectionId>,
    pub position: i64,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionPath {
    pub id: CollectionId,
    pub names: Vec<String>,
}

const COLLECTION_COLUMNS: &str = "id, name, parent_id, position, created_at_unix_ms";

impl Library {
    pub fn create_collection(
        &mut self,
        name: &str,
        parent: Option<CollectionId>,
    ) -> Result<CollectionId, CollectionError> {
        let name = valid_name(name)?;
        if let Some(parent) = parent
            && !is_usable_collection(&self.connection, parent)?
        {
            return Err(CollectionError::NotFound(parent));
        }
        ensure_name_is_free(&self.connection, parent, name, None)?;

        Ok(insert_collection(&self.connection, name, parent)?)
    }

    pub fn collection(&self, id: CollectionId) -> Result<Option<Collection>, LibraryError> {
        Ok(self
            .connection
            .query_row(
                &format!("SELECT {COLLECTION_COLUMNS} FROM collections WHERE id = ?1"),
                [id],
                collection_from_row,
            )
            .optional()?)
    }

    pub fn visible_collections(&self) -> Result<Vec<Collection>, LibraryError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLLECTION_COLUMNS} FROM collections
             WHERE trashed_at_unix_ms IS NULL
             ORDER BY position, created_at_unix_ms, id"
        ))?;
        let collections = statement
            .query_map([], collection_from_row)?
            .collect::<Result<_, _>>()?;
        Ok(collections)
    }

    pub fn collections_of(&self, asset: AssetId) -> Result<Vec<CollectionId>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT asset_collections.collection_id FROM asset_collections
             JOIN collections ON collections.id = asset_collections.collection_id
             WHERE asset_collections.asset_id = ?1 AND collections.trashed_at_unix_ms IS NULL
             ORDER BY asset_collections.collection_id",
        )?;
        let ids = statement
            .query_map([asset], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    pub fn collection_paths(&self) -> Result<Vec<CollectionPath>, LibraryError> {
        let collections = self.visible_collections()?;
        let by_id: HashMap<CollectionId, &Collection> = collections
            .iter()
            .map(|collection| (collection.id, collection))
            .collect();
        let mut paths: Vec<CollectionPath> = collections
            .iter()
            .map(|collection| CollectionPath {
                id: collection.id,
                names: names_from_root(collection, &by_id),
            })
            .collect();
        paths.sort_by_cached_key(|path| {
            path.names
                .iter()
                .map(|name| comparable_name(name))
                .collect::<Vec<_>>()
        });
        Ok(paths)
    }
}

fn names_from_root(
    collection: &Collection,
    by_id: &HashMap<CollectionId, &Collection>,
) -> Vec<String> {
    let mut names = vec![collection.name.clone()];
    let mut parent = collection.parent;
    while let Some(ancestor) = parent.and_then(|id| by_id.get(&id)) {
        names.push(ancestor.name.clone());
        parent = ancestor.parent;
    }
    names.reverse();
    names
}

pub fn is_usable_collection(
    connection: &Connection,
    id: CollectionId,
) -> Result<bool, LibraryError> {
    Ok(connection
        .query_row(
            "SELECT 1 FROM collections WHERE id = ?1 AND trashed_at_unix_ms IS NULL",
            [id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub fn insert_collection(
    connection: &Connection,
    name: &str,
    parent: Option<CollectionId>,
) -> Result<CollectionId, LibraryError> {
    let id = CollectionId::generate();
    connection.execute(
        "INSERT INTO collections (id, parent_id, name, position, created_at_unix_ms)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id,
            parent,
            name,
            next_position(connection, parent)?,
            clock::now_unix_ms()
        ],
    )?;
    Ok(id)
}

pub fn next_position(
    connection: &Connection,
    parent: Option<CollectionId>,
) -> Result<i64, LibraryError> {
    Ok(connection.query_row(
        "SELECT coalesce(max(position) + 1, 0) FROM collections
         WHERE parent_id IS ?1 AND trashed_at_unix_ms IS NULL",
        [parent],
        |row| row.get(0),
    )?)
}

pub fn valid_name(name: &str) -> Result<&str, CollectionError> {
    let name = name.trim();
    if name.is_empty() {
        Err(CollectionError::InvalidName)
    } else {
        Ok(name)
    }
}

pub fn ensure_name_is_free(
    connection: &Connection,
    parent: Option<CollectionId>,
    name: &str,
    renamed: Option<CollectionId>,
) -> Result<(), CollectionError> {
    match find_child_named(connection, parent, name)? {
        Some(existing) if Some(existing) != renamed => {
            Err(CollectionError::NameTaken(name.to_owned()))
        }
        _ => Ok(()),
    }
}

fn collection_from_row(row: &Row) -> rusqlite::Result<Collection> {
    Ok(Collection {
        id: row.get("id")?,
        name: row.get("name")?,
        parent: row.get("parent_id")?,
        position: row.get("position")?,
        created_at_unix_ms: row.get("created_at_unix_ms")?,
    })
}

pub fn find_child_named(
    connection: &Connection,
    parent: Option<CollectionId>,
    name: &str,
) -> Result<Option<CollectionId>, LibraryError> {
    let wanted = comparable_name(name);
    let mut statement = connection.prepare(
        "SELECT id, name FROM collections
         WHERE parent_id IS ?1 AND trashed_at_unix_ms IS NULL
         ORDER BY created_at_unix_ms, id",
    )?;
    let mut siblings = statement.query([parent])?;
    while let Some(row) = siblings.next()? {
        let sibling_name: String = row.get(1)?;
        if comparable_name(&sibling_name) == wanted {
            return Ok(Some(row.get(0)?));
        }
    }
    Ok(None)
}

pub fn comparable_name(name: &str) -> String {
    name.trim().to_lowercase()
}
