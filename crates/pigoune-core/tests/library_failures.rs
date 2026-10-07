use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetView, CollectionCommand, DATABASE_FILE_NAME, FILES_DIR_NAME, ImportOutcome, Library,
};

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn a_library_cannot_be_opened_twice_at_the_same_time() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let library = Library::create(workspace.path(), "Essai").expect("library is created");

    let second = Library::open(library.root());

    assert!(second.is_err());
}

#[test]
fn everything_is_found_again_after_reopening_but_not_the_undo_history() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
    let collection = library.create_collection("Kept", None).expect("collection");
    let ImportOutcome::Imported(asset) = library
        .import_file(&sample("red-dot.png"), None)
        .expect("import succeeds")
    else {
        panic!("the resource is new");
    };
    library
        .apply_collection_command(&CollectionCommand::AddAssets {
            collection,
            assets: vec![asset],
        })
        .expect("resource is added");
    let root = library.root().to_path_buf();
    drop(library);

    let reopened = Library::open(&root).expect("library opens again");

    assert_eq!(
        reopened
            .visible_assets_in(AssetView::Collection(collection))
            .expect("assets are listed")
            .len(),
        1
    );
    assert_eq!(reopened.collections_of(asset).expect("read"), [collection]);
    assert!(!reopened.can_undo());
}

#[test]
fn a_stored_file_that_disappeared_does_not_break_the_library() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
    let ImportOutcome::Imported(asset) = library
        .import_file(&sample("red-dot.png"), None)
        .expect("import succeeds")
    else {
        panic!("the resource is new");
    };
    let stored = library.file_of(&library.asset(asset).expect("read").expect("asset"));
    let root = library.root().to_path_buf();
    drop(library);
    fs::remove_file(stored).expect("file is removed");

    let reopened = Library::open(&root).expect("library opens");

    assert_eq!(
        reopened.visible_assets().expect("assets are listed").len(),
        1
    );
    let copies = reopened.clipboard_copies(&[asset]);
    assert!(copies.is_err() || copies.expect("copies").is_empty());
}

#[test]
fn a_folder_that_is_not_a_library_is_refused() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let folder = workspace.path().join("not-a-library.pigoune");
    fs::create_dir_all(&folder).expect("folder is created");

    assert!(Library::open(&folder).is_err());
    assert!(Library::open(&workspace.path().join("absent")).is_err());
}

#[test]
fn a_damaged_database_is_refused_cleanly() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let folder = workspace.path().join("damaged.pigoune");
    fs::create_dir_all(folder.join(FILES_DIR_NAME)).expect("folder is created");
    fs::write(folder.join(DATABASE_FILE_NAME), b"this is not a database").expect("file is written");

    assert!(Library::open(&folder).is_err());
}

#[test]
fn forbidden_library_names_and_places_are_refused() {
    let workspace = tempfile::tempdir().expect("temporary directory");

    for name in ["", ".", "..", ".hidden", "a/b"] {
        assert!(
            Library::create(workspace.path(), name).is_err(),
            "{name:?} should be refused"
        );
    }
    assert!(Library::create(&workspace.path().join("absent"), "Essai").is_err());
    Library::create(workspace.path(), "Essai").expect("first creation");
    assert!(Library::create(workspace.path(), "Essai").is_err());
}
