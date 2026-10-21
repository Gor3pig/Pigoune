use std::path::{Component, Path, PathBuf};

use super::{AssetId, LibraryError};

pub const LIBRARY_EXTENSION: &str = "pigoune";
pub const DATABASE_FILE_NAME: &str = "library.db";
pub const FILES_DIR_NAME: &str = "files";
pub const CACHE_DIR_NAME: &str = "cache";
pub const DETAILS_FILE_NAME: &str = "visual-details.db";
pub const DETAILS_SIDECAR_SUFFIXES: [&str; 3] = ["-journal", "-wal", "-shm"];
pub const THUMBNAILS_DIR_NAME: &str = "thumbnails";
pub const ANNOTATED_DIR_NAME: &str = "annotated";
pub const EXPORT_DIR_NAME: &str = "export";
pub const CLIPBOARD_DIR_NAME: &str = "clipboard";
pub const OPENING_DIR_NAME: &str = "open";
pub const PASTED_DIR_NAME: &str = "pasted";
pub const CACHE_FOLDER_NAMES: [&str; 6] = [
    THUMBNAILS_DIR_NAME,
    ANNOTATED_DIR_NAME,
    EXPORT_DIR_NAME,
    CLIPBOARD_DIR_NAME,
    OPENING_DIR_NAME,
    PASTED_DIR_NAME,
];

const UUID_TEXT_LENGTH: usize = 36;
const RENDER_KEY_LENGTH: usize = 32;
pub const RANDOM_TEXT_LENGTH: usize = 16;
const MAX_NAME_BYTES: usize = 200;
const BUCKET_NAME_LENGTH: usize = 2;
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

pub fn bucket_name(id: AssetId) -> String {
    let text = id.to_string();
    text[text.len() - BUCKET_NAME_LENGTH..].to_owned()
}

pub fn files_dir(root: &Path) -> PathBuf {
    root.join(FILES_DIR_NAME)
}

pub fn asset_dir(root: &Path, id: AssetId) -> PathBuf {
    files_dir(root).join(bucket_name(id)).join(id.to_string())
}

pub fn cache_dir(root: &Path) -> PathBuf {
    root.join(CACHE_DIR_NAME)
}

pub fn details_path(root: &Path) -> PathBuf {
    root.join(CACHE_DIR_NAME).join(DETAILS_FILE_NAME)
}

pub fn annotated_dir(root: &Path) -> PathBuf {
    root.join(CACHE_DIR_NAME).join(ANNOTATED_DIR_NAME)
}

pub fn annotated_render_path(root: &Path, id: AssetId, key: &str) -> PathBuf {
    annotated_dir(root).join(format!("{id}-{key}.png"))
}

pub fn annotated_render_owner(file_name: &str) -> Option<AssetId> {
    let (identifier, rest) = file_name.split_at_checked(UUID_TEXT_LENGTH)?;
    let id = AssetId::parse(identifier)?;
    let rest = rest.strip_prefix('-')?;
    let (key, extension) = rest.split_once('.')?;
    let finished = extension == "png";
    let unfinished = extension == "partial"
        || extension.strip_suffix(".partial").is_some_and(|random| {
            random.len() == RANDOM_TEXT_LENGTH
                && random.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
    (key.len() == RENDER_KEY_LENGTH
        && key.bytes().all(|byte| byte.is_ascii_hexdigit())
        && (finished || unfinished))
        .then_some(id)
}

pub fn is_annotated_render_of(id: AssetId, file_name: &str) -> bool {
    file_name
        .strip_prefix(&format!("{id}-"))
        .is_some_and(|rest| {
            Path::new(rest)
                .extension()
                .is_some_and(|extension| extension == "png" || extension == "partial")
        })
}

pub fn thumbnails_dir(root: &Path) -> PathBuf {
    root.join(CACHE_DIR_NAME).join(THUMBNAILS_DIR_NAME)
}

pub fn export_dir(root: &Path) -> PathBuf {
    root.join(CACHE_DIR_NAME).join(EXPORT_DIR_NAME)
}

pub fn clipboard_dir(root: &Path) -> PathBuf {
    root.join(CACHE_DIR_NAME).join(CLIPBOARD_DIR_NAME)
}

pub fn pasted_dir(root: &Path) -> PathBuf {
    root.join(CACHE_DIR_NAME).join(PASTED_DIR_NAME)
}

pub fn opening_dir(root: &Path) -> PathBuf {
    root.join(CACHE_DIR_NAME).join(OPENING_DIR_NAME)
}

pub fn thumbnail_path(root: &Path, id: AssetId, pixels: u32) -> PathBuf {
    thumbnails_dir(root)
        .join(pixels.to_string())
        .join(thumbnail_in_size_dir(id))
}

pub fn thumbnail_in_size_dir(id: AssetId) -> PathBuf {
    Path::new(&bucket_name(id)).join(thumbnail_file_name(id))
}

fn thumbnail_file_name(id: AssetId) -> String {
    format!("{id}.png")
}

pub fn is_stored_path_of(id: AssetId, path: &Path) -> bool {
    let Some(Component::Normal(name)) = path.components().next_back() else {
        return false;
    };
    let Some(name) = name.to_str() else {
        return false;
    };
    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
        && Path::new(&stored_path(id, name)) == path
}

pub fn stored_path(id: AssetId, original_file_name: &str) -> String {
    format!(
        "{FILES_DIR_NAME}/{}/{id}/{original_file_name}",
        bucket_name(id)
    )
}

pub fn unfinished_import_dir(root: &Path, id: AssetId) -> PathBuf {
    files_dir(root).join(format!(".{id}{UNFINISHED_IMPORT_SUFFIX}"))
}

pub fn unfinished_replacement_name(id: AssetId) -> String {
    format!(".{id}{UNFINISHED_IMPORT_SUFFIX}")
}

pub fn is_unfinished_import(entry_name: &str) -> bool {
    entry_name.starts_with('.') && entry_name.ends_with(UNFINISHED_IMPORT_SUFFIX)
}

fn strip_library_extension(name: &str) -> &str {
    name.strip_suffix(&format!(".{LIBRARY_EXTENSION}"))
        .unwrap_or(name)
}
