use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::params;

use super::{
    AssetId, Library, LibraryError, cache_guard, clock, details_store, ignored_duplicates, layout,
};

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
        ignored_duplicates::forget_dead_actions(&transaction)?;
        transaction.commit()?;
        details_store::forget(&mut self.connection, &removed);
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
    let _ = super::storage_guard::remove_asset_folder(root, asset);
    forget_thumbnails(root, asset);
}

pub(super) fn forget_thumbnails(root: &Path, asset: AssetId) {
    forget_annotated_renders(root, asset);
    let thumbnails = layout::thumbnails_dir(root);
    if cache_guard::check_folder(root, &thumbnails).is_err() {
        return;
    }
    let Ok(sizes) = fs::read_dir(thumbnails) else {
        return;
    };
    for size in sizes.flatten() {
        if !size.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let bucket = size.path().join(Path::new(&layout::bucket_name(asset)));
        if fs::symlink_metadata(&bucket).is_ok_and(|metadata| metadata.is_dir()) {
            cache_guard::remove_plain_file(&size.path().join(layout::thumbnail_in_size_dir(asset)));
        }
    }
}

pub(super) fn forget_annotated_renders(root: &Path, asset: AssetId) {
    let annotated = layout::annotated_dir(root);
    if cache_guard::check_folder(root, &annotated).is_err() {
        return;
    }
    let Ok(entries) = fs::read_dir(annotated) else {
        return;
    };
    for entry in entries.flatten() {
        let named = entry
            .file_name()
            .to_str()
            .is_some_and(|name| layout::is_annotated_render_of(asset, name));
        if named && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::TRASH_RETENTION;

    #[test]
    fn the_retention_matches_the_thirty_days_the_interface_announces() {
        assert_eq!(
            TRASH_RETENTION,
            Duration::from_hours(30 * 24),
            "update \"After 30 days\" in preferences-dialog.blp and the trash text in window.rs"
        );
    }
}
