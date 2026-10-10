use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetError, AssetId, AssetView, CollectionCommand, CollectionId, ImportOutcome,
    Library, TagCommand,
};
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

    fn collection(&mut self, name: &str) -> CollectionId {
        self.library
            .create_collection(name, None)
            .expect("collection is created")
    }

    fn import(&mut self, name: &str, collection: Option<CollectionId>) -> AssetId {
        match self
            .library
            .import_file(&fixture(name), collection)
            .expect("import succeeds")
        {
            ImportOutcome::Imported(id) => id,
            outcome => panic!("unexpected outcome {outcome:?}"),
        }
    }

    fn apply(&mut self, command: &AssetCommand) -> AssetCommand {
        self.library
            .apply_asset_command(command)
            .expect("command applies")
    }

    fn trash(&mut self, assets: &[AssetId]) -> AssetCommand {
        self.apply(&AssetCommand::SetTrashed {
            assets: assets.to_vec(),
            trashed: true,
        })
    }

    fn shown(&self, view: AssetView) -> Vec<AssetId> {
        let mut ids: Vec<AssetId> = self
            .library
            .visible_assets_in(view)
            .expect("assets are listed")
            .into_iter()
            .map(|asset| asset.id)
            .collect();
        ids.sort();
        ids
    }

    fn stored_file(&self, asset: AssetId) -> PathBuf {
        let asset = self
            .library
            .asset(asset)
            .expect("asset is read")
            .expect("asset exists");
        self.library.file_of(&asset)
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn sorted(mut ids: Vec<AssetId>) -> Vec<AssetId> {
    ids.sort();
    ids
}

#[test]
fn a_trashed_resource_leaves_every_view_for_the_trash() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech");
    let trashed = fixture.import("red-dot.png", Some(tech));
    let kept = fixture.import("dark-circle.svg", None);

    fixture.trash(&[trashed]);

    assert_eq!(fixture.shown(AssetView::All), [kept]);
    assert!(fixture.shown(AssetView::Collection(tech)).is_empty());
    assert_eq!(fixture.shown(AssetView::Trash), [trashed]);
    let counts = fixture.library.view_counts().expect("counts are read");
    assert_eq!(counts.of(AssetView::Trash), 1);
    assert_eq!(counts.of(AssetView::All), 1);
    assert!(
        fixture
            .library
            .view_contains(AssetView::Trash, trashed)
            .expect("view is read")
    );
}

#[test]
fn restoring_brings_back_collections_and_tags() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech");
    let asset = fixture.import("red-dot.png", Some(tech));
    fixture
        .library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![asset],
            name: "logo".to_owned(),
        })
        .expect("tag is added");
    let undo = fixture.trash(&[asset]);

    fixture.apply(&undo);

    assert_eq!(fixture.shown(AssetView::Collection(tech)), [asset]);
    assert_eq!(
        fixture
            .library
            .tags_of(asset)
            .expect("tags are read")
            .into_iter()
            .map(|tag| tag.name)
            .collect::<Vec<_>>(),
        ["logo"]
    );
    assert!(fixture.shown(AssetView::Trash).is_empty());
}

#[test]
fn a_resource_whose_collection_disappeared_comes_back_unclassified() {
    let mut fixture = Fixture::new();
    let gone = fixture.collection("Partie");
    let asset = fixture.import("red-dot.png", Some(gone));
    let undo = fixture.trash(&[asset]);
    fixture
        .library
        .apply_collection_command(&CollectionCommand::Trash { id: gone })
        .expect("collection is trashed");

    fixture.apply(&undo);

    assert_eq!(fixture.shown(AssetView::Unclassified), [asset]);
}

#[test]
fn trashing_twice_only_records_the_real_change() {
    let mut fixture = Fixture::new();
    let first = fixture.import("red-dot.png", None);
    let second = fixture.import("dark-circle.svg", None);
    fixture.trash(&[first]);

    let undo = fixture.trash(&[first, second]);

    assert_eq!(
        undo,
        AssetCommand::SetTrashed {
            assets: vec![second],
            trashed: false,
        }
    );
    assert_eq!(fixture.shown(AssetView::Trash), sorted(vec![first, second]));
}

#[test]
fn an_unknown_resource_cannot_be_trashed() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png", None);
    let unknown = AssetId::parse("00000000-0000-7000-8000-000000000099").expect("id");

    let refused = fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![asset, unknown],
            trashed: true,
        })
        .expect_err("command is refused");

    assert!(
        matches!(refused, AssetError::NotFound(id) if id == unknown),
        "{refused:?}"
    );
    assert_eq!(fixture.shown(AssetView::All), [asset]);
}

#[test]
fn emptying_the_trash_deletes_files_thumbnails_and_records_for_good() {
    let mut fixture = Fixture::new();
    let gone = fixture.collection("Partie");
    let trashed = fixture.import("red-dot.png", Some(gone));
    let kept = fixture.import("dark-circle.svg", Some(gone));
    let tech = fixture.collection("Tech");
    fixture
        .library
        .apply_collection_command(&CollectionCommand::AddAssets {
            collection: tech,
            assets: vec![kept],
        })
        .expect("asset is added");
    let trashed_file = fixture.stored_file(trashed);
    let kept_file = fixture.stored_file(kept);
    let thumbnail = fixture.library.thumbnail_file(trashed, 128);
    fs::create_dir_all(thumbnail.parent().expect("thumbnail folder")).expect("folder created");
    fs::write(&thumbnail, b"png").expect("thumbnail written");
    fixture.trash(&[trashed]);
    fixture
        .library
        .apply_collection_command(&CollectionCommand::Trash { id: gone })
        .expect("collection is trashed");

    let removed = fixture.library.empty_trash().expect("trash is emptied");

    assert_eq!(removed, 1);
    assert!(fixture.shown(AssetView::Trash).is_empty());
    assert!(
        fixture
            .library
            .asset(trashed)
            .expect("asset is read")
            .is_none()
    );
    assert!(
        fixture
            .library
            .collection(gone)
            .expect("collection is read")
            .is_none()
    );
    assert!(!trashed_file.exists());
    assert!(!thumbnail.exists());
    assert!(kept_file.exists());
    assert_eq!(fixture.shown(AssetView::Collection(tech)), [kept]);
}

#[test]
fn a_resource_can_be_imported_again_after_the_trash_is_emptied() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png", None);
    fixture.trash(&[asset]);
    fixture.library.empty_trash().expect("trash is emptied");

    let again = fixture.import("red-dot.png", None);

    assert_ne!(again, asset);
    assert_eq!(fixture.shown(AssetView::All), [again]);
}

#[test]
fn a_file_that_cannot_be_deleted_neither_keeps_its_record_nor_stops_the_others() {
    use std::os::unix::fs::PermissionsExt;

    let mut fixture = Fixture::new();
    let stuck = fixture.import("red-dot.png", None);
    let other = fixture.import("dark-circle.svg", None);
    let stuck_file = fixture.stored_file(stuck);
    let other_file = fixture.stored_file(other);
    fixture.trash(&[stuck, other]);
    let folder = stuck_file.parent().expect("asset folder").to_path_buf();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).expect("permissions change");
    let enforced = fs::write(folder.join("probe"), b"x").is_err();

    let result = fixture.library.empty_trash();

    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).expect("permissions change");
    if enforced {
        assert_eq!(result.expect("trash is emptied"), 2);
        assert!(fixture.library.asset(stuck).expect("read").is_none());
        assert!(fixture.library.asset(other).expect("read").is_none());
        assert!(!other_file.exists());
        assert!(stuck_file.exists());
    }
}
