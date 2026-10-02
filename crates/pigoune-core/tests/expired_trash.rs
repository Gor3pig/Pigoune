use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pigoune_core::{
    AssetCommand, AssetId, AssetView, CollectionCommand, CollectionId, DATABASE_FILE_NAME,
    ImportOutcome, Library, TRASH_RETENTION,
};
use rusqlite::{Connection, params};
use tempfile::TempDir;

const ONE_DAY: Duration = Duration::from_hours(24);

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

    fn trash(&mut self, assets: &[AssetId]) {
        self.library
            .apply_asset_command(&AssetCommand::SetTrashed {
                assets: assets.to_vec(),
                trashed: true,
            })
            .expect("resources are trashed");
    }

    fn age(&self, table: &str, id: &str, age: Duration) {
        let connection =
            Connection::open(self.library.root().join(DATABASE_FILE_NAME)).expect("database opens");
        connection
            .execute(
                &format!("UPDATE {table} SET trashed_at_unix_ms = ?2 WHERE id = ?1"),
                params![id, unix_ms_ago(age)],
            )
            .expect("date is changed");
    }

    fn age_asset(&self, asset: AssetId, age: Duration) {
        self.age("assets", &asset.to_string(), age);
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

fn unix_ms_ago(age: Duration) -> i64 {
    let moment = SystemTime::now().checked_sub(age).expect("moment exists");
    let elapsed = moment.duration_since(UNIX_EPOCH).expect("after the epoch");
    i64::try_from(elapsed.as_millis()).expect("fits")
}

#[test]
fn the_trash_keeps_resources_for_thirty_days() {
    assert_eq!(TRASH_RETENTION, ONE_DAY * 30);
}

#[test]
fn only_resources_trashed_for_more_than_thirty_days_are_deleted() {
    let mut fixture = Fixture::new();
    let old = fixture.import("red-dot.png", None);
    let recent = fixture.import("github-mark.svg", None);
    let kept = fixture.import("still.gif", None);
    fixture.trash(&[old, recent]);
    fixture.age_asset(old, TRASH_RETENTION + ONE_DAY);
    fixture.age_asset(recent, TRASH_RETENTION.saturating_sub(ONE_DAY));
    let old_file = fixture.stored_file(old);
    let thumbnail = fixture.library.thumbnail_file(old, 128);
    fs::create_dir_all(thumbnail.parent().expect("thumbnail folder")).expect("folder created");
    fs::write(&thumbnail, b"png").expect("thumbnail written");

    let removed = fixture
        .library
        .empty_expired_trash()
        .expect("expired trash is emptied");

    assert_eq!(removed, 1);
    assert!(fixture.library.asset(old).expect("asset is read").is_none());
    assert!(!old_file.exists());
    assert!(!thumbnail.exists());
    assert_eq!(fixture.shown(AssetView::Trash), [recent]);
    assert_eq!(fixture.shown(AssetView::All), [kept]);
}

#[test]
fn an_old_deleted_collection_disappears_with_its_resources() {
    let mut fixture = Fixture::new();
    let old = fixture
        .library
        .create_collection("Ancienne", None)
        .expect("collection is created");
    let recent = fixture
        .library
        .create_collection("Récente", None)
        .expect("collection is created");
    let in_old = fixture.import("red-dot.png", Some(old));
    let in_recent = fixture.import("github-mark.svg", Some(recent));
    for id in [old, recent] {
        fixture
            .library
            .apply_collection_command(&CollectionCommand::Trash { id })
            .expect("collection is deleted");
    }
    fixture.age("collections", &old.to_string(), TRASH_RETENTION + ONE_DAY);
    fixture.age_asset(in_old, TRASH_RETENTION + ONE_DAY);

    fixture
        .library
        .empty_expired_trash()
        .expect("expired trash is emptied");

    assert!(fixture.library.collection(old).expect("read").is_none());
    assert!(fixture.library.asset(in_old).expect("read").is_none());
    assert!(fixture.library.collection(recent).expect("read").is_some());
    assert_eq!(fixture.shown(AssetView::Trash), [in_recent]);
}

#[test]
fn nothing_expired_keeps_the_undo_history() {
    let mut fixture = Fixture::new();
    let recent = fixture.import("red-dot.png", None);
    fixture.trash(&[recent]);

    let removed = fixture
        .library
        .empty_expired_trash()
        .expect("expired trash is emptied");

    assert_eq!(removed, 0);
    assert_eq!(fixture.shown(AssetView::Trash), [recent]);
    assert!(fixture.library.can_undo());
}

#[test]
fn deleting_expired_resources_forgets_the_undo_history() {
    let mut fixture = Fixture::new();
    let old = fixture.import("red-dot.png", None);
    fixture.trash(&[old]);
    fixture.age_asset(old, TRASH_RETENTION + ONE_DAY);

    fixture
        .library
        .empty_expired_trash()
        .expect("expired trash is emptied");

    assert!(!fixture.library.can_undo());
}
