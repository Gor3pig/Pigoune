use rusqlite::{Connection, ErrorCode};

use super::LibraryError;

pub const CURRENT_FORMAT_VERSION: u32 = 1;
pub const PIGOUNE_APPLICATION_ID: i32 = 0x5049_4755;

const MIGRATIONS: [&str; CURRENT_FORMAT_VERSION as usize] = [include_str!("migrations/v1.sql")];

pub fn configure_connection(connection: &Connection) -> Result<(), LibraryError> {
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "DELETE")?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

pub fn initialize_new_database(connection: &mut Connection) -> Result<(), LibraryError> {
    connection.pragma_update(None, "application_id", PIGOUNE_APPLICATION_ID)?;
    migrate_to_current_version(connection)
}

pub fn ensure_is_pigoune_database(connection: &Connection) -> Result<bool, LibraryError> {
    match connection.pragma_query_value(None, "application_id", |row| row.get::<_, i32>(0)) {
        Ok(application_id) => Ok(application_id == PIGOUNE_APPLICATION_ID),
        Err(error) if is_not_a_database(&error) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub fn read_format_version(connection: &Connection) -> Result<u32, LibraryError> {
    Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

pub fn migrate_to_current_version(connection: &mut Connection) -> Result<(), LibraryError> {
    let found = read_format_version(connection)?;
    if found > CURRENT_FORMAT_VERSION {
        return Err(LibraryError::NewerFormat {
            found,
            supported: CURRENT_FORMAT_VERSION,
        });
    }
    if found == CURRENT_FORMAT_VERSION {
        return Ok(());
    }

    let transaction = connection.transaction()?;
    for (version, migration) in (1..=CURRENT_FORMAT_VERSION).zip(MIGRATIONS) {
        if version > found {
            transaction.execute_batch(migration)?;
        }
    }
    transaction.pragma_update(None, "user_version", CURRENT_FORMAT_VERSION)?;
    transaction.commit()?;
    Ok(())
}

fn is_not_a_database(error: &rusqlite::Error) -> bool {
    error.sqlite_error_code() == Some(ErrorCode::NotADatabase)
}
