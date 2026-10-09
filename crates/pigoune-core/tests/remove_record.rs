use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetId, AssetView, CollectionCommand, DATABASE_FILE_NAME, ImportOutcome,
    Library, RemoveRecordError, TagCommand,
};
use rusqlite::{Connection, params};

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn import(library: &mut Library, name: &str) -> AssetId {
    match library
        .import_file(&sample(name), None)
        .expect("import succeeds")
    {
        ImportOutcome::Imported(id) => id,
        other => panic!("the resource is new, got {other:?}"),
    }
}

fn stored_file(library: &Library, id: AssetId) -> PathBuf {
    let asset = library.asset(id).expect("query").expect("asset exists");
    library.root().join(asset.stored_path)
}

fn setup(name: &str) -> (tempfile::TempDir, Library, AssetId) {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let id = import(&mut library, name);
    (workspace, library, id)
}

#[test]
fn the_record_of_a_missing_file_is_removed_with_what_hangs_on_it() {
    let (_workspace, mut library, id) = setup("red-dot.png");
    let collection = library
        .create_collection("Marque", None)
        .expect("collection");
    library
        .apply_collection_command(&CollectionCommand::AddAssets {
            collection,
            assets: vec![id],
        })
        .expect("added");
    library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![id],
            name: "logo".to_owned(),
        })
        .expect("tagged");
    let tag = library.tags().expect("tags")[0].id;
    fs::remove_file(stored_file(&library, id)).expect("file removed");

    library
        .remove_record_of_missing_file(id)
        .expect("record removed");

    assert!(library.asset(id).expect("query").is_none());
    assert!(library.visible_assets().expect("assets").is_empty());
    assert!(
        library
            .visible_assets_in(AssetView::Collection(collection))
            .expect("assets")
            .is_empty()
    );
    assert!(
        library
            .visible_assets_in(AssetView::Tag(tag))
            .expect("assets")
            .is_empty()
    );
    assert_eq!(library.tags().expect("tags").len(), 1);
    assert_eq!(library.visible_collections().expect("collections").len(), 1);
}

#[test]
fn a_record_whose_file_exists_is_never_removed() {
    let (_workspace, mut library, id) = setup("red-dot.png");

    let result = library.remove_record_of_missing_file(id);

    assert!(matches!(result, Err(RemoveRecordError::FileStillThere(found)) if found == id));
    assert!(library.asset(id).expect("query").is_some());
    assert!(stored_file(&library, id).is_file());
}

#[test]
fn a_damaged_file_keeps_its_record() {
    let (_workspace, mut library, id) = setup("red-dot.png");
    fs::write(stored_file(&library, id), b"damaged").expect("damaged");

    let result = library.remove_record_of_missing_file(id);

    assert!(matches!(result, Err(RemoveRecordError::FileStillThere(_))));
    assert!(library.asset(id).expect("query").is_some());
}

#[test]
fn an_unknown_resource_is_reported() {
    let (_workspace, mut library, id) = setup("red-dot.png");
    fs::remove_file(stored_file(&library, id)).expect("file removed");
    library
        .remove_record_of_missing_file(id)
        .expect("record removed");

    let again = library.remove_record_of_missing_file(id);

    assert!(matches!(again, Err(RemoveRecordError::AssetNotFound(found)) if found == id));
}

#[test]
fn the_record_of_a_trashed_resource_can_be_removed() {
    let (_workspace, mut library, id) = setup("red-dot.png");
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![id],
            trashed: true,
        })
        .expect("trashed");
    fs::remove_file(stored_file(&library, id)).expect("file removed");

    library
        .remove_record_of_missing_file(id)
        .expect("record removed");

    assert!(library.asset(id).expect("query").is_none());
}

#[test]
fn the_undo_history_forgets_the_removed_resource() {
    let (_workspace, mut library, id) = setup("red-dot.png");
    library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![id],
            favorite: true,
        })
        .expect("favorite");
    assert!(library.can_undo());
    fs::remove_file(stored_file(&library, id)).expect("file removed");

    library
        .remove_record_of_missing_file(id)
        .expect("record removed");

    assert!(!library.can_undo());
}

#[test]
fn another_file_in_the_folder_is_left_alone() {
    let (_workspace, mut library, id) = setup("red-dot.png");
    let file = stored_file(&library, id);
    let stray = file.with_file_name("stray.txt");
    fs::write(&stray, b"keep me").expect("stray");
    fs::remove_file(&file).expect("file removed");

    library
        .remove_record_of_missing_file(id)
        .expect("record removed");

    assert!(stray.is_file());
}

#[test]
fn the_empty_folder_of_the_resource_is_cleaned_up() {
    let (_workspace, mut library, id) = setup("red-dot.png");
    let file = stored_file(&library, id);
    fs::remove_file(&file).expect("file removed");

    library
        .remove_record_of_missing_file(id)
        .expect("record removed");

    assert!(!file.parent().expect("folder").exists());
}

#[test]
fn the_same_image_can_be_imported_again_afterwards() {
    let (_workspace, mut library, id) = setup("red-dot.png");
    fs::remove_file(stored_file(&library, id)).expect("file removed");
    library
        .remove_record_of_missing_file(id)
        .expect("record removed");

    let again = import(&mut library, "red-dot.png");

    assert_ne!(again, id);
    assert!(stored_file(&library, again).is_file());
}

#[test]
fn a_record_pointing_outside_the_library_is_removed_without_touching_the_disk() {
    let (workspace, library, id) = setup("red-dot.png");
    let root = library.root().to_path_buf();
    drop(library);
    let folder = workspace.path().join("outside").join("empty");
    fs::create_dir_all(&folder).expect("folder created");
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .execute(
            "UPDATE assets SET stored_path = ?1 WHERE id = ?2",
            params!["../outside/empty/victim.png", id.to_string()],
        )
        .expect("record is changed");
    drop(connection);
    let mut library = Library::open(&root).expect("library opens");

    library
        .remove_record_of_missing_file(id)
        .expect("record is removed");

    assert!(library.asset(id).expect("query").is_none());
    assert!(folder.is_dir());
}

#[test]
fn an_unreadable_folder_keeps_the_record() {
    let (_workspace, mut library, id) = setup("red-dot.png");
    let file = stored_file(&library, id);
    let folder = file.parent().expect("parent folder").to_path_buf();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o000)).expect("permissions change");
    let enforced = fs::exists(&file).is_err();

    let result = library.remove_record_of_missing_file(id);

    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).expect("permissions change");
    if enforced {
        assert!(matches!(result, Err(RemoveRecordError::FileStillThere(found)) if found == id));
        assert!(library.asset(id).expect("query").is_some());
        assert!(file.is_file());
    }
}
