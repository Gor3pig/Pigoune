use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetId, AssetView, CACHE_DIR_NAME, CollectionCommand, ImportOutcome, Library,
    ReplaceError,
};

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

fn problems(library: &Library) -> usize {
    library
        .health_plan()
        .expect("plan")
        .run(|_| ControlFlow::Continue(()))
        .expect("not cancelled")
        .problems()
}

fn setup(name: &str) -> (tempfile::TempDir, Library, AssetId) {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let id = import(&mut library, name);
    (workspace, library, id)
}

#[test]
fn a_missing_file_is_put_back_from_an_identical_copy() {
    let (_workspace, library, id) = setup("red-dot.png");
    fs::remove_file(stored_file(&library, id)).expect("file removed");
    assert_eq!(problems(&library), 1);

    library
        .replace_stored_file(id, &sample("red-dot.png"))
        .expect("file is replaced");

    assert_eq!(
        fs::read(stored_file(&library, id)).expect("read"),
        fs::read(sample("red-dot.png")).expect("read")
    );
    assert_eq!(problems(&library), 0);
}

#[test]
fn a_damaged_file_is_replaced_without_leaving_anything_behind() {
    let (_workspace, library, id) = setup("blue-photo.jpg");
    fs::write(stored_file(&library, id), b"damaged").expect("damaged");
    assert_eq!(problems(&library), 1);

    library
        .replace_stored_file(id, &sample("blue-photo.jpg"))
        .expect("file is replaced");

    assert_eq!(problems(&library), 0);
}

#[test]
fn a_whole_missing_folder_is_created_again() {
    let (_workspace, library, id) = setup("red-dot.png");
    let folder = stored_file(&library, id)
        .parent()
        .expect("folder")
        .to_path_buf();
    fs::remove_dir_all(&folder).expect("folder removed");

    library
        .replace_stored_file(id, &sample("red-dot.png"))
        .expect("file is replaced");

    assert_eq!(problems(&library), 0);
}

#[test]
fn a_different_file_is_refused_and_nothing_changes() {
    let (_workspace, library, id) = setup("red-dot.png");
    let file = stored_file(&library, id);
    fs::write(&file, b"damaged").expect("damaged");

    let result = library.replace_stored_file(id, &sample("blue-photo.jpg"));

    assert!(matches!(result, Err(ReplaceError::DifferentContent(_))));
    assert_eq!(fs::read(&file).expect("read"), b"damaged");
    assert_eq!(problems(&library), 1);
}

#[test]
fn a_different_file_does_not_bring_a_missing_file_back() {
    let (_workspace, library, id) = setup("red-dot.png");
    let file = stored_file(&library, id);
    fs::remove_file(&file).expect("file removed");

    let result = library.replace_stored_file(id, &sample("blue-photo.jpg"));

    assert!(matches!(result, Err(ReplaceError::DifferentContent(_))));
    assert!(!file.exists());
}

#[test]
fn a_copy_that_cannot_be_read_is_reported() {
    let (workspace, library, id) = setup("red-dot.png");

    let result = library.replace_stored_file(id, &workspace.path().join("nowhere.png"));

    assert!(matches!(result, Err(ReplaceError::Unreadable(_))));
}

#[test]
fn an_unknown_resource_is_reported() {
    let (_workspace, library, id) = setup("red-dot.png");
    let unknown = {
        let other = tempfile::tempdir().expect("temporary directory");
        let mut library = Library::create(other.path(), "Autre").expect("library");
        let unknown = import(&mut library, "blue-photo.jpg");
        assert_ne!(unknown, id);
        unknown
    };

    let result = library.replace_stored_file(unknown, &sample("blue-photo.jpg"));

    assert!(matches!(result, Err(ReplaceError::AssetNotFound(found)) if found == unknown));
}

#[test]
fn the_file_of_a_trashed_resource_can_be_replaced() {
    let (_workspace, library, id) = setup("red-dot.png");
    let mut library = library;
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![id],
            trashed: true,
        })
        .expect("trashed");
    let file = stored_file(&library, id);
    fs::remove_file(&file).expect("file removed");

    library
        .replace_stored_file(id, &sample("red-dot.png"))
        .expect("file is replaced");

    assert!(file.is_file());
}

#[test]
fn the_record_keeps_its_name_favorite_and_collections() {
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
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![id],
            favorite: true,
        })
        .expect("favorite");
    fs::remove_file(stored_file(&library, id)).expect("file removed");

    library
        .replace_stored_file(id, &sample("red-dot.png"))
        .expect("file is replaced");

    assert_eq!(
        library
            .visible_assets_in(AssetView::Collection(collection))
            .expect("assets")
            .len(),
        1
    );
    assert_eq!(
        library
            .visible_assets_in(AssetView::Favorites)
            .expect("assets")
            .len(),
        1
    );
}

#[test]
fn the_thumbnails_of_the_resource_are_made_again() {
    let (_workspace, library, id) = setup("red-dot.png");
    let text = id.to_string();
    let thumbnail = library
        .root()
        .join(CACHE_DIR_NAME)
        .join("thumbnails")
        .join("256")
        .join(&text[text.len() - 2..])
        .join(format!("{text}.png"));
    fs::create_dir_all(thumbnail.parent().expect("folder")).expect("folder");
    fs::write(&thumbnail, b"old thumbnail").expect("thumbnail");

    library
        .replace_stored_file(id, &sample("red-dot.png"))
        .expect("file is replaced");

    assert!(!thumbnail.exists());
}
