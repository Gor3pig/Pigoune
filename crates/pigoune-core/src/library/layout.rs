use std::path::{Path, PathBuf};

use super::{AssetId, LibraryError};

pub const LIBRARY_EXTENSION: &str = "pigoune";
pub const DATABASE_FILE_NAME: &str = "library.db";
pub const FILES_DIR_NAME: &str = "files";
pub const CACHE_DIR_NAME: &str = "cache";
const THUMBNAILS_DIR_NAME: &str = "thumbnails";
const EXPORT_DIR_NAME: &str = "export";

const MAX_NAME_BYTES: usize = 200;
const UNFINISHED_IMPORT_SUFFIX: &str = ".partial";

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

#[must_use]
pub fn library_display_name(root: &Path) -> String {
    root.file_name()
        .map(|folder_name| strip_library_extension(&folder_name.to_string_lossy()).to_owned())
        .unwrap_or_default()
}

pub fn database_path(root: &Path) -> PathBuf {
    root.join(DATABASE_FILE_NAME)
}

pub fn asset_dir(root: &Path, id: AssetId) -> PathBuf {
    root.join(FILES_DIR_NAME).join(id.to_string())
}

pub fn thumbnails_dir(root: &Path) -> PathBuf {
    root.join(CACHE_DIR_NAME).join(THUMBNAILS_DIR_NAME)
}

pub fn export_dir(root: &Path) -> PathBuf {
    root.join(CACHE_DIR_NAME).join(EXPORT_DIR_NAME)
}

pub fn thumbnail_path(root: &Path, id: AssetId, pixels: u32) -> PathBuf {
    thumbnails_dir(root)
        .join(pixels.to_string())
        .join(thumbnail_file_name(id))
}

pub fn thumbnail_file_name(id: AssetId) -> String {
    format!("{id}.png")
}

pub fn stored_path(id: AssetId, original_file_name: &str) -> String {
    format!("{FILES_DIR_NAME}/{id}/{original_file_name}")
}

pub fn unfinished_import_dir(root: &Path, id: AssetId) -> PathBuf {
    root.join(FILES_DIR_NAME)
        .join(format!(".{id}{UNFINISHED_IMPORT_SUFFIX}"))
}

pub fn is_unfinished_import(entry_name: &str) -> bool {
    entry_name.starts_with('.') && entry_name.ends_with(UNFINISHED_IMPORT_SUFFIX)
}

fn strip_library_extension(name: &str) -> &str {
    name.strip_suffix(&format!(".{LIBRARY_EXTENSION}"))
        .unwrap_or(name)
}
