use std::fs;
use std::time::SystemTime;

use super::{Library, LibraryError, schema};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryOverview {
    pub resources: usize,
    pub favorites: usize,
    pub animated: usize,
    pub vectors: usize,
    pub in_trash: usize,
    pub collections: usize,
    pub tags: usize,
    pub format_version: u32,
    pub created_at: Option<SystemTime>,
}

impl Library {
    pub fn overview(&self) -> Result<LibraryOverview, LibraryError> {
        let count = |query: &str| -> Result<usize, LibraryError> {
            let total: i64 = self.connection.query_row(query, [], |row| row.get(0))?;
            Ok(usize::try_from(total).unwrap_or(0))
        };
        let kept = "FROM assets WHERE trashed_at_unix_ms IS NULL";
        Ok(LibraryOverview {
            resources: count(&format!("SELECT count(*) {kept}"))?,
            favorites: count(&format!("SELECT count(*) {kept} AND is_favorite = 1"))?,
            animated: count(&format!("SELECT count(*) {kept} AND is_animated = 1"))?,
            vectors: count(&format!("SELECT count(*) {kept} AND format = 'svg'"))?,
            in_trash: count("SELECT count(*) FROM assets WHERE trashed_at_unix_ms IS NOT NULL")?,
            collections: count(
                "SELECT count(*) FROM collections WHERE trashed_at_unix_ms IS NULL",
            )?,
            tags: count("SELECT count(*) FROM tags")?,
            format_version: schema::read_format_version(&self.connection)?,
            created_at: fs::metadata(&self.root)
                .and_then(|metadata| metadata.created())
                .ok(),
        })
    }
}
