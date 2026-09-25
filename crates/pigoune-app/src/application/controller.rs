use super::config::{ConfigError, ConfigStore, LibraryLocator};
use pigoune_core::{DatabaseError, Library, LibraryError, LibraryId, ManifestError, StoreError};
use std::{
    fs, io,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
};

#[derive(Clone, Copy, Debug)]
pub enum ErrorKind {
    LocationUnavailable,
    StorageUnavailable,
    NotALibrary,
    InvalidLibrary,
    IncompatibleVersion,
    InvalidConfiguration,
    PersistenceFailed,
    ForgetDurabilityUncertain,
    DestinationExists,
}

#[derive(Clone, Debug)]
pub struct OpenInfo {
    pub id: LibraryId,
    pub display_name: String,
    pub locator: LibraryLocator,
    pub persistence_warning: Option<String>,
}

#[derive(Clone, Debug)]
pub struct OpenError {
    pub kind: ErrorKind,
    pub diagnostic: String,
    pub configured: bool,
    pub can_retry: bool,
    pub can_open_managed: bool,
    pub has_session: bool,
}

#[derive(Clone, Debug)]
pub enum ApplicationState {
    Welcome,
    Opening,
    Open(OpenInfo),
    OpenError(OpenError),
}

#[derive(Clone, Debug)]
pub enum Command {
    Startup,
    CreateManagedDefault,
    OpenManagedDefault,
    CreateExternal {
        locator: LibraryLocator,
        path: PathBuf,
    },
    OpenExternal {
        locator: LibraryLocator,
        path: PathBuf,
    },
    RetryOpen,
    RetryPersistConfiguration,
    ForgetConfigured,
    ReturnToOpen,
}

struct LibrarySession {
    library: Library,
    locator: LibraryLocator,
    path: PathBuf,
    persistence_warning: Option<String>,
}

impl LibrarySession {
    fn present(&self) -> ApplicationState {
        let display_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("")
            .to_owned();
        ApplicationState::Open(OpenInfo {
            id: self.library.id,
            display_name,
            locator: self.locator.clone(),
            persistence_warning: self.persistence_warning.clone(),
        })
    }
}

pub struct LibraryController {
    store: ConfigStore,
    session: Option<LibrarySession>,
    last_open: Option<(LibraryLocator, PathBuf, bool)>,
}

impl LibraryController {
    pub fn new(store: ConfigStore) -> Self {
        Self {
            store,
            session: None,
            last_open: None,
        }
    }

    pub fn start(store: ConfigStore) -> (Sender<Command>, Receiver<ApplicationState>) {
        let (commands, incoming) = mpsc::channel();
        let (outgoing, states) = mpsc::channel();
        std::thread::spawn(move || {
            let mut controller = Self::new(store);
            while let Ok(command) = incoming.recv() {
                for state in controller.handle(command) {
                    if outgoing.send(state).is_err() {
                        return;
                    }
                }
            }
        });
        (commands, states)
    }

    pub fn handle(&mut self, command: Command) -> Vec<ApplicationState> {
        match command {
            Command::Startup => match self.store.load() {
                Ok(None) => vec![ApplicationState::Welcome],
                Ok(Some(locator)) => self.open_locator(locator, false),
                Err(error) => vec![self.config_error(error)],
            },
            Command::CreateManagedDefault => {
                let locator = LibraryLocator::ManagedDefault;
                let path = self.store.managed_path();
                if let Err(error) = fs::create_dir_all(self.store.managed_parent()) {
                    return vec![self.error(
                        ErrorKind::LocationUnavailable,
                        error.to_string(),
                        false,
                    )];
                }
                self.create(locator, path)
            }
            Command::OpenManagedDefault => self.open_locator(LibraryLocator::ManagedDefault, true),
            Command::CreateExternal { locator, path } => self.create(locator, path),
            Command::OpenExternal { locator, path } => self.open_path(locator, path, true),
            Command::RetryOpen => match self.last_open.clone() {
                Some((locator, path, persist)) => self.open_path(locator, path, persist),
                None => self.handle(Command::Startup),
            },
            Command::RetryPersistConfiguration => {
                let Some(session) = self.session.as_mut() else {
                    return vec![ApplicationState::Welcome];
                };
                session.persistence_warning = self
                    .store
                    .save(&session.locator)
                    .err()
                    .map(|error| error.to_string());
                vec![session.present()]
            }
            Command::ForgetConfigured => match self.store.forget() {
                Ok(()) => {
                    self.session = None;
                    self.last_open = None;
                    vec![ApplicationState::Welcome]
                }
                Err(error) => {
                    let kind = classify_forget_error(&error);
                    let mut state = self.error(kind, error.to_string(), false);
                    if let ApplicationState::OpenError(ref mut details) = state {
                        details.configured = true;
                        details.can_retry = false;
                    }
                    vec![state]
                }
            },
            Command::ReturnToOpen => vec![
                self.session
                    .as_ref()
                    .map_or(ApplicationState::Welcome, LibrarySession::present),
            ],
        }
    }

    fn create(&mut self, locator: LibraryLocator, path: PathBuf) -> Vec<ApplicationState> {
        self.last_open = None;
        let mut states = vec![ApplicationState::Opening];
        match Library::create(&path) {
            Ok(library) => states.push(self.install_session(library, locator, path, true)),
            Err(error @ LibraryError::Published(_)) => {
                self.last_open = Some((locator, path, true));
                states.push(self.error(
                    classify_library_error(&error),
                    format!("library publication may have completed; try opening the existing destination: {error}"),
                    false,
                ));
            }
            Err(LibraryError::DestinationExists) => {
                let can_open_managed = locator == LibraryLocator::ManagedDefault;
                self.last_open = Some((locator, path, true));
                states.push(self.error(
                    ErrorKind::DestinationExists,
                    "library destination already exists".into(),
                    can_open_managed,
                ));
            }
            Err(error) => states.push(self.library_error(error, false)),
        }
        states
    }

    fn open_locator(&mut self, locator: LibraryLocator, persist: bool) -> Vec<ApplicationState> {
        match locator.resolve_path(&self.store) {
            Ok(path) => self.open_path(locator, path, persist),
            Err(error) => {
                let mut state =
                    self.error(ErrorKind::LocationUnavailable, error.to_string(), false);
                if let ApplicationState::OpenError(ref mut details) = state {
                    details.configured = !persist;
                    details.can_retry = !persist;
                }
                vec![state]
            }
        }
    }

    fn open_path(
        &mut self,
        locator: LibraryLocator,
        path: PathBuf,
        persist: bool,
    ) -> Vec<ApplicationState> {
        self.last_open = Some((locator.clone(), path.clone(), persist));
        let mut states = vec![ApplicationState::Opening];
        match Library::open(&path) {
            Ok(library) => states.push(self.install_session(library, locator, path, persist)),
            Err(error) => {
                let mut state = if matches!(fs::metadata(&path), Err(ref io) if io.kind() == io::ErrorKind::NotFound || io.kind() == io::ErrorKind::PermissionDenied)
                {
                    self.error(ErrorKind::LocationUnavailable, error.to_string(), false)
                } else if matches!(fs::metadata(path.join("library.db")), Err(ref io) if io.kind() == io::ErrorKind::NotFound)
                    && path.join("library.json").exists()
                {
                    self.error(ErrorKind::NotALibrary, error.to_string(), false)
                } else {
                    self.library_error(error, false)
                };
                if let ApplicationState::OpenError(ref mut details) = state {
                    details.configured = !persist;
                }
                states.push(state);
            }
        }
        states
    }

    fn install_session(
        &mut self,
        library: Library,
        locator: LibraryLocator,
        path: PathBuf,
        persist: bool,
    ) -> ApplicationState {
        let warning = if persist {
            self.store
                .save(&locator)
                .err()
                .map(|error| error.to_string())
        } else {
            None
        };
        self.session = Some(LibrarySession {
            library,
            locator,
            path,
            persistence_warning: warning,
        });
        self.session.as_ref().unwrap().present()
    }

    fn config_error(&self, error: ConfigError) -> ApplicationState {
        let kind = match error {
            ConfigError::Io(ref io)
                if io.kind() == io::ErrorKind::NotFound
                    || io.kind() == io::ErrorKind::PermissionDenied =>
            {
                ErrorKind::LocationUnavailable
            }
            _ => ErrorKind::InvalidConfiguration,
        };
        let mut state = self.error(kind, error.to_string(), false);
        if let ApplicationState::OpenError(ref mut details) = state {
            details.configured = true;
            details.can_retry = true;
        }
        state
    }

    fn library_error(&self, error: LibraryError, can_open_managed: bool) -> ApplicationState {
        let kind = classify_library_error(&error);
        self.error(kind, error.to_string(), can_open_managed)
    }

    fn error(
        &self,
        kind: ErrorKind,
        diagnostic: String,
        can_open_managed: bool,
    ) -> ApplicationState {
        ApplicationState::OpenError(OpenError {
            kind,
            diagnostic,
            configured: false,
            can_retry: self.last_open.is_some(),
            can_open_managed,
            has_session: self.session.is_some(),
        })
    }
}

fn classify_io(error: &io::Error) -> ErrorKind {
    match error.kind() {
        io::ErrorKind::NotFound
        | io::ErrorKind::PermissionDenied
        | io::ErrorKind::NotADirectory => ErrorKind::LocationUnavailable,
        _ => ErrorKind::StorageUnavailable,
    }
}

fn classify_forget_error(error: &ConfigError) -> ErrorKind {
    match error {
        ConfigError::DurabilityUncertain(_) => ErrorKind::ForgetDurabilityUncertain,
        _ => ErrorKind::PersistenceFailed,
    }
}

fn classify_library_error(error: &LibraryError) -> ErrorKind {
    match error {
        LibraryError::Manifest(ManifestError::Io(error))
            if error.kind() == io::ErrorKind::NotFound =>
        {
            ErrorKind::NotALibrary
        }
        LibraryError::Manifest(ManifestError::Io(error)) => classify_io(error),
        LibraryError::Manifest(ManifestError::UnsupportedFormatVersion(_)) => {
            ErrorKind::IncompatibleVersion
        }
        LibraryError::Manifest(_) => ErrorKind::InvalidLibrary,
        LibraryError::Database(DatabaseError::SQLiteTooOld(_) | DatabaseError::SchemaTooNew(_)) => {
            ErrorKind::IncompatibleVersion
        }
        LibraryError::Database(DatabaseError::UninitializedDatabase) => ErrorKind::NotALibrary,
        LibraryError::Database(DatabaseError::InvalidJournalMode(_)) => {
            ErrorKind::StorageUnavailable
        }
        LibraryError::Database(DatabaseError::Io(error))
            if error.kind() == io::ErrorKind::NotFound =>
        {
            ErrorKind::NotALibrary
        }
        LibraryError::Database(DatabaseError::Io(error)) => classify_io(error),
        LibraryError::Database(DatabaseError::Sqlite(error)) => match error.sqlite_error_code() {
            Some(
                rusqlite::ErrorCode::SystemIoFailure
                | rusqlite::ErrorCode::DiskFull
                | rusqlite::ErrorCode::FileLockingProtocolFailed,
            ) => ErrorKind::StorageUnavailable,
            Some(
                rusqlite::ErrorCode::PermissionDenied
                | rusqlite::ErrorCode::ReadOnly
                | rusqlite::ErrorCode::CannotOpen
                | rusqlite::ErrorCode::DatabaseBusy
                | rusqlite::ErrorCode::DatabaseLocked,
            ) => ErrorKind::LocationUnavailable,
            _ => ErrorKind::InvalidLibrary,
        },
        LibraryError::DestinationExists => ErrorKind::DestinationExists,
        LibraryError::Io(error) | LibraryError::InvalidParent(error) => classify_io(error),
        LibraryError::Store(
            StoreError::Io(error) | StoreError::SourceIo(error) | StoreError::PublicationIo(error),
        ) => classify_io(error),
        LibraryError::Publication(error) => classify_io(error),
        LibraryError::AtomicPublicationUnsupported(_) => ErrorKind::StorageUnavailable,
        LibraryError::Published(error) => classify_library_error(error),
        _ => ErrorKind::InvalidLibrary,
    }
}

pub fn validate_folder_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;
    use gio::prelude::*;

    fn setup() -> (tempfile::TempDir, LibraryController) {
        let root = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(root.path(), root.path(), "test.app");
        (root, LibraryController::new(store))
    }

    #[test]
    fn startup_and_managed_lifecycle() {
        let (_root, mut controller) = setup();
        assert!(matches!(
            controller.handle(Command::Startup)[0],
            ApplicationState::Welcome
        ));
        assert!(matches!(
            controller.handle(Command::CreateManagedDefault)[..],
            [ApplicationState::Opening, ApplicationState::Open(_)]
        ));
        let path = controller.store.managed_path();
        assert!(path.join("library.json").exists());
        assert!(matches!(
            controller.handle(Command::CreateManagedDefault)[1],
            ApplicationState::OpenError(OpenError {
                kind: ErrorKind::DestinationExists,
                ..
            })
        ));
        assert!(path.exists());
        assert!(matches!(
            controller.handle(Command::OpenManagedDefault)[1],
            ApplicationState::Open(_)
        ));
        assert!(matches!(
            controller.handle(Command::Startup)[1],
            ApplicationState::Open(_)
        ));
        assert!(matches!(
            controller.handle(Command::ForgetConfigured)[0],
            ApplicationState::Welcome
        ));
        assert!(path.exists());
    }

    #[test]
    fn startup_reports_missing_invalid_and_new_schema() {
        let (_root, mut controller) = setup();
        controller
            .store
            .save(&LibraryLocator::ManagedDefault)
            .unwrap();
        assert!(matches!(
            controller.handle(Command::Startup)[1],
            ApplicationState::OpenError(OpenError {
                kind: ErrorKind::LocationUnavailable,
                ..
            })
        ));
        assert!(matches!(
            controller.handle(Command::RetryOpen)[1],
            ApplicationState::OpenError(OpenError {
                kind: ErrorKind::LocationUnavailable,
                ..
            })
        ));
        fs::write(controller.store.path(), "invalid").unwrap();
        assert!(matches!(
            controller.handle(Command::Startup)[0],
            ApplicationState::OpenError(OpenError {
                kind: ErrorKind::InvalidConfiguration,
                ..
            })
        ));
        controller
            .store
            .save(&LibraryLocator::ManagedDefault)
            .unwrap();
        fs::create_dir_all(controller.store.managed_parent()).unwrap();
        let path = controller.store.managed_path();
        let library = Library::create(&path).unwrap();
        drop(library);
        fs::write(path.join("library.json"), "invalid").unwrap();
        assert!(matches!(
            controller.handle(Command::Startup)[1],
            ApplicationState::OpenError(OpenError {
                kind: ErrorKind::InvalidLibrary,
                ..
            })
        ));
    }

    #[test]
    fn name_is_one_component() {
        for bad in ["", ".", "..", "a/b"] {
            assert!(!validate_folder_name(bad));
        }
        assert!(validate_folder_name("Ma bibliothèque"));
    }

    #[test]
    fn successful_open_persists_only_after_open() {
        let (root, mut controller) = setup();
        let path = root.path().join("external");
        let library = Library::create(&path).unwrap();
        let id = library.id;
        drop(library);
        let locator = LibraryLocator::FileUri {
            uri: gio::File::for_path(&path).uri().to_string(),
        };
        assert!(!controller.store.path().exists());
        let result = controller.handle(Command::OpenExternal {
            locator: locator.clone(),
            path: path.clone(),
        });
        assert!(matches!(&result[1], ApplicationState::Open(info) if info.id == id));
        assert_eq!(controller.store.load().unwrap(), Some(locator));
        assert!(path.join("library.json").exists());
    }

    #[test]
    fn config_failure_keeps_open_library_and_old_config() {
        let (root, mut controller) = setup();
        let old = LibraryLocator::ManagedDefault;
        controller.store.save(&old).unwrap();
        let path = root.path().join("external");
        let library = Library::create(&path).unwrap();
        let id = library.id;
        drop(library);
        let locator = LibraryLocator::FileUri {
            uri: gio::File::for_path(&path).uri().to_string(),
        };
        let config_dir = controller.store.path().parent().unwrap().to_path_buf();
        let permissions = fs::metadata(&config_dir).unwrap().permissions();
        let mut readonly = permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&config_dir, readonly).unwrap();
        let result = controller.handle(Command::OpenExternal {
            locator,
            path: path.clone(),
        });
        fs::set_permissions(&config_dir, permissions).unwrap();
        assert!(matches!(&result[1], ApplicationState::Open(info)
            if info.id == id && info.persistence_warning.is_some()));
        assert_eq!(controller.store.load().unwrap(), Some(old));
        assert!(path.join("library.json").exists());
    }

    #[test]
    fn newer_schema_is_incompatible() {
        let (_root, mut controller) = setup();
        fs::create_dir_all(controller.store.managed_parent()).unwrap();
        let path = controller.store.managed_path();
        let library = Library::create(&path).unwrap();
        drop(library);
        controller
            .store
            .save(&LibraryLocator::ManagedDefault)
            .unwrap();
        let connection = rusqlite::Connection::open(path.join("library.db")).unwrap();
        connection.pragma_update(None, "user_version", 99).unwrap();
        drop(connection);
        assert!(matches!(
            controller.handle(Command::Startup)[1],
            ApplicationState::OpenError(OpenError {
                kind: ErrorKind::IncompatibleVersion,
                ..
            })
        ));
    }

    #[test]
    fn external_creation_failure_keeps_previous_session_and_config() {
        let (root, mut controller) = setup();
        assert!(matches!(
            controller.handle(Command::CreateManagedDefault)[1],
            ApplicationState::Open(_)
        ));
        let original = controller.store.load().unwrap();
        let nonexistent_parent = root.path().join("missing");
        let result = controller.handle(Command::CreateExternal {
            locator: LibraryLocator::FileUri {
                uri: gio::File::for_path(nonexistent_parent.join("TEST"))
                    .uri()
                    .to_string(),
            },
            path: nonexistent_parent.join("TEST"),
        });
        assert!(matches!(
            &result[1],
            ApplicationState::OpenError(OpenError {
                kind: ErrorKind::LocationUnavailable,
                configured: false,
                can_retry: false,
                has_session: true,
                ..
            })
        ));
        assert_eq!(controller.store.load().unwrap(), original);
        assert!(matches!(
            controller.handle(Command::ReturnToOpen)[0],
            ApplicationState::Open(_)
        ));
    }

    #[test]
    fn sqlite_io_error_is_a_storage_error() {
        let error = LibraryError::Database(DatabaseError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
            None,
        )));
        assert!(matches!(
            classify_library_error(&error),
            ErrorKind::StorageUnavailable
        ));
    }

    #[test]
    fn filesystem_and_object_store_errors_are_not_library_corruption() {
        let io_error = || io::Error::other("storage I/O failed");
        let errors = [
            LibraryError::Io(io_error()),
            LibraryError::Store(StoreError::Io(io_error())),
            LibraryError::Store(StoreError::PublicationIo(io_error())),
            LibraryError::Publication(io_error()),
            LibraryError::AtomicPublicationUnsupported(io_error()),
            LibraryError::Database(DatabaseError::InvalidJournalMode("delete".into())),
            LibraryError::Published(Box::new(LibraryError::Io(io_error()))),
        ];
        for error in errors {
            assert!(
                matches!(
                    classify_library_error(&error),
                    ErrorKind::StorageUnavailable
                ),
                "unexpected classification for {error}"
            );
        }
        assert!(matches!(
            classify_library_error(&LibraryError::Store(StoreError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "access denied",
            )))),
            ErrorKind::LocationUnavailable
        ));
    }

    #[test]
    fn forgetting_distinguishes_an_unconfirmed_sync_from_a_failed_removal() {
        assert!(matches!(
            classify_forget_error(&ConfigError::DurabilityUncertain(io::Error::other(
                "directory sync failed"
            ))),
            ErrorKind::ForgetDurabilityUncertain
        ));
        assert!(matches!(
            classify_forget_error(&ConfigError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "removal failed"
            ))),
            ErrorKind::PersistenceFailed
        ));
    }
}
