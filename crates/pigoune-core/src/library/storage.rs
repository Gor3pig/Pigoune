use std::collections::BTreeMap;
use std::fs;

use super::{Library, LibraryError, layout};
use crate::media::{self, AssetColor, AssetFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatShare {
    pub format: AssetFormat,
    pub count: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorShare {
    pub color: AssetColor,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageUse {
    pub resources: u64,
    pub trash: u64,
    pub thumbnails: u64,
    pub database: u64,
}

impl StorageUse {
    #[must_use]
    pub fn total(&self) -> u64 {
        self.resources + self.trash + self.thumbnails + self.database
    }
}

impl Library {
    pub fn format_shares(&self) -> Result<Vec<FormatShare>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT format, count(*), sum(byte_size) FROM assets
             WHERE trashed_at_unix_ms IS NULL
             GROUP BY format",
        )?;
        let shares = statement.query_map([], |row| {
            Ok(FormatShare {
                format: row.get(0)?,
                count: usize::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                bytes: u64::try_from(row.get::<_, i64>(2)?).unwrap_or(0),
            })
        })?;
        Ok(shares.collect::<Result<_, _>>()?)
    }

    pub fn color_shares(&self) -> Result<Vec<ColorShare>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT colors FROM assets
             WHERE trashed_at_unix_ms IS NULL AND colors IS NOT NULL",
        )?;
        let mut counts: BTreeMap<AssetColor, usize> = BTreeMap::new();
        for colors in statement.query_map([], |row| row.get::<_, String>(0))? {
            let mut families: Vec<AssetColor> = media::dominant_from_text(&colors?)
                .into_iter()
                .map(|dominant| dominant.family)
                .collect();
            families.sort_unstable();
            families.dedup();
            for family in families {
                *counts.entry(family).or_default() += 1;
            }
        }
        let mut shares: Vec<ColorShare> = counts
            .into_iter()
            .map(|(color, count)| ColorShare { color, count })
            .collect();
        shares.sort_by_key(|share| std::cmp::Reverse(share.count));
        Ok(shares)
    }

    pub fn storage_use(&self) -> Result<StorageUse, LibraryError> {
        let bytes = |condition: &str| -> Result<u64, LibraryError> {
            let total: i64 = self.connection.query_row(
                &format!("SELECT coalesce(sum(byte_size), 0) FROM assets WHERE {condition}"),
                [],
                |row| row.get(0),
            )?;
            Ok(u64::try_from(total).unwrap_or(0))
        };
        Ok(StorageUse {
            resources: bytes("trashed_at_unix_ms IS NULL")?,
            trash: bytes("trashed_at_unix_ms IS NOT NULL")?,
            thumbnails: self.thumbnail_cache_bytes(),
            database: fs::metadata(layout::database_path(&self.root))
                .map_or(0, |metadata| metadata.len()),
        })
    }
}
