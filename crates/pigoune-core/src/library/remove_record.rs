use std::fs;

use super::trash::forget_thumbnails;
use super::{AssetId, Library, RemoveRecordError, layout};
use super::{details_store, ignored_duplicates};

impl Library {
    pub fn remove_record_of_missing_file(&mut self, id: AssetId) -> Result<(), RemoveRecordError> {
        let asset = self
            .asset(id)?
            .ok_or(RemoveRecordError::AssetNotFound(id))?;
        let inside = layout::is_stored_path_of(id, &asset.stored_path);
        let file = self.root.join(&asset.stored_path);
        if inside && !matches!(fs::exists(&file), Ok(false)) {
            return Err(RemoveRecordError::FileStillThere(id));
        }
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM assets WHERE id = ?1", [id])?;
        ignored_duplicates::forget_dead_actions(&transaction)?;
        transaction.commit()?;
        details_store::forget(&mut self.connection, &[id]);
        self.history.clear();
        forget_thumbnails(&self.root, id);
        if inside && let Some(folder) = file.parent() {
            remove_original_files(folder);
            let _ = fs::remove_dir(folder);
        }
        Ok(())
    }
}

fn remove_original_files(folder: &std::path::Path) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let is_original = entry
            .file_name()
            .to_str()
            .and_then(layout::original_generation_of)
            .is_some();
        if is_original && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            let _ = fs::remove_file(entry.path());
        }
    }
}
