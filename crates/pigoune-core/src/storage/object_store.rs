use crate::ObjectHash;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    ffi::OsStr,
    fmt, fs,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    SourceIo(io::Error),
    PublicationIo(io::Error),
    StagingModified {
        expected: ObjectHash,
        found: ObjectHash,
    },
    InvalidInternalPath(PathBuf),
    InconsistentStore(&'static str),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "object store I/O error: {error}"),
            Self::SourceIo(error) => write!(f, "source read error: {error}"),
            Self::PublicationIo(error) => write!(f, "object publication I/O error: {error}"),
            Self::StagingModified { expected, found } => {
                write!(
                    f,
                    "staged object changed: expected {expected}, found {found}"
                )
            }
            Self::InvalidInternalPath(path) => {
                write!(f, "invalid object store path: {}", path.display())
            }
            Self::InconsistentStore(reason) => write!(f, "inconsistent object store: {reason}"),
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) | Self::SourceIo(error) | Self::PublicationIo(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// An immutable physical object. Its path is relative to the library root.
/// The filename extension is informative and is not part of the object's identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectRecord {
    pub hash: ObjectHash,
    pub size: u64,
    pub relative_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreResult {
    pub object: ObjectRecord,
    pub reused: bool,
}

/// Validation is supplied by the caller. A successful result is a trust
/// boundary, not proof that the validator inspected the bytes correctly.
pub trait StagedValidator {
    type Output;
    type Error;

    fn validate(&self, staged: &StagedObject<'_>) -> Result<Self::Output, Self::Error>;
}

/// A complete, synchronized copy in `objects/.tmp`; dropping it removes the copy.
/// This type has no publication operation.
pub struct StagedObject<'store> {
    store: &'store ObjectStore,
    temporary: TemporaryFile,
    hash: ObjectHash,
    size: u64,
    extension: Option<String>,
}

impl<'store> StagedObject<'store> {
    pub fn hash(&self) -> ObjectHash {
        self.hash
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    /// Opens the internal staged bytes for reading, independently of the source.
    pub fn open_read(&self) -> io::Result<File> {
        File::open(&self.temporary.0)
    }

    pub fn validate_with<V: StagedValidator>(
        self,
        validator: &V,
    ) -> Result<ValidatedStagedObject<'store, V::Output>, V::Error> {
        let validation = validator.validate(&self)?;
        Ok(ValidatedStagedObject {
            staged: self,
            validation,
        })
    }

    fn publish(mut self) -> Result<StoreResult, StoreError> {
        let result = self.store.publish_staged(&mut self)?;
        Ok(result)
    }
}

/// A staged object whose caller-provided validator has succeeded.
/// Only this type can publish its bytes.
pub struct ValidatedStagedObject<'store, T> {
    staged: StagedObject<'store>,
    validation: T,
}

impl<T> ValidatedStagedObject<'_, T> {
    pub fn validation(&self) -> &T {
        &self.validation
    }

    pub fn publish(self) -> Result<PublishedObject<T>, StoreError> {
        let stored = self.staged.publish()?;
        Ok(PublishedObject {
            stored,
            validation: self.validation,
        })
    }
}

/// The durable physical object and the validator's result.
pub struct PublishedObject<T> {
    pub stored: StoreResult,
    pub validation: T,
}

/// Content-addressed physical storage within one library.
///
/// Incoming bytes remain in `objects/.tmp` until a caller-provided validator
/// succeeds. Publication rechecks their SHA-256 and does not read the source.
/// Identical bytes reuse the first physical representation, even when later
/// source names have different extensions. Published objects are never edited.
pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn new(library_root: impl AsRef<Path>) -> Result<Self, StoreError> {
        validate_directory(library_root.as_ref())?;
        let root = fs::canonicalize(library_root)?;
        ensure_directory(&root.join("objects"))?;
        ensure_directory(&root.join("objects/.tmp"))?;
        let lock_path = root.join("objects/.lock");
        let lock = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if !fs::symlink_metadata(&lock_path)?.file_type().is_file() {
                    return Err(StoreError::InvalidInternalPath(lock_path));
                }
                OpenOptions::new().write(true).open(lock_path)?
            }
            Err(error) => return Err(error.into()),
        };
        lock.sync_all()?;
        sync_directory(&root.join("objects"))?;
        Ok(Self { root })
    }

    pub fn stage_file(&self, source: impl AsRef<Path>) -> Result<StagedObject<'_>, StoreError> {
        let source = source.as_ref();
        let input = File::open(source).map_err(StoreError::SourceIo)?;
        self.stage_reader(input, source.file_name())
    }

    /// Copies a reader into the store without publishing it. Only a simple
    /// filename hint is used; no external source path is retained.
    pub fn stage_reader(
        &self,
        mut reader: impl Read,
        name_hint: Option<&OsStr>,
    ) -> Result<StagedObject<'_>, StoreError> {
        let extension = safe_extension(name_hint);
        let temporary_directory = self.root.join("objects/.tmp");
        validate_directory(&temporary_directory)?;
        let temporary_path = temporary_directory.join(Uuid::new_v4().to_string());
        let mut temporary = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        let cleanup = TemporaryFile::new(temporary_path.clone());

        let mut hasher = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = reader.read(&mut buffer).map_err(StoreError::SourceIo)?;
            if count == 0 {
                break;
            }
            temporary.write_all(&buffer[..count])?;
            hasher.update(&buffer[..count]);
            size += count as u64;
        }
        temporary.set_permissions(read_only_permissions(&temporary)?)?;
        temporary.sync_all()?;
        drop(temporary);

        Ok(StagedObject {
            store: self,
            temporary: cleanup,
            hash: ObjectHash::from_digest(hasher),
            size,
            extension,
        })
    }

    fn publish_staged(&self, staged: &mut StagedObject<'_>) -> Result<StoreResult, StoreError> {
        let hash = staged.hash;
        let temporary_directory = self.root.join("objects/.tmp");
        validate_directory(&temporary_directory).map_err(publication_error)?;
        let lock = self.lock().map_err(publication_error)?;
        let found = ObjectHash::from_reader(staged.open_read().map_err(StoreError::PublicationIo)?)
            .map_err(StoreError::PublicationIo)?;
        if found != hash {
            return Err(StoreError::StagingModified {
                expected: hash,
                found,
            });
        }
        if let Some(object) = self.find_locked(hash).map_err(publication_error)? {
            sync_directory(&self.root.join("objects").join(&hash.to_string()[..2]))
                .map_err(publication_error)?;
            staged.temporary.remove().map_err(publication_error)?;
            sync_directory(&temporary_directory).map_err(publication_error)?;
            return Ok(StoreResult {
                object,
                reused: true,
            });
        }

        let hash_text = hash.to_string();
        let shard = self.root.join("objects").join(&hash_text[..2]);
        ensure_directory(&shard).map_err(publication_error)?;
        let filename = match &staged.extension {
            Some(extension) => format!("{hash_text}.{extension}"),
            None => hash_text.clone(),
        };
        let relative_path = PathBuf::from("objects")
            .join(&hash_text[..2])
            .join(filename);
        let destination = self.root.join(&relative_path);

        // hard_link publishes atomically without the replacement behavior of rename.
        match fs::hard_link(&staged.temporary.0, &destination) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let object = self.find_locked(hash).map_err(publication_error)?.ok_or(
                    StoreError::InconsistentStore("destination exists but cannot be found by hash"),
                )?;
                sync_directory(&shard).map_err(publication_error)?;
                staged.temporary.remove().map_err(publication_error)?;
                sync_directory(&temporary_directory).map_err(publication_error)?;
                return Ok(StoreResult {
                    object,
                    reused: true,
                });
            }
            Err(error) => return Err(StoreError::PublicationIo(error)),
        }
        sync_directory(&shard).map_err(publication_error)?;
        staged.temporary.remove().map_err(publication_error)?;
        sync_directory(&temporary_directory).map_err(publication_error)?;
        drop(lock);

        Ok(StoreResult {
            object: ObjectRecord {
                hash,
                size: staged.size,
                relative_path,
            },
            reused: false,
        })
    }

    pub fn find(&self, hash: ObjectHash) -> Result<Option<ObjectRecord>, StoreError> {
        let _lock = self.lock()?;
        self.find_locked(hash)
    }

    pub fn contains(&self, hash: ObjectHash) -> Result<bool, StoreError> {
        Ok(self.find(hash)?.is_some())
    }

    fn lock(&self) -> Result<File, StoreError> {
        validate_directory(&self.root.join("objects"))?;
        let path = self.root.join("objects/.lock");
        if !fs::symlink_metadata(&path)?.file_type().is_file() {
            return Err(StoreError::InvalidInternalPath(path));
        }
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        file.lock()?;
        Ok(file)
    }

    fn find_locked(&self, hash: ObjectHash) -> Result<Option<ObjectRecord>, StoreError> {
        let hash_text = hash.to_string();
        let shard = self.root.join("objects").join(&hash_text[..2]);
        match fs::symlink_metadata(&shard) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
            Ok(_) => validate_directory(&shard)?,
        }

        let mut found = None;
        for entry in fs::read_dir(&shard)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                if name.to_string_lossy().starts_with(&hash_text) {
                    return Err(StoreError::InvalidInternalPath(entry.path()));
                }
                continue;
            };
            let is_candidate = name == hash_text
                || name
                    .strip_prefix(&hash_text)
                    .is_some_and(|suffix| suffix.starts_with('.'));
            if !is_candidate {
                continue;
            }
            if let Some(extension) = name.strip_prefix(&format!("{hash_text}."))
                && !is_safe_extension(extension)
            {
                return Err(StoreError::InvalidInternalPath(entry.path()));
            }
            if found.is_some() {
                return Err(StoreError::InconsistentStore(
                    "multiple objects have the same hash",
                ));
            }
            let metadata = fs::symlink_metadata(entry.path())?;
            if !metadata.file_type().is_file() {
                return Err(StoreError::InvalidInternalPath(entry.path()));
            }
            let file = File::open(entry.path())?;
            if ObjectHash::from_reader(file)? != hash {
                return Err(StoreError::InconsistentStore(
                    "object bytes do not match its hash",
                ));
            }
            found = Some(ObjectRecord {
                hash,
                size: metadata.len(),
                relative_path: PathBuf::from("objects").join(&hash_text[..2]).join(name),
            });
        }
        Ok(found)
    }
}

fn safe_extension(name_hint: Option<&OsStr>) -> Option<String> {
    let name = name_hint?;
    let path = Path::new(name);
    if path.file_name() != Some(name) {
        return None;
    }
    let extension = path.extension()?.to_str()?;
    is_safe_extension(extension).then(|| extension.to_ascii_lowercase())
}

fn publication_error(error: StoreError) -> StoreError {
    match error {
        StoreError::Io(error) => StoreError::PublicationIo(error),
        other => other,
    }
}

fn is_safe_extension(extension: &str) -> bool {
    !extension.is_empty()
        && extension.len() <= 16
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn validate_directory(path: &Path) -> Result<(), StoreError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() {
        return Err(StoreError::InvalidInternalPath(path.to_path_buf()));
    }
    Ok(())
}

fn ensure_directory(path: &Path) -> Result<(), StoreError> {
    match fs::create_dir(path) {
        Ok(()) => sync_directory(
            path.parent()
                .ok_or(StoreError::InconsistentStore("missing parent directory"))?,
        )?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    validate_directory(path)
}

fn sync_directory(path: &Path) -> Result<(), StoreError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

fn read_only_permissions(file: &File) -> Result<fs::Permissions, StoreError> {
    let mut permissions = file.metadata()?.permissions();
    permissions.set_readonly(true);
    Ok(permissions)
}

struct TemporaryFile(PathBuf);

impl TemporaryFile {
    fn new(path: PathBuf) -> Self {
        Self(path)
    }

    fn remove(&mut self) -> Result<(), StoreError> {
        fs::remove_file(&self.0)?;
        self.0.clear();
        Ok(())
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ObjectStore, StagedObject, StagedValidator, StoreError};
    use std::{fs, io, io::Read, os::unix::fs::PermissionsExt};
    use tempfile::tempdir;

    struct AcceptStagedBytes;

    impl StagedValidator for AcceptStagedBytes {
        type Output = Vec<u8>;
        type Error = io::Error;

        fn validate(&self, staged: &StagedObject<'_>) -> Result<Self::Output, Self::Error> {
            let mut bytes = Vec::new();
            staged.open_read()?.read_to_end(&mut bytes)?;
            Ok(bytes)
        }
    }

    #[test]
    fn changed_staging_is_rejected_before_publication() {
        let library = tempdir().unwrap();
        let store = ObjectStore::new(library.path()).unwrap();
        let validated = store
            .stage_reader(&b"original"[..], None)
            .unwrap()
            .validate_with(&AcceptStagedBytes)
            .unwrap();
        let path = &validated.staged.temporary.0;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions).unwrap();
        fs::write(path, b"tampered").unwrap();
        assert!(matches!(
            validated.publish(),
            Err(StoreError::StagingModified { .. })
        ));
        assert_eq!(
            fs::read_dir(library.path().join("objects/.tmp"))
                .unwrap()
                .count(),
            0
        );
        assert!(
            !store
                .contains(crate::ObjectHash::from_bytes(b"original"))
                .unwrap()
        );
        assert!(
            !store
                .contains(crate::ObjectHash::from_bytes(b"tampered"))
                .unwrap()
        );
    }
}
