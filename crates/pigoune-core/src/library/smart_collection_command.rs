use rusqlite::{Connection, OptionalExtension, params};

use super::clock::now_unix_ms;
use super::smart_collection::{self, SmartCollection, can_be_saved_from, normalized};
use super::{AssetFilter, AssetView, Change, Library, LibraryError, SmartCollectionId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartCollectionCommand {
    Create { collection: SmartCollection },
    Update { collection: SmartCollection },
    Delete { id: SmartCollectionId },
    Arrange { order: Vec<SmartCollectionId> },
}

#[derive(Debug, thiserror::Error)]
pub enum SmartCollectionError {
    #[error("a smart collection name cannot be empty")]
    InvalidName,
    #[error("a smart collection named {0} already exists")]
    NameTaken(String),
    #[error("the smart collection {0} does not exist")]
    NotFound(SmartCollectionId),
    #[error("a smart collection cannot be saved from this view")]
    InvalidScope,
    #[error("the smart collections to arrange no longer match the library")]
    OutdatedOrder,
    #[error(transparent)]
    Library(#[from] LibraryError),
}

impl From<rusqlite::Error> for SmartCollectionError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Library(error.into())
    }
}

impl Library {
    pub fn create_smart_collection(
        &mut self,
        name: &str,
        scope: AssetView,
        filter: &AssetFilter,
    ) -> Result<SmartCollectionId, SmartCollectionError> {
        let id = SmartCollectionId::generate();
        self.apply_smart_collection_command(&SmartCollectionCommand::Create {
            collection: SmartCollection {
                id,
                name: name.to_owned(),
                scope,
                filter: filter.clone(),
                position: next_position(&self.connection)?,
                created_at_unix_ms: now_unix_ms(),
            },
        })?;
        Ok(id)
    }

    pub fn apply_smart_collection_command(
        &mut self,
        command: &SmartCollectionCommand,
    ) -> Result<SmartCollectionCommand, SmartCollectionError> {
        let inverse = self.run_smart_collection_command(command)?;
        self.history.record(
            Change::SmartCollection(command.clone()),
            Change::SmartCollection(inverse.clone()),
        );
        Ok(inverse)
    }

    pub(super) fn run_smart_collection_command(
        &mut self,
        command: &SmartCollectionCommand,
    ) -> Result<SmartCollectionCommand, SmartCollectionError> {
        let transaction = self.connection.transaction()?;
        let inverse = apply(&transaction, command)?;
        transaction.commit()?;
        Ok(inverse)
    }
}

fn apply(
    connection: &Connection,
    command: &SmartCollectionCommand,
) -> Result<SmartCollectionCommand, SmartCollectionError> {
    match command {
        SmartCollectionCommand::Create { collection } => create(connection, collection),
        SmartCollectionCommand::Update { collection } => update(connection, collection),
        SmartCollectionCommand::Delete { id } => delete(connection, *id),
        SmartCollectionCommand::Arrange { order } => arrange(connection, order),
    }
}

fn create(
    connection: &Connection,
    collection: &SmartCollection,
) -> Result<SmartCollectionCommand, SmartCollectionError> {
    if !can_be_saved_from(collection.scope) {
        return Err(SmartCollectionError::InvalidScope);
    }
    let name = valid_name(&collection.name)?;
    ensure_name_is_free(connection, name, None)?;
    let collection = SmartCollection {
        name: name.to_owned(),
        ..collection.clone()
    };
    smart_collection::insert(connection, &collection)?;
    Ok(SmartCollectionCommand::Delete { id: collection.id })
}

fn update(
    connection: &Connection,
    collection: &SmartCollection,
) -> Result<SmartCollectionCommand, SmartCollectionError> {
    let existing = existing(connection, collection.id)?;
    if !can_be_saved_from(collection.scope) {
        return Err(SmartCollectionError::InvalidScope);
    }
    let name = valid_name(&collection.name)?;
    ensure_name_is_free(connection, name, Some(collection.id))?;
    connection.execute(
        "UPDATE smart_collections
         SET name = ?1, normalized_name = ?2, scope = ?3, search_text = ?4, formats = ?5,
             favorites_only = ?6
         WHERE id = ?7",
        params![
            name,
            normalized(name),
            smart_collection::scope_text(collection.scope),
            collection.filter.text,
            smart_collection::formats_text(&collection.filter.formats),
            collection.filter.favorites_only,
            collection.id,
        ],
    )?;
    Ok(SmartCollectionCommand::Update {
        collection: existing,
    })
}

fn delete(
    connection: &Connection,
    id: SmartCollectionId,
) -> Result<SmartCollectionCommand, SmartCollectionError> {
    let existing = existing(connection, id)?;
    connection.execute("DELETE FROM smart_collections WHERE id = ?1", [id])?;
    Ok(SmartCollectionCommand::Create {
        collection: existing,
    })
}

fn arrange(
    connection: &Connection,
    order: &[SmartCollectionId],
) -> Result<SmartCollectionCommand, SmartCollectionError> {
    let mut statement =
        connection.prepare("SELECT id FROM smart_collections ORDER BY position, id")?;
    let previous = statement
        .query_map([], |row| row.get::<_, SmartCollectionId>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut wanted = order.to_vec();
    let mut known = previous.clone();
    wanted.sort();
    wanted.dedup();
    known.sort();
    if wanted != known || wanted.len() != order.len() {
        return Err(SmartCollectionError::OutdatedOrder);
    }
    for (position, id) in order.iter().enumerate() {
        connection.execute(
            "UPDATE smart_collections SET position = ?1 WHERE id = ?2",
            params![i64::try_from(position).unwrap_or(i64::MAX), id],
        )?;
    }
    Ok(SmartCollectionCommand::Arrange { order: previous })
}

fn next_position(connection: &Connection) -> Result<i64, SmartCollectionError> {
    Ok(connection.query_row(
        "SELECT coalesce(max(position) + 1, 0) FROM smart_collections",
        [],
        |row| row.get(0),
    )?)
}

fn existing(
    connection: &Connection,
    id: SmartCollectionId,
) -> Result<SmartCollection, SmartCollectionError> {
    smart_collection::find(connection, id)?.ok_or(SmartCollectionError::NotFound(id))
}

fn valid_name(name: &str) -> Result<&str, SmartCollectionError> {
    let name = name.trim();
    if name.is_empty() {
        Err(SmartCollectionError::InvalidName)
    } else {
        Ok(name)
    }
}

fn ensure_name_is_free(
    connection: &Connection,
    name: &str,
    renamed: Option<SmartCollectionId>,
) -> Result<(), SmartCollectionError> {
    let holder: Option<SmartCollectionId> = connection
        .query_row(
            "SELECT id FROM smart_collections WHERE normalized_name = ?1",
            [normalized(name)],
            |row| row.get(0),
        )
        .optional()?;
    match holder {
        Some(holder) if Some(holder) != renamed => {
            Err(SmartCollectionError::NameTaken(name.to_owned()))
        }
        _ => Ok(()),
    }
}
