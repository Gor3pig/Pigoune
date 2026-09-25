use gio::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    fs::File,
    fs::OpenOptions,
    io,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum LibraryLocator {
    ManagedDefault,
    FileUri { uri: String },
}

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    DurabilityUncertain(io::Error),
    Json(serde_json::Error),
    InvalidType,
    UnsupportedVersion,
    InvalidLocator,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "configuration I/O error: {error}"),
            Self::DurabilityUncertain(error) => {
                write!(f, "configuration change could not be synchronized: {error}")
            }
            Self::Json(error) => write!(f, "invalid configuration JSON: {error}"),
            Self::InvalidType => f.write_str("invalid configuration type"),
            Self::UnsupportedVersion => f.write_str("unsupported configuration version"),
            Self::InvalidLocator => f.write_str("invalid local library URI"),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AppConfig {
    #[serde(rename = "type")]
    document_type: String,
    version: u32,
    library: LibraryLocator,
}

pub struct ConfigStore {
    config_dir: PathBuf,
    data_dir: PathBuf,
}

impl ConfigStore {
    pub fn new(config_root: &Path, data_root: &Path, application_id: &str) -> Self {
        Self {
            config_dir: config_root.join(application_id),
            data_dir: data_root.join(application_id),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.config_dir.join("config.json")
    }
    pub fn managed_path(&self) -> PathBuf {
        self.data_dir.join("library")
    }
    pub fn managed_parent(&self) -> &Path {
        &self.data_dir
    }

    pub fn load(&self) -> Result<Option<LibraryLocator>, ConfigError> {
        let file = match File::open(self.path()) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ConfigError::Io(error)),
        };
        let config: AppConfig = serde_json::from_reader(file).map_err(ConfigError::Json)?;
        if config.document_type != "pigoune-app-config" {
            return Err(ConfigError::InvalidType);
        }
        if config.version != 1 {
            return Err(ConfigError::UnsupportedVersion);
        }
        config.library.resolve_path(self)?;
        Ok(Some(config.library))
    }

    pub fn save(&self, library: &LibraryLocator) -> Result<(), ConfigError> {
        library.resolve_path(self)?;
        fs::create_dir_all(&self.config_dir).map_err(ConfigError::Io)?;
        sync_ancestor_directories(&self.config_dir).map_err(ConfigError::Io)?;
        let config = AppConfig {
            document_type: "pigoune-app-config".into(),
            version: 1,
            library: library.clone(),
        };
        let mut bytes = serde_json::to_vec_pretty(&config).map_err(ConfigError::Json)?;
        bytes.push(b'\n');
        for _ in 0..10 {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let temp = self
                .config_dir
                .join(format!(".config-{}-{serial}.tmp", std::process::id()));
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&temp) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(ConfigError::Io(error)),
            };
            let result = (|| {
                file.write_all(&bytes)?;
                file.sync_all()?;
                drop(file);
                fs::rename(&temp, self.path())?;
                Ok::<(), io::Error>(())
            })();
            if result.is_err() {
                let _ = fs::remove_file(&temp);
            }
            result.map_err(ConfigError::Io)?;
            return sync_directory(&self.config_dir).map_err(ConfigError::DurabilityUncertain);
        }
        Err(ConfigError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "cannot allocate configuration temporary file",
        )))
    }

    pub fn forget(&self) -> Result<(), ConfigError> {
        self.forget_with_sync(sync_directory)
    }

    fn forget_with_sync(
        &self,
        sync: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<(), ConfigError> {
        let removed = match fs::remove_file(self.path()) {
            Ok(()) => true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(ConfigError::Io(error)),
        };
        match sync(&self.config_dir) {
            Ok(()) => Ok(()),
            Err(error) if !removed && error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ConfigError::DurabilityUncertain(error)),
        }
    }
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

fn sync_ancestor_directories(path: &Path) -> io::Result<()> {
    let mut current = path;
    while let Some(parent) = current.parent() {
        sync_directory(parent)?;
        current = parent;
    }
    Ok(())
}

impl LibraryLocator {
    pub fn resolve_path(&self, store: &ConfigStore) -> Result<PathBuf, ConfigError> {
        match self {
            Self::ManagedDefault => Ok(store.managed_path()),
            Self::FileUri { uri } => {
                if !uri.starts_with("file:") {
                    return Err(ConfigError::InvalidLocator);
                }
                gio::File::for_uri(uri)
                    .path()
                    .ok_or(ConfigError::InvalidLocator)
            }
        }
    }

    pub fn from_file(file: &gio::File) -> Result<(Self, PathBuf), ConfigError> {
        let uri = file.uri().to_string();
        let locator = Self::FileUri { uri };
        let path = file.path().ok_or(ConfigError::InvalidLocator)?;
        if !file
            .uri_scheme()
            .as_deref()
            .is_some_and(|scheme| scheme == "file")
        {
            return Err(ConfigError::InvalidLocator);
        }
        Ok((locator, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_and_forgets() {
        let root = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(root.path(), root.path(), "test.app");
        assert_eq!(store.load().unwrap(), None);
        store.save(&LibraryLocator::ManagedDefault).unwrap();
        assert_eq!(store.load().unwrap(), Some(LibraryLocator::ManagedDefault));
        let content = fs::read_to_string(store.path()).unwrap();
        assert!(!content.contains(root.path().to_str().unwrap()));
        let path = root.path().join("external");
        let locator = LibraryLocator::FileUri {
            uri: gio::File::for_path(&path).uri().to_string(),
        };
        store.save(&locator).unwrap();
        assert_eq!(store.load().unwrap(), Some(locator));
        store.forget().unwrap();
        store.forget().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn rejects_invalid_documents() {
        let root = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(root.path(), root.path(), "test.app");
        fs::create_dir_all(&store.config_dir).unwrap();
        for content in [
            "not json",
            r#"{"type":"other","version":1,"library":{"kind":"managed-default"}}"#,
            r#"{"type":"pigoune-app-config","version":2,"library":{"kind":"managed-default"}}"#,
            r#"{"type":"pigoune-app-config","version":1,"library":{"kind":"unknown"}}"#,
            r#"{"type":"pigoune-app-config","version":1,"library":{"kind":"file-uri","uri":"https://example.org"}}"#,
        ] {
            fs::write(store.path(), content).unwrap();
            assert!(store.load().is_err());
        }
    }

    #[test]
    fn managed_path_depends_on_application_id() {
        let root = tempfile::tempdir().unwrap();
        let a = ConfigStore::new(root.path(), root.path(), "app.a");
        let b = ConfigStore::new(root.path(), root.path(), "app.b");
        assert_eq!(a.managed_path(), root.path().join("app.a/library"));
        assert_ne!(a.managed_path(), b.managed_path());
    }

    #[test]
    fn local_file_uri_resolves_to_the_gfile_path() {
        let root = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(root.path(), root.path(), "test.app");
        let path = root.path().join("bibliothèque");
        let file = gio::File::for_path(&path);
        let (locator, selected_path) = LibraryLocator::from_file(&file).unwrap();
        assert_eq!(selected_path, path);
        assert_eq!(locator.resolve_path(&store).unwrap(), selected_path);
        assert!(matches!(
            LibraryLocator::FileUri {
                uri: "https://example.org/library".into()
            }
            .resolve_path(&store),
            Err(ConfigError::InvalidLocator)
        ));
    }

    #[test]
    fn forget_retries_directory_sync_after_the_file_was_removed() {
        let root = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(root.path(), root.path(), "test.app");
        store.save(&LibraryLocator::ManagedDefault).unwrap();

        let error = store
            .forget_with_sync(|_| Err(io::Error::other("directory sync failed")))
            .unwrap_err();
        assert!(matches!(error, ConfigError::DurabilityUncertain(_)));
        assert!(!store.path().exists());

        store.forget().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn creates_and_synchronizes_a_new_configuration_directory() {
        let root = tempfile::tempdir().unwrap();
        let config_root = root.path().join("nested/config");
        let store = ConfigStore::new(&config_root, root.path(), "test.app");
        store.save(&LibraryLocator::ManagedDefault).unwrap();
        assert_eq!(store.load().unwrap(), Some(LibraryLocator::ManagedDefault));
    }
}
