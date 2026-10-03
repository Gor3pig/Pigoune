use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{AssetId, CACHE_DIR_NAME, DATABASE_FILE_NAME, ImportOutcome, Library};
use rusqlite::Connection;
use tempfile::TempDir;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn import(library: &mut Library, name: &str) -> AssetId {
    match library
        .import_file(&fixture(name), None)
        .expect("import succeeds")
    {
        ImportOutcome::Imported(id) => id,
        outcome => panic!("unexpected outcome {outcome:?}"),
    }
}

fn library_in(workspace: &TempDir) -> Library {
    Library::create(workspace.path(), "Essai").expect("library is created")
}

#[test]
fn visible_assets_are_listed_from_newest_to_oldest() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = library_in(&workspace);
    let first = import(&mut library, "red-dot.png");
    let second = import(&mut library, "dark-circle.svg");
    let third = import(&mut library, "spinner.gif");

    let listed: Vec<AssetId> = library
        .visible_assets()
        .expect("assets are listed")
        .into_iter()
        .map(|asset| asset.id)
        .collect();

    assert_eq!(listed, [third, second, first]);
}

#[test]
fn assets_in_the_trash_are_not_listed() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = library_in(&workspace);
    let kept = import(&mut library, "red-dot.png");
    let trashed = import(&mut library, "dark-circle.svg");
    Connection::open(library.root().join(DATABASE_FILE_NAME))
        .expect("database opens")
        .execute(
            "UPDATE assets SET trashed_at_unix_ms = 1 WHERE id = ?1",
            [trashed.to_string()],
        )
        .expect("asset trashed");

    let listed: Vec<AssetId> = library
        .visible_assets()
        .expect("assets are listed")
        .into_iter()
        .map(|asset| asset.id)
        .collect();

    assert_eq!(listed, [kept]);
}

#[test]
fn an_empty_library_lists_nothing() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let library = library_in(&workspace);

    assert!(
        library
            .visible_assets()
            .expect("assets are listed")
            .is_empty()
    );
}

#[test]
fn the_file_of_an_asset_is_its_copy_inside_the_library() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = library_in(&workspace);
    let id = import(&mut library, "dark-circle.svg");
    let asset = library
        .asset(id)
        .expect("asset is read")
        .expect("asset exists");

    let file = library.file_of(&asset);

    assert!(file.starts_with(library.root()));
    assert_eq!(
        fs::read(file).expect("copy readable"),
        fs::read(fixture("dark-circle.svg")).expect("fixture readable")
    );
}

#[test]
fn thumbnails_live_in_the_disposable_cache_folder_by_size() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = library_in(&workspace);
    let id = import(&mut library, "red-dot.png");

    let small = library.thumbnail_file(id, 128);
    let large = library.thumbnail_file(id, 256);

    assert_eq!(
        small,
        library
            .root()
            .join(CACHE_DIR_NAME)
            .join("thumbnails/128")
            .join(format!("{id}.png"))
    );
    assert_ne!(small, large);
}

#[test]
fn an_asset_id_survives_a_round_trip_through_text() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = library_in(&workspace);
    let id = import(&mut library, "red-dot.png");

    assert_eq!(AssetId::parse(&id.to_string()), Some(id));
    assert_eq!(AssetId::parse("not an id"), None);
}
