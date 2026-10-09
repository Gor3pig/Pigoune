use std::collections::HashSet;
use std::path::PathBuf;

use super::view::{self, AssetView, SUBTREE};
use super::{AssetId, Library, LibraryError, layout};
use crate::media::{
    self, AnimationTiming, AssetFormat, Dimensions, DominantColor, dominant_from_text,
};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{OptionalExtension, Row, params};

const EMBEDDED_SIZES_SEPARATOR: char = ',';
const ASSET_COLUMNS: &str =
    "id, display_name, original_file_name, stored_path, format, width, height,
    byte_size, content_hash, is_animated, embedded_sizes, added_at_unix_ms, trashed_at_unix_ms,
    is_favorite, note, source_url, license, author, colors";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub id: AssetId,
    pub display_name: String,
    pub original_file_name: String,
    pub stored_path: PathBuf,
    pub format: AssetFormat,
    pub dimensions: Option<Dimensions>,
    pub byte_size: u64,
    pub content_hash: String,
    pub is_animated: bool,
    pub embedded_sizes: Vec<Dimensions>,
    pub added_at_unix_ms: i64,
    pub trashed_at_unix_ms: Option<i64>,
    pub is_favorite: bool,
    pub note: String,
    pub source_url: String,
    pub license: String,
    pub author: String,
    pub colors: Vec<DominantColor>,
}

impl Library {
    pub fn asset(&self, id: AssetId) -> Result<Option<Asset>, LibraryError> {
        Ok(self
            .connection
            .query_row(
                &format!("SELECT {ASSET_COLUMNS} FROM assets WHERE id = ?1"),
                [id],
                asset_from_row,
            )
            .optional()?)
    }

    pub fn visible_assets(&self) -> Result<Vec<Asset>, LibraryError> {
        self.visible_assets_in(AssetView::All)
    }

    pub fn visible_assets_in(&self, view: AssetView) -> Result<Vec<Asset>, LibraryError> {
        if let AssetView::Smart(id) = view {
            return self.smart_collection_assets(id);
        }
        let filter = view::filter(view);
        let clause = filter.clause();
        let mut statement = self.connection.prepare(&format!(
            "{SUBTREE} SELECT {ASSET_COLUMNS} FROM assets
             WHERE {clause}
             ORDER BY added_at_unix_ms DESC, id DESC"
        ))?;
        let assets = statement
            .query_map([filter.parameter], asset_from_row)?
            .collect::<Result<_, _>>()?;
        Ok(assets)
    }

    pub fn view_contains(&self, view: AssetView, asset: AssetId) -> Result<bool, LibraryError> {
        if let AssetView::Smart(id) = view {
            return Ok(self
                .smart_collection_assets(id)?
                .iter()
                .any(|candidate| candidate.id == asset));
        }
        let filter = view::filter(view);
        let clause = filter.clause();
        Ok(self
            .connection
            .query_row(
                &format!("{SUBTREE} SELECT 1 FROM assets WHERE id = ?2 AND {clause}"),
                params![filter.parameter, asset],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    pub fn assets_outside_view(
        &self,
        view: AssetView,
        assets: &[AssetId],
    ) -> Result<Vec<AssetId>, LibraryError> {
        if let AssetView::Smart(id) = view {
            let inside: HashSet<AssetId> = self
                .smart_collection_assets(id)?
                .into_iter()
                .map(|asset| asset.id)
                .collect();
            return Ok(assets
                .iter()
                .copied()
                .filter(|asset| !inside.contains(asset))
                .collect());
        }
        let mut outside = Vec::new();
        for asset in assets {
            if !self.view_contains(view, *asset)? {
                outside.push(*asset);
            }
        }
        Ok(outside)
    }

    #[must_use]
    pub fn file_of(&self, asset: &Asset) -> PathBuf {
        self.root.join(&asset.stored_path)
    }

    #[must_use]
    pub fn animation_timing(&self, asset: &Asset) -> Option<AnimationTiming> {
        if asset.is_animated {
            media::animation_timing(&self.file_of(asset), asset.format)
        } else {
            None
        }
    }

    pub fn thumbnail_file(&self, id: AssetId, pixels: u32) -> PathBuf {
        layout::thumbnail_path(&self.root, id, pixels)
    }

    pub fn assets_awaiting_colors(&self, limit: usize) -> Result<Vec<AssetId>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT id FROM assets WHERE colors IS NULL
             ORDER BY added_at_unix_ms DESC, id DESC LIMIT ?1",
        )?;
        let ids = statement
            .query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    pub fn record_colors(
        &mut self,
        id: AssetId,
        colors: &[DominantColor],
    ) -> Result<(), LibraryError> {
        self.connection.execute(
            "UPDATE assets SET colors = ?2 WHERE id = ?1",
            params![id, media::dominant_text(colors)],
        )?;
        Ok(())
    }
}

fn asset_from_row(row: &Row) -> rusqlite::Result<Asset> {
    let width: Option<u32> = row.get("width")?;
    let height: Option<u32> = row.get("height")?;
    Ok(Asset {
        id: row.get("id")?,
        display_name: row.get("display_name")?,
        original_file_name: row.get("original_file_name")?,
        stored_path: PathBuf::from(row.get::<_, String>("stored_path")?),
        format: row.get("format")?,
        dimensions: width
            .zip(height)
            .and_then(|(width, height)| Dimensions::new(width, height)),
        byte_size: row.get("byte_size")?,
        content_hash: row.get("content_hash")?,
        is_animated: row.get("is_animated")?,
        embedded_sizes: row.get::<_, EmbeddedSizes>("embedded_sizes")?.0,
        added_at_unix_ms: row.get("added_at_unix_ms")?,
        trashed_at_unix_ms: row.get("trashed_at_unix_ms")?,
        is_favorite: row.get("is_favorite")?,
        note: row.get("note")?,
        source_url: row.get("source_url")?,
        license: row.get("license")?,
        author: row.get("author")?,
        colors: row
            .get::<_, Option<String>>("colors")?
            .map(|text| dominant_from_text(&text))
            .unwrap_or_default(),
    })
}

pub(super) struct EmbeddedSizes(pub Vec<Dimensions>);

impl ToSql for EmbeddedSizes {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let text = self
            .0
            .iter()
            .map(Dimensions::to_string)
            .collect::<Vec<_>>()
            .join(&EMBEDDED_SIZES_SEPARATOR.to_string());
        Ok(ToSqlOutput::from(text))
    }
}

impl FromSql for EmbeddedSizes {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        if text.is_empty() {
            return Ok(Self(Vec::new()));
        }
        text.split(EMBEDDED_SIZES_SEPARATOR)
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map(Self)
            .map_err(|_| FromSqlError::InvalidType)
    }
}

impl ToSql for AssetFormat {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.code()))
    }
}

impl FromSql for AssetFormat {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Self::from_code(value.as_str()?).ok_or(FromSqlError::InvalidType)
    }
}
