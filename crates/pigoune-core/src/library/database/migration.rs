use super::{DatabaseError, array16};
use crate::{DATABASE_SCHEMA_VERSION, LibraryId};
use rusqlite::{Connection, TransactionBehavior};

const MIN_SQLITE_VERSION: i32 = 3_037_000;
const INITIAL_SCHEMA: &str = include_str!("../../migrations/0001_initial.sql");
const IMAGE_METADATA_SCHEMA: &str = include_str!("../../migrations/0002_image_metadata.sql");

pub(super) fn check_sqlite_version() -> Result<(), DatabaseError> {
    let version = rusqlite::version_number();
    if version < MIN_SQLITE_VERSION {
        return Err(DatabaseError::SQLiteTooOld(version));
    }
    Ok(())
}

pub(super) fn configure_writer(connection: &Connection) -> Result<(), DatabaseError> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.busy_timeout(std::time::Duration::from_millis(5_000))?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    let mode: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(DatabaseError::InvalidJournalMode(mode));
    }
    Ok(())
}

pub(super) fn schema_version(connection: &Connection) -> Result<i32, DatabaseError> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(Into::into)
}

pub(super) fn migrate(
    connection: &mut Connection,
    library_id: LibraryId,
) -> Result<(), DatabaseError> {
    let version = schema_version(connection)?;
    if version > DATABASE_SCHEMA_VERSION {
        return Err(DatabaseError::SchemaTooNew(version));
    }
    if version < 1 {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(INITIAL_SCHEMA)?;
        transaction.execute(
            "INSERT INTO library_metadata (singleton, library_id) VALUES (1, ?1)",
            [library_id.to_bytes().as_slice()],
        )?;
        transaction.pragma_update(None, "user_version", 1)?;
        transaction.commit()?;
    }
    if version < 2 {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(IMAGE_METADATA_SCHEMA)?;
        transaction.pragma_update(None, "user_version", 2)?;
        transaction.commit()?;
    }
    Ok(())
}

pub(super) fn verify_library_id(
    connection: &Connection,
    expected: LibraryId,
) -> Result<(), DatabaseError> {
    let found = read_library_id(connection)?;
    if found != expected {
        return Err(DatabaseError::LibraryIdMismatch { expected, found });
    }
    Ok(())
}

pub(super) fn read_library_id(connection: &Connection) -> Result<LibraryId, DatabaseError> {
    let mut statement = connection.prepare("SELECT singleton, library_id FROM library_metadata")?;
    let mut rows = statement.query([])?;
    let Some(row) = rows.next()? else {
        return Err(DatabaseError::InvalidMetadata);
    };
    let singleton: i64 = row.get(0)?;
    let bytes: Vec<u8> = row.get(1)?;
    if singleton != 1 || rows.next()?.is_some() {
        return Err(DatabaseError::InvalidMetadata);
    }
    let bytes = array16(bytes, "library ID")?;
    LibraryId::from_bytes(bytes).map_err(DatabaseError::InvalidStoredLibraryId)
}
