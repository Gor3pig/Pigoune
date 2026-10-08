use std::fs;

use super::trash::forget_thumbnails;
use super::{AssetId, Library, RemoveRecordError};

impl Library {
    pub fn remove_record_of_missing_file(&mut self, id: AssetId) -> Result<(), RemoveRecordError> {
        let asset = self
            .asset(id)?
            .ok_or(RemoveRecordError::AssetNotFound(id))?;
        let file = self.root.join(&asset.stored_path);
        if file.exists() {
            return Err(RemoveRecordError::FileStillThere(id));
        }
        self.connection
            .execute("DELETE FROM assets WHERE id = ?1", [id])?;
        self.history.clear();
        forget_thumbnails(&self.root, id);
        if let Some(folder) = file.parent() {
            let _ = fs::remove_dir(folder);
        }
        Ok(())
    }
}
