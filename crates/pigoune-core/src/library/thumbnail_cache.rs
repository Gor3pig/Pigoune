use std::fs;
use std::io;
use std::path::Path;

use super::{Library, LibraryError, cache_guard, layout};

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
        let cache = layout::cache_dir(&self.root);
        cache_guard::check_folder(&self.root, &cache)?;
        let mut kept = None;
        for folder in [
            layout::thumbnails_dir(&self.root),
            layout::annotated_dir(&self.root),
        ] {
            match cache_guard::remove_tree_keeping_links(&folder) {
                Ok(found) => kept = kept.or(found),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        kept.map_or(Ok(()), |path| Err(LibraryError::UnsafeCache(path)))
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
