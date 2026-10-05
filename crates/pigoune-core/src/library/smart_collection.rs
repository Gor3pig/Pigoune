use rusqlite::{Connection, OptionalExtension, Row, params};

use super::view::AssetView;
use super::{Asset, AssetFilter, CollectionId, Library, LibraryError, SmartCollectionId, TagId};
use crate::media::{AssetFormat, colors_from_text, colors_text};

const SCOPE_ALL: &str = "all";
const SCOPE_FAVORITES: &str = "favorites";
const SCOPE_UNCLASSIFIED: &str = "unclassified";
const SCOPE_COLLECTION: &str = "collection:";
const SCOPE_TAG: &str = "tag:";
const FORMAT_SEPARATOR: char = ',';
const COLUMNS: &str =
    "id, name, scope, search_text, formats, favorites_only, position, created_at_unix_ms, colors";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartCollection {
    pub id: SmartCollectionId,
    pub name: String,
    pub scope: AssetView,
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
            Some(collection) => self.find_assets_in(collection.scope, &collection.filter),
            None => Ok(Vec::new()),
        }
    }
}

#[must_use]
pub fn can_be_saved_from(view: AssetView) -> bool {
    matches!(
        view,
        AssetView::All
            | AssetView::Favorites
            | AssetView::Unclassified
            | AssetView::Collection(_)
            | AssetView::Tag(_)
    )
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
              created_at_unix_ms, colors)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            collection.id,
            collection.name,
            normalized(&collection.name),
            scope_text(collection.scope),
            collection.filter.text,
            formats_text(&collection.filter.formats),
            collection.filter.favorites_only,
            collection.position,
            collection.created_at_unix_ms,
            colors_text(&collection.filter.colors),
        ],
    )?;
    Ok(())
}

pub fn normalized(name: &str) -> String {
    name.trim().to_lowercase()
}

fn smart_collection_from_row(row: &Row<'_>) -> rusqlite::Result<SmartCollection> {
    let scope: String = row.get(2)?;
    let formats: String = row.get(4)?;
    Ok(SmartCollection {
        id: row.get(0)?,
        name: row.get(1)?,
        scope: scope_from_text(&scope),
        filter: AssetFilter {
            text: row.get(3)?,
            formats: formats_from_text(&formats),
            favorites_only: row.get(5)?,
            colors: colors_from_text(&row.get::<_, String>(8)?),
        },
        position: row.get(6)?,
        created_at_unix_ms: row.get(7)?,
    })
}

pub fn scope_text(scope: AssetView) -> String {
    match scope {
        AssetView::Favorites => SCOPE_FAVORITES.to_owned(),
        AssetView::Unclassified => SCOPE_UNCLASSIFIED.to_owned(),
        AssetView::Collection(id) => format!("{SCOPE_COLLECTION}{id}"),
        AssetView::Tag(id) => format!("{SCOPE_TAG}{id}"),
        AssetView::All | AssetView::Trash | AssetView::Smart(_) => SCOPE_ALL.to_owned(),
    }
}

fn scope_from_text(text: &str) -> AssetView {
    match text {
        SCOPE_FAVORITES => AssetView::Favorites,
        SCOPE_UNCLASSIFIED => AssetView::Unclassified,
        _ => text
            .strip_prefix(SCOPE_COLLECTION)
            .and_then(CollectionId::parse)
            .map(AssetView::Collection)
            .or_else(|| {
                text.strip_prefix(SCOPE_TAG)
                    .and_then(TagId::parse)
                    .map(AssetView::Tag)
            })
            .unwrap_or(AssetView::All),
    }
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
    use super::{formats_from_text, formats_text, scope_from_text, scope_text};
    use crate::library::{AssetView, CollectionId, TagId};
    use crate::media::AssetFormat;

    #[test]
    fn every_scope_survives_a_round_trip_through_text() {
        let collection = CollectionId::parse("00000000-0000-7000-8000-000000000007").expect("id");
        let tag = TagId::parse("00000000-0000-7000-8000-000000000009").expect("id");
        for scope in [
            AssetView::All,
            AssetView::Favorites,
            AssetView::Unclassified,
            AssetView::Collection(collection),
            AssetView::Tag(tag),
        ] {
            assert_eq!(scope_from_text(&scope_text(scope)), scope);
        }
        assert_eq!(scope_from_text("nonsense"), AssetView::All);
    }

    #[test]
    fn formats_survive_a_round_trip_through_text() {
        let formats = vec![AssetFormat::Svg, AssetFormat::Png];
        assert_eq!(formats_from_text(&formats_text(&formats)), formats);
        assert!(formats_from_text("").is_empty());
    }
}
