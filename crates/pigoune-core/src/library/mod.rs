mod database;
mod filename;
mod id;
mod manifest;
mod path;

pub use database::{AssetRecord, DatabaseError, LibraryDatabase, StoredObject};
pub use filename::{OriginalFilename, OriginalFilenameError};
pub use id::{LibraryId, ParseLibraryIdError};
pub use manifest::{LibraryManifest, ManifestError};

use crate::{ObjectStore, StoreError};
use rustix::fs::{RenameFlags, renameat_with};
use rustix::io::Errno;
use std::{
    error::Error,
    fmt,
    fs::{self, File},
    io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const LIBRARY_FORMAT_VERSION: u32 = 1;
pub const DATABASE_SCHEMA_VERSION: i32 = 2;

pub struct Library {
    pub id: LibraryId,
    pub database: LibraryDatabase,
    pub object_store: ObjectStore,
}

#[derive(Debug)]
pub enum LibraryError {
    Io(io::Error),
    InvalidDestination(&'static str),
    InvalidParent(io::Error),
    DestinationExists,
    Manifest(ManifestError),
    Database(DatabaseError),
    Store(StoreError),
    Publication(io::Error),
    AtomicPublicationUnsupported(io::Error),
    Published(Box<LibraryError>),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "library I/O error: {error}"),
            Self::InvalidDestination(reason) => write!(f, "invalid library destination: {reason}"),
            Self::InvalidParent(error) => write!(f, "invalid library parent: {error}"),
            Self::DestinationExists => f.write_str("library destination already exists"),
            Self::Manifest(error) => error.fmt(f),
            Self::Database(error) => error.fmt(f),
            Self::Store(error) => error.fmt(f),
            Self::Publication(error) => write!(f, "atomic library publication failed: {error}"),
            Self::AtomicPublicationUnsupported(error) => {
                write!(f, "atomic library publication is unsupported: {error}")
            }
            Self::Published(error) => {
                write!(f, "library was published but finalization failed: {error}")
            }
        }
    }
}

impl Error for LibraryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error)
            | Self::InvalidParent(error)
            | Self::Publication(error)
            | Self::AtomicPublicationUnsupported(error) => Some(error),
            Self::Manifest(error) => Some(error),
            Self::Database(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Published(error) => Some(error),
            Self::InvalidDestination(_) | Self::DestinationExists => None,
        }
    }
}

impl Library {
    /// Create the v1 layout at a destination that does not exist yet.
    pub fn create(root: &Path) -> Result<Self, LibraryError> {
        let name = destination_name(root)?;
        let parent_path = root
            .parent()
            .ok_or(LibraryError::InvalidDestination("missing parent"))?;
        let parent = fs::canonicalize(parent_path).map_err(LibraryError::InvalidParent)?;
        if !parent.is_dir() {
            return Err(LibraryError::InvalidDestination(
                "parent is not a directory",
            ));
        }
        let destination = parent.join(name);
        match fs::symlink_metadata(&destination) {
            Ok(_) => return Err(LibraryError::DestinationExists),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(LibraryError::Io(error)),
        }
        let parent_file = File::open(&parent).map_err(LibraryError::InvalidParent)?;
        let mut workspace = CreationWorkspace::new(&parent).map_err(LibraryError::Io)?;
        let id = LibraryId::new();
        Self::initialize(&workspace.path, id)?;
        match renameat_with(
            &parent_file,
            workspace.name.as_str(),
            &parent_file,
            name,
            RenameFlags::NOREPLACE,
        ) {
            Ok(()) => workspace.published = true,
            Err(Errno::EXIST | Errno::NOTEMPTY) => return Err(LibraryError::DestinationExists),
            Err(error)
                if error == Errno::NOSYS || error == Errno::INVAL || error == Errno::OPNOTSUPP =>
            {
                return Err(LibraryError::AtomicPublicationUnsupported(error.into()));
            }
            Err(error) => return Err(LibraryError::Publication(error.into())),
        }
        parent_file
            .sync_all()
            .map_err(|error| LibraryError::Published(Box::new(LibraryError::Io(error))))?;
        Self::open(&destination).map_err(|error| LibraryError::Published(Box::new(error)))
    }

    fn initialize(root: &Path, id: LibraryId) -> Result<(), LibraryError> {
        fs::create_dir(root.join("recovery")).map_err(LibraryError::Io)?;
        let object_store = ObjectStore::new(root).map_err(LibraryError::Store)?;
        let database = LibraryDatabase::create(root, id).map_err(LibraryError::Database)?;
        database.close().map_err(LibraryError::Database)?;
        drop(object_store);
        LibraryManifest::new(id)
            .write_new(root)
            .map_err(LibraryError::Manifest)
    }

    pub fn open(root: &Path) -> Result<Self, LibraryError> {
        let manifest = LibraryManifest::read(root).map_err(LibraryError::Manifest)?;
        let database =
            LibraryDatabase::open(root, manifest.library_id).map_err(LibraryError::Database)?;
        let object_store = ObjectStore::new(root).map_err(LibraryError::Store)?;
        Ok(Self {
            id: manifest.library_id,
            database,
            object_store,
        })
    }
}

fn destination_name(root: &Path) -> Result<&std::ffi::OsStr, LibraryError> {
    let bytes = root.as_os_str().as_bytes();
    let final_component = bytes
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default();
    if final_component.is_empty() || final_component == b"." || final_component == b".." {
        return Err(LibraryError::InvalidDestination(
            "missing usable final name",
        ));
    }
    let parent = root
        .parent()
        .ok_or(LibraryError::InvalidDestination("missing parent"))?;
    if parent.as_os_str().is_empty() {
        return Err(LibraryError::InvalidDestination("explicit parent required"));
    }
    root.file_name().ok_or(LibraryError::InvalidDestination(
        "missing usable final name",
    ))
}

struct CreationWorkspace {
    path: PathBuf,
    name: String,
    published: bool,
}

impl CreationWorkspace {
    fn new(parent: &Path) -> io::Result<Self> {
        for _ in 0..10 {
            let name = format!(".pigoune-create-{}", Uuid::new_v4());
            let path = parent.join(&name);
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        name,
                        published: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique creation directory",
        ))
    }
}

impl Drop for CreationWorkspace {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CreationWorkspace, Library, LibraryError, LibraryId};

    #[test]
    fn abandoned_creation_workspace_is_removed() {
        let parent = tempfile::tempdir().unwrap();
        let path = {
            let workspace = CreationWorkspace::new(parent.path()).unwrap();
            let path = workspace.path.clone();
            std::fs::write(path.join("partial"), b"bytes").unwrap();
            path
        };
        assert!(!path.exists());
    }

    #[test]
    fn failed_initialization_removes_only_its_workspace() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("library");
        let workspace_path = {
            let workspace = CreationWorkspace::new(parent.path()).unwrap();
            std::fs::write(workspace.path.join("recovery"), b"collision").unwrap();
            assert!(matches!(
                Library::initialize(&workspace.path, LibraryId::new()),
                Err(LibraryError::Io(_))
            ));
            workspace.path.clone()
        };
        assert!(!workspace_path.exists());
        assert!(!destination.exists());
    }
}
