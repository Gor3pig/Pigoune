use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{AssetId, CURRENT_FORMAT_VERSION, DATABASE_FILE_NAME, ImportOutcome, Library};
use rusqlite::Connection;
use tempfile::TempDir;

const FORMAT_BEFORE_ANIMATED_PNG_AND_WEBP: u32 = 1;

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
        other => panic!("unexpected outcome {other:?}"),
    }
}

fn library_as_saved_by_format_1(workspace: &TempDir, names: &[&str]) -> (PathBuf, Vec<AssetId>) {
    let mut library = Library::create(workspace.path(), "Ancienne").expect("library is created");
    let ids = names
        .iter()
        .map(|name| import(&mut library, name))
        .collect();
    let root = library.root().to_path_buf();
    drop(library);

    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .execute(
            "UPDATE assets SET is_animated = 0 WHERE format IN ('png', 'webp')",
            [],
        )
        .expect("animations forgotten");
    connection
        .pragma_update(None, "user_version", FORMAT_BEFORE_ANIMATED_PNG_AND_WEBP)
        .expect("version updated");
    (root, ids)
}

fn is_animated(library: &Library, id: AssetId) -> bool {
    library
        .asset(id)
        .expect("asset readable")
        .expect("asset exists")
        .is_animated
}

#[test]
fn opening_a_format_1_library_finds_its_animated_png_and_webp() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let (root, ids) = library_as_saved_by_format_1(
        &workspace,
        &[
            "blinking.png",
            "blinking.webp",
            "red-dot.png",
            "green-square.webp",
            "spinner.gif",
        ],
    );

    let library = Library::open(&root).expect("library opens");

    assert_eq!(
        library.format_version().expect("version readable"),
        CURRENT_FORMAT_VERSION
    );
    let animated: Vec<bool> = ids.iter().map(|id| is_animated(&library, *id)).collect();
    assert_eq!(animated, [true, true, false, false, true]);
}

#[test]
fn a_missing_file_does_not_prevent_the_upgrade() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let (root, ids) = library_as_saved_by_format_1(&workspace, &["blinking.png", "blinking.webp"]);
    let stored_path: String = Connection::open(root.join(DATABASE_FILE_NAME))
        .expect("database opens")
        .query_row(
            "SELECT stored_path FROM assets WHERE id = ?1",
            [ids[0].to_string()],
            |row| row.get(0),
        )
        .expect("stored path readable");
    fs::remove_file(root.join(stored_path)).expect("file removed");

    let library = Library::open(&root).expect("library opens");

    assert!(!is_animated(&library, ids[0]));
    assert!(is_animated(&library, ids[1]));
}
