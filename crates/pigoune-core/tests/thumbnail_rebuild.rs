use std::path::{Path, PathBuf};

use pigoune_core::{AssetCommand, AssetId, ImportOutcome, Library};
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    library: Library,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        Self {
            _workspace: workspace,
            library,
        }
    }

    fn import(&mut self, name: &str) -> AssetId {
        match self
            .library
            .import_file(&fixture(name), None)
            .expect("import succeeds")
        {
            ImportOutcome::Imported(id) => id,
            outcome => panic!("unexpected outcome {outcome:?}"),
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn every_asset_outside_the_trash_is_listed_for_its_thumbnail_newest_first() {
    let mut fixture = Fixture::new();
    let first = fixture.import("red-dot.png");
    let second = fixture.import("dark-circle.svg");

    assert_eq!(
        fixture.library.asset_ids_for_thumbnails().expect("listed"),
        [second, first]
    );
}

#[test]
fn a_trashed_asset_gets_no_thumbnail_rebuilt() {
    let mut fixture = Fixture::new();
    let kept = fixture.import("red-dot.png");
    let trashed = fixture.import("dark-circle.svg");
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![trashed],
            trashed: true,
        })
        .expect("asset is trashed");

    assert_eq!(
        fixture.library.asset_ids_for_thumbnails().expect("listed"),
        [kept]
    );
}

#[test]
fn a_library_without_assets_lists_nothing_for_thumbnails() {
    let fixture = Fixture::new();

    assert!(
        fixture
            .library
            .asset_ids_for_thumbnails()
            .expect("listed")
            .is_empty()
    );
}
