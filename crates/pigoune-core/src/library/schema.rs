use std::path::Path;

use rusqlite::{Connection, ErrorCode};

use super::{LibraryError, animation_recheck};

pub const CURRENT_FORMAT_VERSION: u32 = 2;
pub const PIGOUNE_APPLICATION_ID: i32 = 0x5049_4755;

enum Migration {
    Statements(&'static str),
    RecheckAnimations,
}

const MIGRATIONS: [Migration; CURRENT_FORMAT_VERSION as usize] = [
    Migration::Statements(include_str!("migrations/v1.sql")),
    Migration::RecheckAnimations,
];

pub fn configure_connection(connection: &Connection) -> Result<(), LibraryError> {
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "DELETE")?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

pub fn initialize_new_database(
    connection: &mut Connection,
    root: &Path,
) -> Result<(), LibraryError> {
    connection.pragma_update(None, "application_id", PIGOUNE_APPLICATION_ID)?;
    migrate_to_current_version(connection, root)
}

pub fn ensure_is_pigoune_database(connection: &Connection) -> Result<bool, LibraryError> {
    match connection.pragma_query_value(None, "application_id", |row| row.get::<_, i32>(0)) {
        Ok(application_id) => Ok(application_id == PIGOUNE_APPLICATION_ID),
        Err(error) if is_not_a_database(&error) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub fn ensure_is_intact(connection: &Connection) -> Result<(), LibraryError> {
    let verdict: String = connection.pragma_query_value(None, "quick_check", |row| row.get(0))?;
    if verdict == "ok" {
        Ok(())
    } else {
        Err(LibraryError::Damaged)
    }
}

pub fn ensure_is_writable(connection: &Connection) -> Result<(), LibraryError> {
    if connection.is_readonly(rusqlite::MAIN_DB)? {
        Err(LibraryError::PermissionDenied)
    } else {
        Ok(())
    }
}

pub fn read_format_version(connection: &Connection) -> Result<u32, LibraryError> {
    Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

pub fn migrate_to_current_version(
    connection: &mut Connection,
    root: &Path,
) -> Result<(), LibraryError> {
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
            apply(&transaction, &migration, root)?;
        }
    }
    transaction.pragma_update(None, "user_version", CURRENT_FORMAT_VERSION)?;
    transaction.commit()?;
    Ok(())
}

fn apply(connection: &Connection, migration: &Migration, root: &Path) -> Result<(), LibraryError> {
    match migration {
        Migration::Statements(statements) => Ok(connection.execute_batch(statements)?),
        Migration::RecheckAnimations => animation_recheck::mark_animated_assets(connection, root),
    }
}

fn is_not_a_database(error: &rusqlite::Error) -> bool {
    error.sqlite_error_code() == Some(ErrorCode::NotADatabase)
}
