use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use pigoune_core::{
    CACHE_DIR_NAME, CURRENT_FORMAT_VERSION, DATABASE_FILE_NAME, FILES_DIR_NAME, Library,
    LibraryError,
};
use rusqlite::Connection;
use tempfile::TempDir;

fn workspace() -> TempDir {
    tempfile::tempdir().expect("temporary directory")
}

fn entries_of(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("readable directory")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn table_names(database_path: &Path) -> Vec<String> {
    let connection = Connection::open(database_path).expect("database opens");
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
        .expect("query prepares");
    statement
        .query_map([], |row| row.get(0))
        .expect("query runs")
        .collect::<Result<_, _>>()
        .expect("rows read")
}

fn set_format_version(database_path: &Path, version: u32) {
    let connection = Connection::open(database_path).expect("database opens");
    connection
        .pragma_update(None, "user_version", version)
        .expect("version updated");
}

#[test]
fn creating_a_library_builds_the_expected_folder_layout() {
    let workspace = workspace();

    let library = Library::create(workspace.path(), "Mes logos").expect("library is created");

    assert_eq!(library.root(), workspace.path().join("Mes logos.pigoune"));
    assert_eq!(
        entries_of(library.root()),
        [CACHE_DIR_NAME, FILES_DIR_NAME, DATABASE_FILE_NAME]
    );
}

#[test]
fn a_new_library_uses_the_current_format_version() {
    let workspace = workspace();

    let library = Library::create(workspace.path(), "Mes logos").expect("library is created");

    assert_eq!(
        library.format_version().expect("version is readable"),
        CURRENT_FORMAT_VERSION
    );
}

#[test]
fn a_new_library_contains_every_table_of_the_format() {
    let workspace = workspace();
    let library = Library::create(workspace.path(), "Mes logos").expect("library is created");
    let database_path = library.root().join(DATABASE_FILE_NAME);
    drop(library);

    assert_eq!(
        table_names(&database_path),
        [
            "asset_collections",
            "asset_tags",
            "assets",
            "collections",
            "tags"
        ]
    );
}

#[test]
fn the_library_name_comes_from_its_folder() {
    let workspace = workspace();

    let library = Library::create(workspace.path(), "  Travail  ").expect("library is created");

    assert_eq!(library.name(), "Travail");
}

#[test]
fn a_name_already_ending_with_the_extension_is_not_doubled() {
    let workspace = workspace();

    let library = Library::create(workspace.path(), "Perso.pigoune").expect("library is created");

    assert_eq!(library.root(), workspace.path().join("Perso.pigoune"));
}

#[test]
fn invalid_names_are_refused_without_creating_anything() {
    let workspace = workspace();
    let too_long = "a".repeat(201);

    for name in [
        "", "   ", ".", "..", ".cachee", "a/b", "nul\0", ".pigoune", &too_long,
    ] {
        let result = Library::create(workspace.path(), name);
        assert!(
            matches!(result, Err(LibraryError::InvalidName)),
            "name {name:?} should be refused"
        );
    }
    assert!(entries_of(workspace.path()).is_empty());
}

#[test]
fn creating_over_an_existing_folder_is_refused_and_leaves_it_untouched() {
    let workspace = workspace();
    let existing = workspace.path().join("Mes logos.pigoune");
    fs::create_dir(&existing).expect("folder created");
    fs::write(existing.join("precious.txt"), "keep me").expect("file written");

    let result = Library::create(workspace.path(), "Mes logos");

    assert!(matches!(result, Err(LibraryError::AlreadyExists(_))));
    assert_eq!(entries_of(&existing), ["precious.txt"]);
}

#[test]
fn creating_in_a_missing_folder_is_refused() {
    let workspace = workspace();

    let result = Library::create(&workspace.path().join("absent"), "Mes logos");

    assert!(matches!(result, Err(LibraryError::NotFound(_))));
}

#[test]
fn a_failed_creation_leaves_no_partial_folder_behind() {
    let workspace = workspace();
    let read_only_parent = workspace.path().join("lecture-seule");
    fs::create_dir(&read_only_parent).expect("folder created");
    fs::set_permissions(&read_only_parent, fs::Permissions::from_mode(0o555))
        .expect("permissions set");

    let result = Library::create(&read_only_parent, "Mes logos");

    fs::set_permissions(&read_only_parent, fs::Permissions::from_mode(0o755))
        .expect("permissions restored");
    assert!(matches!(result, Err(LibraryError::PermissionDenied)));
    assert!(entries_of(&read_only_parent).is_empty());
}

#[test]
fn a_created_library_can_be_reopened() {
    let workspace = workspace();
    let root = Library::create(workspace.path(), "Mes logos")
        .expect("library is created")
        .root()
        .to_path_buf();

    let reopened = Library::open(&root).expect("library reopens");

    assert_eq!(reopened.name(), "Mes logos");
}

#[test]
fn a_moved_library_still_opens() {
    let workspace = workspace();
    let original_root = Library::create(workspace.path(), "Mes logos")
        .expect("library is created")
        .root()
        .to_path_buf();
    let elsewhere = workspace.path().join("disque-externe");
    fs::create_dir(&elsewhere).expect("folder created");
    let moved_root = elsewhere.join("Mes logos.pigoune");
    fs::rename(&original_root, &moved_root).expect("library moved");

    let reopened = Library::open(&moved_root).expect("moved library opens");

    assert_eq!(reopened.root(), moved_root);
}

#[test]
fn opening_a_missing_folder_reports_not_found() {
    let workspace = workspace();

    let result = Library::open(&workspace.path().join("absent.pigoune"));

    assert!(matches!(result, Err(LibraryError::NotFound(_))));
}

#[test]
fn opening_an_ordinary_folder_reports_not_a_library() {
    let workspace = workspace();

    let result = Library::open(workspace.path());

    assert!(matches!(result, Err(LibraryError::NotALibrary(_))));
}

#[test]
fn opening_a_folder_with_a_foreign_database_reports_not_a_library() {
    let workspace = workspace();
    let foreign = Connection::open(workspace.path().join(DATABASE_FILE_NAME)).expect("opens");
    foreign
        .execute_batch("CREATE TABLE notes (text TEXT)")
        .expect("foreign table created");
    drop(foreign);

    let result = Library::open(workspace.path());

    assert!(matches!(result, Err(LibraryError::NotALibrary(_))));
}

#[test]
fn opening_a_folder_with_a_corrupt_database_reports_not_a_library() {
    let workspace = workspace();
    fs::write(
        workspace.path().join(DATABASE_FILE_NAME),
        "this is definitely not a SQLite database, just some random text",
    )
    .expect("file written");

    let result = Library::open(workspace.path());

    assert!(matches!(result, Err(LibraryError::NotALibrary(_))));
}

#[test]
fn a_library_from_a_newer_pigoune_is_refused_and_left_untouched() {
    let workspace = workspace();
    let root = Library::create(workspace.path(), "Mes logos")
        .expect("library is created")
        .root()
        .to_path_buf();
    let database_path = root.join(DATABASE_FILE_NAME);
    set_format_version(&database_path, CURRENT_FORMAT_VERSION + 1);
    let bytes_before = fs::read(&database_path).expect("database readable");

    let result = Library::open(&root);

    assert!(matches!(
        result,
        Err(LibraryError::NewerFormat { found, supported })
            if found == CURRENT_FORMAT_VERSION + 1 && supported == CURRENT_FORMAT_VERSION
    ));
    assert_eq!(
        fs::read(&database_path).expect("database readable"),
        bytes_before
    );
}

#[test]
fn a_library_cannot_be_opened_twice_at_the_same_time() {
    let workspace = workspace();
    let library = Library::create(workspace.path(), "Mes logos").expect("library is created");

    let second_opening = Library::open(library.root());

    assert!(matches!(second_opening, Err(LibraryError::InUse)));
}

#[test]
fn a_closed_library_can_be_opened_again() {
    let workspace = workspace();
    let library = Library::create(workspace.path(), "Mes logos").expect("library is created");
    let root = library.root().to_path_buf();
    drop(library);

    assert!(Library::open(&root).is_ok());
}

#[test]
fn a_deleted_cache_folder_is_recreated_on_opening() {
    let workspace = workspace();
    let root = Library::create(workspace.path(), "Mes logos")
        .expect("library is created")
        .root()
        .to_path_buf();
    fs::remove_dir_all(root.join(CACHE_DIR_NAME)).expect("cache removed");

    let reopened = Library::open(&root).expect("library reopens");

    assert!(reopened.root().join(CACHE_DIR_NAME).is_dir());
}

#[test]
fn a_read_only_library_is_refused_with_a_permission_error() {
    let workspace = workspace();
    let root = Library::create(workspace.path(), "Mes logos")
        .expect("library is created")
        .root()
        .to_path_buf();
    let database_path = root.join(DATABASE_FILE_NAME);
    fs::set_permissions(&database_path, fs::Permissions::from_mode(0o444))
        .expect("permissions set");

    let result = Library::open(&root);

    fs::set_permissions(&database_path, fs::Permissions::from_mode(0o644))
        .expect("permissions restored");
    assert!(matches!(result, Err(LibraryError::PermissionDenied)));
}

#[test]
fn a_library_with_a_damaged_database_is_reported_as_damaged() {
    let workspace = workspace();
    let root = Library::create(workspace.path(), "Mes logos")
        .expect("library is created")
        .root()
        .to_path_buf();
    let database_path = root.join(DATABASE_FILE_NAME);
    let mut bytes = fs::read(&database_path).expect("database readable");
    let page_size = usize::from(u16::from_be_bytes([bytes[16], bytes[17]]));
    assert!(
        bytes.len() >= 3 * page_size,
        "the database has several pages"
    );
    bytes[page_size..2 * page_size].fill(0xA5);
    fs::write(&database_path, bytes).expect("database damaged");

    let result = Library::open(&root);

    assert!(matches!(result, Err(LibraryError::Damaged)));
}
