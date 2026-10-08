use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use pigoune_core::{AdoptError, AssetId, AssetView, ImportOutcome, Library};

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

fn forget_record(root: &Path, id: AssetId) {
    let connection = rusqlite::Connection::open(root.join("library.db")).expect("database");
    connection
        .execute("DELETE FROM assets WHERE id = ?1", [id.to_string()])
        .expect("record removed");
}

fn unrecorded(library: &Library) -> Vec<PathBuf> {
    library
        .health_plan()
        .expect("plan")
        .run(|_| ControlFlow::Continue(()))
        .expect("not cancelled")
        .unrecorded
}

fn library_with_a_forgotten_file(name: &str) -> (tempfile::TempDir, Library, AssetId, PathBuf) {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let id = import(&mut library, name);
    let root = library.root().to_path_buf();
    drop(library);
    forget_record(&root, id);
    let library = Library::open(&root).expect("library reopens");
    let file = unrecorded(&library).pop().expect("one unrecorded file");
    (workspace, library, id, file)
}

#[test]
fn a_forgotten_file_gets_its_record_back_under_the_same_identity() {
    let (_workspace, mut library, id, file) = library_with_a_forgotten_file("red-dot.png");
    assert!(library.visible_assets().expect("assets").is_empty());

    let adopted = library.adopt_unrecorded(&file).expect("file is adopted");

    assert_eq!(adopted, id);
    let asset = library.asset(id).expect("query").expect("record exists");
    assert_eq!(asset.display_name, "red-dot");
    assert_eq!(
        library.root().join(asset.stored_path),
        library.root().join(&file)
    );
    assert_eq!(
        library
            .visible_assets_in(AssetView::Unclassified)
            .expect("assets")
            .len(),
        1
    );
    assert!(unrecorded(&library).is_empty());
}

#[test]
fn an_adopted_resource_behaves_like_any_other() {
    let (_workspace, mut library, id, file) = library_with_a_forgotten_file("blue-photo.jpg");
    library.adopt_unrecorded(&file).expect("file is adopted");

    library
        .apply_asset_command(&pigoune_core::AssetCommand::SetFavorite {
            assets: vec![id],
            favorite: true,
        })
        .expect("favorite");

    assert_eq!(
        library
            .visible_assets_in(AssetView::Favorites)
            .expect("assets")
            .len(),
        1
    );
}

#[test]
fn a_file_is_adopted_only_once() {
    let (_workspace, mut library, _id, file) = library_with_a_forgotten_file("red-dot.png");
    library.adopt_unrecorded(&file).expect("first adoption");

    let second = library.adopt_unrecorded(&file);

    assert!(matches!(second, Err(AdoptError::NotAdoptable(_))));
}

#[test]
fn a_file_that_is_not_an_image_is_refused_and_left_alone() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    import(&mut library, "red-dot.png");
    let stray_folder = library
        .root()
        .join("files/01/0190aaaa-0000-7000-8000-000000000001");
    fs::create_dir_all(&stray_folder).expect("folder");
    fs::write(stray_folder.join("notes.txt"), b"hello").expect("file");
    let file = unrecorded(&library).pop().expect("one unrecorded file");

    let result = library.adopt_unrecorded(&file);

    assert!(matches!(result, Err(AdoptError::UnsupportedFormat(_))));
    assert!(stray_folder.join("notes.txt").is_file());
    assert_eq!(library.visible_assets().expect("assets").len(), 1);
}

#[test]
fn a_file_with_the_content_of_a_known_resource_is_refused() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let id = import(&mut library, "red-dot.png");
    let stray_folder = library
        .root()
        .join("files/02/0190aaaa-0000-7000-8000-000000000002");
    fs::create_dir_all(&stray_folder).expect("folder");
    fs::copy(sample("red-dot.png"), stray_folder.join("copy.png")).expect("copy");
    let file = unrecorded(&library).pop().expect("one unrecorded file");

    let result = library.adopt_unrecorded(&file);

    assert!(matches!(result, Err(AdoptError::AlreadyKnown(_, known)) if known == id));
    assert_eq!(library.visible_assets().expect("assets").len(), 1);
}

#[test]
fn paths_that_do_not_follow_the_storage_layout_are_refused() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    import(&mut library, "red-dot.png");
    fs::write(library.root().join("files").join("loose.png"), b"x").expect("loose file");

    for path in [
        "files/loose.png",
        "../outside.png",
        "/etc/passwd",
        "files/aa/not-an-id/file.png",
        "files/aa/0190aaaa-0000-7000-8000-000000000003/missing.png",
        "library.db",
        "",
    ] {
        let result = library.adopt_unrecorded(Path::new(path));
        assert!(
            matches!(result, Err(AdoptError::NotAdoptable(_))),
            "{path} must be refused"
        );
    }
}

#[test]
fn a_recorded_file_cannot_be_adopted() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let id = import(&mut library, "red-dot.png");
    let stored = library
        .asset(id)
        .expect("query")
        .expect("asset")
        .stored_path;

    let result = library.adopt_unrecorded(&stored);

    assert!(matches!(result, Err(AdoptError::NotAdoptable(_))));
}
