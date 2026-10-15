use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rusqlite::{Connection, ErrorCode};

use super::{AssetId, LibraryError, layout};

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS details.asset_details (
    asset_id TEXT PRIMARY KEY NOT NULL,
    detail BLOB NOT NULL
)";
const FILE_SETTINGS: &str = "PRAGMA details.journal_mode = DELETE;
    PRAGMA details.synchronous = NORMAL;";

pub fn attach(connection: &Connection, root: &Path) -> Result<(), LibraryError> {
    let path = layout::details_path(root);
    match open_attached(connection, &path).and_then(|()| check_intact(connection)) {
        Ok(()) => {
            forget_orphans(connection);
            return Ok(());
        }
        Err(error) if is_damage(&error) => {}
        Err(error) => return Err(error.into()),
    }
    let _ = connection.execute_batch("DETACH DATABASE details");
    let _ = fs::remove_file(&path);
    open_attached(connection, &path)?;
    Ok(())
}

fn open_attached(connection: &Connection, path: &Path) -> rusqlite::Result<()> {
    if !path.exists() {
        fs::File::create(path).map_err(io_error)?;
    }
    connection.execute(
        "ATTACH DATABASE ?1 AS details",
        [path.as_os_str().as_bytes()],
    )?;
    connection.execute_batch(FILE_SETTINGS)?;
    connection.execute_batch(SCHEMA)
}

fn is_damage(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(failure.code, ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
    )
}

pub fn forget(connection: &mut Connection, assets: &[AssetId]) {
    let _ = forget_in_one_transaction(connection, assets);
}

fn forget_in_one_transaction(
    connection: &mut Connection,
    assets: &[AssetId],
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    {
        let mut statement =
            transaction.prepare_cached("DELETE FROM details.asset_details WHERE asset_id = ?1")?;
        for asset in assets {
            statement.execute([asset])?;
        }
    }
    transaction.commit()
}

fn check_intact(connection: &Connection) -> rusqlite::Result<()> {
    let verdict: String =
        connection.pragma_query_value(Some("details"), "quick_check", |row| row.get(0))?;
    if verdict == "ok" {
        Ok(())
    } else {
        Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
            None,
        ))
    }
}

fn forget_orphans(connection: &Connection) {
    let _ = connection.execute(
        "DELETE FROM details.asset_details
         WHERE asset_id NOT IN (SELECT id FROM main.assets)",
        [],
    );
}

pub fn move_out_of_assets(connection: &Connection, root: &Path) -> Result<(), LibraryError> {
    let has_column: bool = connection.query_row(
        "SELECT count(*) > 0 FROM pragma_table_info('assets') WHERE name = 'visual_detail'",
        [],
        |row| row.get(0),
    )?;
    if !has_column {
        return Ok(());
    }
    let _ = copy_to_details_file(connection, root);
    connection.execute_batch("ALTER TABLE assets DROP COLUMN visual_detail")?;
    Ok(())
}

fn copy_to_details_file(connection: &Connection, root: &Path) -> rusqlite::Result<()> {
    fs::create_dir_all(root.join(layout::CACHE_DIR_NAME)).map_err(io_error)?;
    let mut target = Connection::open(layout::details_path(root))?;
    target.execute_batch(&SCHEMA.replace("details.", ""))?;
    let transaction = target.transaction()?;
    {
        let mut insert = transaction
            .prepare("INSERT OR REPLACE INTO asset_details (asset_id, detail) VALUES (?1, ?2)")?;
        let mut statement = connection
            .prepare("SELECT id, visual_detail FROM assets WHERE visual_detail IS NOT NULL")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            insert.execute((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))?;
        }
    }
    transaction.commit()
}

fn io_error(error: std::io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}
