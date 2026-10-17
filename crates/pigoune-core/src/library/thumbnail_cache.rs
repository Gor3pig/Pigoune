use std::fs;
use std::io;
use std::path::Path;

use super::{Library, LibraryError, layout};

#[must_use]
pub fn thumbnail_cache_bytes_in(root: &Path) -> u64 {
    folder_bytes(&layout::thumbnails_dir(root)) + folder_bytes(&layout::annotated_dir(root))
}

impl Library {
    #[must_use]
    pub fn thumbnail_cache_bytes(&self) -> u64 {
        thumbnail_cache_bytes_in(&self.root)
    }

    pub fn clear_thumbnail_cache(&self) -> Result<(), LibraryError> {
        for folder in [
            layout::thumbnails_dir(&self.root),
            layout::annotated_dir(&self.root),
        ] {
            match fs::remove_dir_all(folder) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
                _ => {}
            }
        }
        Ok(())
    }
}

fn folder_bytes(folder: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(folder) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => folder_bytes(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |metadata| metadata.len()),
            _ => 0,
        })
        .sum()
}
