use rusqlite::{Connection, OptionalExtension, params};

use super::{AssetId, CollectionError, CollectionId, Library, LibraryError, clock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    pub id: CollectionId,
    pub name: String,
    pub parent: Option<CollectionId>,
}

impl Library {
    pub fn create_collection(
        &mut self,
        name: &str,
        parent: Option<CollectionId>,
    ) -> Result<CollectionId, CollectionError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CollectionError::InvalidName);
        }
        if let Some(parent) = parent
            && !is_usable_collection(&self.connection, parent)?
        {
            return Err(CollectionError::NotFound(parent));
        }

        Ok(insert_collection(&self.connection, name, parent)?)
    }

    pub fn collection(&self, id: CollectionId) -> Result<Option<Collection>, LibraryError> {
        Ok(self
            .connection
            .query_row(
                "SELECT id, name, parent_id FROM collections WHERE id = ?1",
                [id],
                |row| {
                    Ok(Collection {
                        id: row.get("id")?,
                        name: row.get("name")?,
                        parent: row.get("parent_id")?,
                    })
                },
            )
            .optional()?)
    }

    pub fn collections_of(&self, asset: AssetId) -> Result<Vec<CollectionId>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT collection_id FROM asset_collections
             WHERE asset_id = ?1 ORDER BY collection_id",
        )?;
        let ids = statement
            .query_map([asset], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }
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
        "INSERT INTO collections (id, parent_id, name, created_at_unix_ms)
         VALUES (?1, ?2, ?3, ?4)",
        params![id, parent, name, clock::now_unix_ms()],
    )?;
    Ok(id)
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
