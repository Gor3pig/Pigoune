use super::{LibraryError, layout};
use std::fs::{self, DirBuilder};
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Expected {
    Folder,
    File,
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
