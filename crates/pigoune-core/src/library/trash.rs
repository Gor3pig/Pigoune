use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::params;

use super::{AssetId, Library, LibraryError, clock, layout};

pub const TRASH_RETENTION: Duration = Duration::from_hours(30 * 24);

impl Library {
    pub fn empty_trash(&mut self) -> Result<usize, LibraryError> {
        self.empty_trash_before(i64::MAX)
    }

    pub fn empty_expired_trash(&mut self) -> Result<usize, LibraryError> {
        let retention = i64::try_from(TRASH_RETENTION.as_millis()).unwrap_or(i64::MAX);
        self.empty_trash_before(clock::now_unix_ms().saturating_sub(retention))
    }

    fn empty_trash_before(&mut self, limit_unix_ms: i64) -> Result<usize, LibraryError> {
        let transaction = self.connection.transaction()?;
        let removed: Vec<AssetId> = {
            let mut statement = transaction.prepare(
                "SELECT id FROM assets WHERE trashed_at_unix_ms IS NOT NULL
                 AND trashed_at_unix_ms <= ?1",
            )?;
            statement
                .query_map([limit_unix_ms], |row| row.get(0))?
                .collect::<Result<_, _>>()?
        };
        transaction.execute(
            "DELETE FROM assets WHERE trashed_at_unix_ms IS NOT NULL AND trashed_at_unix_ms <= ?1",
            params![limit_unix_ms],
        )?;
        let removed_collections = transaction.execute(
            "DELETE FROM collections
             WHERE trashed_at_unix_ms IS NOT NULL AND trashed_at_unix_ms <= ?1",
            params![limit_unix_ms],
        )?;
        transaction.commit()?;
        if !removed.is_empty() || removed_collections > 0 {
            self.history.clear();
        }
        for asset in &removed {
            forget_files(&self.root, *asset);
        }
        Ok(removed.len())
    }
}

fn forget_files(root: &Path, asset: AssetId) {
    let _ = fs::remove_dir_all(layout::asset_dir(root, asset));
    forget_thumbnails(root, asset);
}

pub(super) fn forget_thumbnails(root: &Path, asset: AssetId) {
    let Ok(sizes) = fs::read_dir(layout::thumbnails_dir(root)) else {
        return;
    };
    for size in sizes.flatten() {
        let _ = fs::remove_file(size.path().join(layout::thumbnail_in_size_dir(asset)));
    }
}
