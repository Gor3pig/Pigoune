use std::fs;
use std::path::Path;

use super::{AssetId, Library, LibraryError, layout};

impl Library {
    pub fn empty_trash(&mut self) -> Result<usize, LibraryError> {
        let transaction = self.connection.transaction()?;
        let removed: Vec<AssetId> = {
            let mut statement = transaction
                .prepare("SELECT id FROM assets WHERE trashed_at_unix_ms IS NOT NULL")?;
            statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<_, _>>()?
        };
        transaction.execute(
            "DELETE FROM assets WHERE trashed_at_unix_ms IS NOT NULL",
            [],
        )?;
        transaction.execute(
            "DELETE FROM collections WHERE trashed_at_unix_ms IS NOT NULL",
            [],
        )?;
        transaction.commit()?;
        self.history.clear();
        for asset in &removed {
            forget_files(&self.root, *asset);
        }
        Ok(removed.len())
    }
}

fn forget_files(root: &Path, asset: AssetId) {
    let _ = fs::remove_dir_all(layout::asset_dir(root, asset));
    let Ok(sizes) = fs::read_dir(layout::thumbnails_dir(root)) else {
        return;
    };
    for size in sizes.flatten() {
        let _ = fs::remove_file(size.path().join(layout::thumbnail_file_name(asset)));
    }
}
