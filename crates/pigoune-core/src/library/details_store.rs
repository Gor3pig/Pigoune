use std::fs;
use std::path::Path;

use rusqlite::Connection;

use super::{LibraryError, layout};

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS details.asset_details (
    asset_id TEXT PRIMARY KEY NOT NULL,
    detail BLOB NOT NULL
)";
const FILE_SETTINGS: &str = "PRAGMA details.journal_mode = DELETE;
    PRAGMA details.synchronous = NORMAL;";

pub fn attach(connection: &Connection, root: &Path) -> Result<(), LibraryError> {
    let path = layout::details_path(root);
    if open_attached(connection, &path).is_ok() {
        return Ok(());
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
    connection.execute("ATTACH DATABASE ?1 AS details", [path.to_string_lossy()])?;
    connection.execute_batch(FILE_SETTINGS)?;
    connection.execute_batch(SCHEMA)
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
