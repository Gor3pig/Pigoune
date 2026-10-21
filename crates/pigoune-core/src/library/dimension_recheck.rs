use std::path::Path;

use rusqlite::{Connection, params};

use super::{AssetId, LibraryError, OriginalFile};
use crate::media::{self, AssetFormat};

const ORIENTED_FORMATS: [AssetFormat; 4] = [
    AssetFormat::Jpeg,
    AssetFormat::Tiff,
    AssetFormat::Webp,
    AssetFormat::Png,
];

pub fn correct_oriented_dimensions(
    connection: &Connection,
    root: &Path,
) -> Result<(), LibraryError> {
    for format in ORIENTED_FORMATS {
        for (id, stored_path, width, height) in stored_dimensions(connection, format)? {
            let Some(asset) = AssetId::parse(&id) else {
                continue;
            };
            let Ok(original) = OriginalFile::for_migration(root, asset, Path::new(&stored_path))
            else {
                continue;
            };
            if original.check().is_err() {
                continue;
            }
            let Some(found) = media::dimensions_of(original.path(), format) else {
                continue;
            };
            let turned = found.width() == height && found.height() == width;
            if turned && width != height {
                connection.execute(
                    "UPDATE assets SET width = ?2, height = ?3 WHERE id = ?1",
                    params![id, found.width(), found.height()],
                )?;
            }
        }
    }
    Ok(())
}

fn stored_dimensions(
    connection: &Connection,
    format: AssetFormat,
) -> Result<Vec<(String, String, u32, u32)>, LibraryError> {
    let mut statement = connection.prepare(
        "SELECT id, stored_path, width, height FROM assets
         WHERE format = ?1 AND width IS NOT NULL AND height IS NOT NULL",
    )?;
    let rows = statement.query_map(params![format], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
