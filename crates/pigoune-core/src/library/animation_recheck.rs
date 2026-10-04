use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use super::LibraryError;
use crate::media::{self, AssetFormat};

const FORMATS_ANIMATED_SINCE_FORMAT_2: [AssetFormat; 2] = [AssetFormat::Png, AssetFormat::Webp];

pub fn mark_animated_assets(connection: &Connection, root: &Path) -> Result<(), LibraryError> {
    for format in FORMATS_ANIMATED_SINCE_FORMAT_2 {
        for (id, stored_path) in still_assets(connection, format)? {
            if media::is_animated(&root.join(stored_path), format).unwrap_or(false) {
                connection.execute("UPDATE assets SET is_animated = 1 WHERE id = ?1", [id])?;
            }
        }
    }
    Ok(())
}

fn still_assets(
    connection: &Connection,
    format: AssetFormat,
) -> Result<Vec<(String, PathBuf)>, LibraryError> {
    let mut statement = connection
        .prepare("SELECT id, stored_path FROM assets WHERE format = ?1 AND is_animated = 0")?;
    let rows = statement.query_map(params![format], |row| {
        Ok((row.get(0)?, PathBuf::from(row.get::<_, String>(1)?)))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
