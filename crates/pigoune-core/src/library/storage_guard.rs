use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use super::{AssetId, LibraryError, cache_guard, layout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Bucketed,
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginalFile {
    root: PathBuf,
    id: AssetId,
    shape: Shape,
    path: PathBuf,
}

fn refused(path: &Path) -> LibraryError {
    LibraryError::UnsafeStorage(path.to_path_buf())
}

fn require_plain(path: &Path, folder: bool) -> Result<(), LibraryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if folder && metadata.is_dir() => Ok(()),
        Ok(metadata) if !folder && metadata.is_file() => Ok(()),
        Ok(_) => Err(refused(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

impl OriginalFile {
    pub fn new(root: &Path, id: AssetId, stored_path: &Path) -> Result<Self, LibraryError> {
        let path =
            layout::original_path(root, id, stored_path).ok_or_else(|| refused(stored_path))?;
        Ok(Self {
            root: root.to_path_buf(),
            id,
            shape: Shape::Bucketed,
            path,
        })
    }

    pub(super) fn for_migration(
        root: &Path,
        id: AssetId,
        stored_path: &Path,
    ) -> Result<Self, LibraryError> {
        if let Ok(original) = Self::new(root, id, stored_path) {
            return Ok(original);
        }
        let path = layout::legacy_original_path(root, id, stored_path)
            .ok_or_else(|| refused(stored_path))?;
        Ok(Self {
            root: root.to_path_buf(),
            id,
            shape: Shape::Legacy,
            path,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn id(&self) -> AssetId {
        self.id
    }

    pub fn check(&self) -> Result<(), LibraryError> {
        require_plain(&layout::files_dir(&self.root), true)?;
        match self.shape {
            Shape::Bucketed => {
                require_plain(&layout::bucket_dir(&self.root, self.id), true)?;
                require_plain(&layout::asset_dir(&self.root, self.id), true)?;
            }
            Shape::Legacy => {
                require_plain(
                    &layout::files_dir(&self.root).join(self.id.to_string()),
                    true,
                )?;
            }
        }
        require_plain(&self.path, false)
    }

    pub fn open(&self) -> io::Result<File> {
        self.check().map_err(io::Error::other)?;
        cache_guard::open_plain_file(&self.path)
    }
}
