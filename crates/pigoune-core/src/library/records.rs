use rusqlite::OptionalExtension;

use super::{Asset, AssetId, Library, LibraryError};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LibraryRecords {
    pub heaviest: Option<Asset>,
    pub largest: Option<Asset>,
    pub newest: Option<Asset>,
    pub oldest: Option<Asset>,
}

impl Library {
    pub fn records(&self) -> Result<LibraryRecords, LibraryError> {
        Ok(LibraryRecords {
            heaviest: self.first_kept("ORDER BY byte_size DESC")?,
            largest: self.first_kept(
                "AND format != 'svg' AND width IS NOT NULL AND height IS NOT NULL
                 ORDER BY width * height DESC",
            )?,
            newest: self.first_kept("ORDER BY added_at_unix_ms DESC")?,
            oldest: self.first_kept("ORDER BY added_at_unix_ms ASC")?,
        })
    }

    fn first_kept(&self, ordering: &str) -> Result<Option<Asset>, LibraryError> {
        let id: Option<AssetId> = self
            .connection
            .query_row(
                &format!(
                    "SELECT id FROM assets WHERE trashed_at_unix_ms IS NULL {ordering}, id LIMIT 1"
                ),
                [],
                |row| row.get(0),
            )
            .optional()?;
        match id {
            Some(id) => self.asset(id),
            None => Ok(None),
        }
    }
}
