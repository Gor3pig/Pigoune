use rusqlite::{Connection, OptionalExtension, Row, params};

use super::view::AssetView;
use super::{Asset, AssetFilter, Library, LibraryError, SmartCollectionId};
use crate::media::{
    AssetFormat, Rgb, families_from_text, families_text, shapes_from_text, shapes_text,
};

const FORMAT_SEPARATOR: char = ',';
const COLUMNS: &str =
    "id, name, search_text, formats, favorites_only, position, created_at_unix_ms, colors, custom_color,
     shapes, fits_screen";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartCollection {
    pub id: SmartCollectionId,
    pub name: String,
    pub filter: AssetFilter,
    pub position: i64,
    pub created_at_unix_ms: i64,
}

impl Library {
    pub fn smart_collections(&self) -> Result<Vec<SmartCollection>, LibraryError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM smart_collections ORDER BY normalized_name, id"
        ))?;
        let collections = statement
            .query_map([], smart_collection_from_row)?
            .collect::<Result<_, _>>()?;
        Ok(collections)
    }

    pub fn smart_collection(
        &self,
        id: SmartCollectionId,
    ) -> Result<Option<SmartCollection>, LibraryError> {
        find(&self.connection, id)
    }

    pub(super) fn smart_collection_assets(
        &self,
        id: SmartCollectionId,
    ) -> Result<Vec<Asset>, LibraryError> {
        match self.smart_collection(id)? {
            Some(collection) => self.find_assets_in(AssetView::All, &collection.filter),
            None => Ok(Vec::new()),
        }
    }
}

pub fn find(
    connection: &Connection,
    id: SmartCollectionId,
) -> Result<Option<SmartCollection>, LibraryError> {
    Ok(connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM smart_collections WHERE id = ?1"),
            [id],
            smart_collection_from_row,
        )
        .optional()?)
}

pub fn insert(connection: &Connection, collection: &SmartCollection) -> Result<(), LibraryError> {
    connection.execute(
        "INSERT INTO smart_collections
             (id, name, normalized_name, scope, search_text, formats, favorites_only, position,
              created_at_unix_ms, colors, custom_color, shapes, fits_screen)
         VALUES (?1, ?2, ?3, 'all', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            collection.id,
            collection.name,
            normalized(&collection.name),
            collection.filter.text,
            formats_text(&collection.filter.formats),
            collection.filter.favorites_only,
            collection.position,
            collection.created_at_unix_ms,
            families_text(&collection.filter.colors),
            collection.filter.custom_color.map(Rgb::hex),
            shapes_text(&collection.filter.shapes),
            collection.filter.fits_screen,
        ],
    )?;
    Ok(())
}

pub fn normalized(name: &str) -> String {
    name.trim().to_lowercase()
}

fn smart_collection_from_row(row: &Row<'_>) -> rusqlite::Result<SmartCollection> {
    let formats: String = row.get(3)?;
    Ok(SmartCollection {
        id: row.get(0)?,
        name: row.get(1)?,
        filter: AssetFilter {
            text: row.get(2)?,
            formats: formats_from_text(&formats),
            favorites_only: row.get(4)?,
            colors: families_from_text(&row.get::<_, String>(7)?),
            custom_color: row
                .get::<_, Option<String>>(8)?
                .and_then(|text| Rgb::from_hex(&text)),
            shapes: shapes_from_text(&row.get::<_, String>(9)?),
            fits_screen: row.get(10)?,
            screen: None,
        },
        position: row.get(5)?,
        created_at_unix_ms: row.get(6)?,
    })
}

pub fn formats_text(formats: &[AssetFormat]) -> String {
    formats
        .iter()
        .map(|format| format.code())
        .collect::<Vec<_>>()
        .join(&FORMAT_SEPARATOR.to_string())
}

fn formats_from_text(text: &str) -> Vec<AssetFormat> {
    text.split(FORMAT_SEPARATOR)
        .filter_map(AssetFormat::from_code)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{formats_from_text, formats_text};
    use crate::media::AssetFormat;

    #[test]
    fn formats_survive_a_round_trip_through_text() {
        let formats = vec![AssetFormat::Svg, AssetFormat::Png];
        assert_eq!(formats_from_text(&formats_text(&formats)), formats);
        assert!(formats_from_text("").is_empty());
    }
}
