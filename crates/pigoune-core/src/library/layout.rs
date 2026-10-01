use std::path::{Path, PathBuf};

use super::LibraryError;

pub const LIBRARY_EXTENSION: &str = "pigoune";
pub const DATABASE_FILE_NAME: &str = "library.db";
pub const FILES_DIR_NAME: &str = "files";
pub const CACHE_DIR_NAME: &str = "cache";

const MAX_NAME_BYTES: usize = 200;

pub fn library_folder_name(requested_name: &str) -> Result<String, LibraryError> {
    let name = strip_library_extension(requested_name.trim()).trim();

    let is_forbidden = name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('.')
        || name.len() > MAX_NAME_BYTES
        || name.contains(['/', '\0']);

    if is_forbidden {
        return Err(LibraryError::InvalidName);
    }

    Ok(format!("{name}.{LIBRARY_EXTENSION}"))
}

pub fn library_display_name(root: &Path) -> String {
    root.file_name()
        .map(|folder_name| strip_library_extension(&folder_name.to_string_lossy()).to_owned())
        .unwrap_or_default()
}

pub fn database_path(root: &Path) -> PathBuf {
    root.join(DATABASE_FILE_NAME)
}

fn strip_library_extension(name: &str) -> &str {
    name.strip_suffix(&format!(".{LIBRARY_EXTENSION}"))
        .unwrap_or(name)
}
