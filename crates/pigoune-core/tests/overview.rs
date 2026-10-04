use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetId, CURRENT_FORMAT_VERSION, DATABASE_FILE_NAME, ImportOutcome, Library,
    TagCommand,
};
use rusqlite::Connection;

fn sample_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn import(library: &mut Library, name: &str) -> AssetId {
    match library
        .import_file(&sample_file(name), None)
        .expect("import succeeds")
    {
        ImportOutcome::Imported(id) => id,
        other => panic!("unexpected outcome {other:?}"),
    }
}

#[test]
fn an_empty_library_has_an_empty_overview() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let library = Library::create(workspace.path(), "Vide").expect("library is created");

    let overview = library.overview().expect("overview is read");

    assert_eq!(overview.resources, 0);
    assert_eq!(overview.collections, 0);
    assert_eq!(overview.tags, 0);
    assert_eq!(overview.format_version, CURRENT_FORMAT_VERSION);
}

#[test]
fn the_overview_counts_what_the_library_holds() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
    let svg = import(&mut library, "dark-circle.svg");
    let gif = import(&mut library, "spinner.gif");
    let png = import(&mut library, "red-dot.png");
    let trashed = import(&mut library, "blue-photo.jpg");
    library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![svg, gif],
            favorite: true,
        })
        .expect("favorites set");
    library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![png],
            name: "rouge".to_owned(),
        })
        .expect("tag added");
    library
        .create_collection("Logos", None)
        .expect("collection created");
    Connection::open(library.root().join(DATABASE_FILE_NAME))
        .expect("database opens")
        .execute(
            "UPDATE assets SET trashed_at_unix_ms = 1 WHERE id = ?1",
            [trashed.to_string()],
        )
        .expect("asset trashed");

    let overview = library.overview().expect("overview is read");

    assert_eq!(overview.resources, 3);
    assert_eq!(overview.favorites, 2);
    assert_eq!(overview.animated, 1);
    assert_eq!(overview.vectors, 1);
    assert_eq!(overview.in_trash, 1);
    assert_eq!(overview.collections, 1);
    assert_eq!(overview.tags, 1);
}
