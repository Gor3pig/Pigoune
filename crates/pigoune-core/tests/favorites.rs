use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetError, AssetId, AssetView, DATABASE_FILE_NAME, ImportOutcome, Library,
};
use rusqlite::Connection;
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

    fn apply(&mut self, command: &AssetCommand) -> AssetCommand {
        self.library
            .apply_asset_command(command)
            .expect("command applies")
    }

    fn is_favorite(&self, asset: AssetId) -> bool {
        self.library
            .asset(asset)
            .expect("asset is read")
            .expect("asset exists")
            .is_favorite
    }

    fn favorites(&self) -> Vec<AssetId> {
        let mut ids: Vec<AssetId> = self
            .library
            .visible_assets_in(AssetView::Favorites)
            .expect("assets are listed")
            .into_iter()
            .map(|asset| asset.id)
            .collect();
        ids.sort();
        ids
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn favorite(assets: &[AssetId], favorite: bool) -> AssetCommand {
    AssetCommand::SetFavorite {
        assets: assets.to_vec(),
        favorite,
    }
}

#[test]
fn a_favorite_shows_in_the_favorites_view_and_its_count() {
    let mut fixture = Fixture::new();
    let liked = fixture.import("red-dot.png");
    fixture.import("dark-circle.svg");

    fixture.apply(&favorite(&[liked], true));

    assert!(fixture.is_favorite(liked));
    assert_eq!(fixture.favorites(), [liked]);
    assert_eq!(
        fixture
            .library
            .view_counts()
            .expect("counts read")
            .of(AssetView::Favorites),
        1
    );
}

#[test]
fn undoing_restores_each_previous_state() {
    let mut fixture = Fixture::new();
    let already = fixture.import("red-dot.png");
    let newly = fixture.import("dark-circle.svg");
    fixture.apply(&favorite(&[already], true));

    let undo = fixture.apply(&favorite(&[already, newly], true));
    assert!(fixture.is_favorite(newly));
    let redo = fixture.apply(&undo);

    assert!(fixture.is_favorite(already));
    assert!(!fixture.is_favorite(newly));
    fixture.apply(&redo);
    assert!(fixture.is_favorite(newly));
}

#[test]
fn removing_a_favorite_takes_it_out_of_the_view() {
    let mut fixture = Fixture::new();
    let liked = fixture.import("red-dot.png");
    fixture.apply(&favorite(&[liked], true));

    fixture.apply(&favorite(&[liked], false));

    assert!(fixture.favorites().is_empty());
}

#[test]
fn an_unknown_or_trashed_resource_is_refused_and_nothing_changes() {
    let mut fixture = Fixture::new();
    let kept = fixture.import("red-dot.png");
    let trashed = fixture.import("dark-circle.svg");
    Connection::open(fixture.library.root().join(DATABASE_FILE_NAME))
        .expect("database opens")
        .execute(
            "UPDATE assets SET trashed_at_unix_ms = 1 WHERE id = ?1",
            [trashed.to_string()],
        )
        .expect("asset trashed");

    let result = fixture
        .library
        .apply_asset_command(&favorite(&[kept, trashed], true));

    assert!(
        matches!(result, Err(AssetError::NotFound(id)) if id == trashed),
        "{result:?}"
    );
    assert!(!fixture.is_favorite(kept));
}

#[test]
fn a_trashed_favorite_is_neither_shown_nor_counted() {
    let mut fixture = Fixture::new();
    let liked = fixture.import("red-dot.png");
    fixture.apply(&favorite(&[liked], true));
    Connection::open(fixture.library.root().join(DATABASE_FILE_NAME))
        .expect("database opens")
        .execute(
            "UPDATE assets SET trashed_at_unix_ms = 1 WHERE id = ?1",
            [liked.to_string()],
        )
        .expect("asset trashed");

    assert!(fixture.favorites().is_empty());
    assert_eq!(
        fixture
            .library
            .view_counts()
            .expect("counts read")
            .favorites,
        0
    );
}
