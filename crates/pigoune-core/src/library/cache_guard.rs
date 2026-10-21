use std::ffi::OsStr;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use uuid::Uuid;

use super::{LibraryError, layout};

const STALE_TEMPORARY: Duration = Duration::from_mins(10);
const TEMPORARY_ATTEMPTS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Expected {
    Folder,
    File,
}

#[derive(Clone, Copy)]
enum Temporary {
    Hidden,
    Visible,
}

fn unsafe_entry(path: &Path) -> LibraryError {
    LibraryError::UnsafeCache(path.to_path_buf())
}

fn invalid_destination() -> LibraryError {
    LibraryError::Io(io::Error::from(io::ErrorKind::InvalidInput))
}

fn require(path: &Path, expected: Expected) -> Result<(), LibraryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if expected == Expected::Folder && metadata.is_dir() => Ok(()),
        Ok(metadata) if expected == Expected::File && metadata.is_file() => Ok(()),
        Ok(_) => Err(unsafe_entry(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn make_plain_folder(path: &Path) -> Result<(), LibraryError> {
    for _ in 0..2 {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => return Ok(()),
            Ok(_) => return Err(unsafe_entry(path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match DirBuilder::new().create(path) {
                    Ok(()) => return Ok(()),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error.into()),
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(unsafe_entry(path))
}

pub(super) fn verify_cache(root: &Path) -> Result<(), LibraryError> {
    let cache = layout::cache_dir(root);
    require(&cache, Expected::Folder)?;
    for name in layout::CACHE_FOLDER_NAMES {
        require(&cache.join(name), Expected::Folder)?;
    }
    require(&layout::details_path(root), Expected::File)?;
    for suffix in layout::DETAILS_SIDECAR_SUFFIXES {
        let mut sidecar = layout::details_path(root).into_os_string();
        sidecar.push(suffix);
        require(Path::new(&sidecar), Expected::File)?;
    }
    verify_thumbnail_tree(&layout::thumbnails_dir(root))
}

fn verify_thumbnail_tree(thumbnails: &Path) -> Result<(), LibraryError> {
    let Ok(sizes) = fs::read_dir(thumbnails) else {
        return Ok(());
    };
    for size in sizes.flatten() {
        let Ok(kind) = size.file_type() else {
            continue;
        };
        if is_link_to_a_folder(&size.path(), kind) {
            return Err(unsafe_entry(&size.path()));
        }
        if !kind.is_dir() {
            continue;
        }
        let Ok(buckets) = fs::read_dir(size.path()) else {
            continue;
        };
        for bucket in buckets.flatten() {
            let path = bucket.path();
            if bucket
                .file_type()
                .is_ok_and(|kind| is_link_to_a_folder(&path, kind))
            {
                return Err(unsafe_entry(&path));
            }
        }
    }
    Ok(())
}

fn is_link_to_a_folder(path: &Path, kind: fs::FileType) -> bool {
    kind.is_symlink() && fs::metadata(path).is_ok_and(|target| target.is_dir())
}

pub(super) fn ensure_cache_folder(root: &Path) -> Result<(), LibraryError> {
    make_plain_folder(&layout::cache_dir(root))
}

pub(super) fn check_folder(library_root: &Path, folder: &Path) -> Result<(), LibraryError> {
    let relative = folder
        .strip_prefix(library_root)
        .map_err(|_| unsafe_entry(folder))?;
    let mut current = library_root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        require(&current, Expected::Folder)?;
    }
    Ok(())
}

pub fn create_cache_folder(folder: &Path) -> Result<(), LibraryError> {
    let components: Vec<Component<'_>> = folder.components().collect();
    let Some(cache_index) = components
        .iter()
        .rposition(|component| component.as_os_str() == layout::CACHE_DIR_NAME)
    else {
        return Err(invalid_destination());
    };
    let mut current = PathBuf::new();
    for (index, component) in components.iter().enumerate() {
        current.push(component);
        if index >= cache_index {
            make_plain_folder(&current)?;
        }
    }
    Ok(())
}

pub(super) fn remove_tree_keeping_links(path: &Path) -> io::Result<Option<PathBuf>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if metadata.is_file() {
        fs::remove_file(path)?;
        return Ok(None);
    }
    if !metadata.is_dir() {
        return Ok(Some(path.to_path_buf()));
    }
    let mut kept = None;
    for child in fs::read_dir(path)? {
        let found = remove_tree_keeping_links(&child?.path())?;
        kept = kept.or(found);
    }
    if kept.is_none() {
        fs::remove_dir(path)?;
    }
    Ok(kept)
}

pub(super) fn remove_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
        && fs::remove_file(path).is_ok()
}

pub fn write_thumbnail_file(destination: &Path, content: &[u8]) -> Result<(), LibraryError> {
    write_cache_file(destination, content, Temporary::Hidden)
}

pub fn write_render_file(destination: &Path, content: &[u8]) -> Result<(), LibraryError> {
    write_cache_file(destination, content, Temporary::Visible)
}

fn write_cache_file(
    destination: &Path,
    content: &[u8],
    style: Temporary,
) -> Result<(), LibraryError> {
    let folder = destination.parent().ok_or_else(invalid_destination)?;
    let name = destination
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(invalid_destination)?;
    create_cache_folder(folder)?;
    clear_old_temporaries(folder, name, style)?;
    let (mut file, temporary) = create_temporary(folder, name, style)?;
    let written = file.write_all(content).and_then(|()| match style {
        Temporary::Visible => file.sync_all(),
        Temporary::Hidden => Ok(()),
    });
    drop(file);
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    publish_temporary(&temporary, destination)
}

fn stem_of(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

fn is_random_part(text: &str) -> bool {
    text.len() == layout::RANDOM_TEXT_LENGTH && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_temporary_of(candidate: &str, name: &str, style: Temporary) -> bool {
    let prefix = match style {
        Temporary::Hidden => format!(".{name}."),
        Temporary::Visible => format!("{}.", stem_of(name)),
    };
    candidate.strip_prefix(&prefix).is_some_and(|rest| {
        rest == "partial" || rest.strip_suffix(".partial").is_some_and(is_random_part)
    })
}

fn temporary_name(name: &str, style: Temporary) -> String {
    let random = format!("{:016x}", Uuid::now_v7().as_u64_pair().1);
    match style {
        Temporary::Hidden => format!(".{name}.{random}.partial"),
        Temporary::Visible => format!("{}.{random}.partial", stem_of(name)),
    }
}

fn clear_old_temporaries(folder: &Path, name: &str, style: Temporary) -> Result<(), LibraryError> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let candidate = entry.file_name();
        if !candidate
            .to_str()
            .is_some_and(|text| is_temporary_of(text, name, style))
        {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_file() {
            return Err(unsafe_entry(&entry.path()));
        }
        let is_old = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age >= STALE_TEMPORARY);
        if is_old {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

fn create_temporary(
    folder: &Path,
    name: &str,
    style: Temporary,
) -> Result<(fs::File, PathBuf), LibraryError> {
    for _ in 0..TEMPORARY_ATTEMPTS {
        let path = folder.join(temporary_name(name, style));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(unsafe_entry(&folder.join(name)))
}

fn publish_temporary(temporary: &Path, destination: &Path) -> Result<(), LibraryError> {
    let refusal = match fs::symlink_metadata(destination) {
        Ok(metadata) if !metadata.is_file() => Some(unsafe_entry(destination)),
        Err(error) if error.kind() != io::ErrorKind::NotFound => Some(error.into()),
        _ => None,
    };
    if let Some(refusal) = refusal {
        let _ = fs::remove_file(temporary);
        return Err(refusal);
    }
    fs::rename(temporary, destination).map_err(|error| {
        let _ = fs::remove_file(temporary);
        error.into()
    })
}
