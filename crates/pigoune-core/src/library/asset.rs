use std::fmt;
use std::path::PathBuf;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{OptionalExtension, Row};
use uuid::Uuid;

use super::{Library, LibraryError};
use crate::media::{AssetFormat, Dimensions};

const EMBEDDED_SIZES_SEPARATOR: char = ',';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(Uuid);

impl AssetId {
    pub(super) fn generate() -> Self {
        Self(Uuid::now_v7())
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(formatter)
    }
}

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
}

impl Library {
    pub fn asset(&self, id: AssetId) -> Result<Option<Asset>, LibraryError> {
        Ok(self
            .connection
            .query_row(
                "SELECT id, display_name, original_file_name, stored_path, format, width, height,
                        byte_size, content_hash, is_animated, embedded_sizes, added_at_unix_ms
                 FROM assets WHERE id = ?1",
                [id],
                asset_from_row,
            )
            .optional()?)
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

impl ToSql for AssetId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for AssetId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Uuid::parse_str(value.as_str()?)
            .map(Self)
            .map_err(|error| FromSqlError::Other(Box::new(error)))
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
