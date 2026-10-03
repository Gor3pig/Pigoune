use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetId, AssetView, CollectionCommand, CollectionId, DATABASE_FILE_NAME, ImportOutcome, Library,
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

    fn collection(&mut self, name: &str, parent: Option<CollectionId>) -> CollectionId {
        self.library
            .create_collection(name, parent)
            .expect("collection is created")
    }

    fn import(&mut self, name: &str, collection: Option<CollectionId>) -> AssetId {
        match self
            .library
            .import_file(&fixture(name), collection)
            .expect("import succeeds")
        {
            ImportOutcome::Imported(id) | ImportOutcome::AddedToCollection(id) => id,
            outcome => panic!("unexpected outcome {outcome:?}"),
        }
    }

    fn shown(&self, view: AssetView) -> BTreeSet<AssetId> {
        self.library
            .visible_assets_in(view)
            .expect("assets are listed")
            .into_iter()
            .map(|asset| asset.id)
            .collect()
    }

    fn trash_asset(&self, asset: AssetId) {
        Connection::open(self.library.root().join(DATABASE_FILE_NAME))
            .expect("database opens")
            .execute(
                "UPDATE assets SET trashed_at_unix_ms = 1 WHERE id = ?1",
                [asset.to_string()],
            )
            .expect("asset trashed");
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn set(ids: &[AssetId]) -> BTreeSet<AssetId> {
    ids.iter().copied().collect()
}

struct Sample {
    fixture: Fixture,
    brands: CollectionId,
    tech: CollectionId,
    fashion: CollectionId,
    in_brands: AssetId,
    in_tech: AssetId,
    in_tech_and_fashion: AssetId,
    unclassified: AssetId,
}

fn sample() -> Sample {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    let tech = fixture.collection("Tech", Some(brands));
    let fashion = fixture.collection("Mode", None);
    let in_brands = fixture.import("red-dot.png", Some(brands));
    let in_tech = fixture.import("dark-circle.svg", Some(tech));
    let in_tech_and_fashion = fixture.import("spinner.gif", Some(tech));
    fixture.import("spinner.gif", Some(fashion));
    let unclassified = fixture.import("still.gif", None);
    let trashed = fixture.import("blue-photo.jpg", Some(tech));
    fixture.trash_asset(trashed);
    Sample {
        fixture,
        brands,
        tech,
        fashion,
        in_brands,
        in_tech,
        in_tech_and_fashion,
        unclassified,
    }
}

#[test]
fn a_collection_shows_its_resources_and_those_of_its_sub_collections_once() {
    let sample = sample();

    assert_eq!(
        sample.fixture.shown(AssetView::Collection(sample.brands)),
        set(&[sample.in_brands, sample.in_tech, sample.in_tech_and_fashion])
    );
    assert_eq!(
        sample.fixture.shown(AssetView::Collection(sample.tech)),
        set(&[sample.in_tech, sample.in_tech_and_fashion])
    );
    assert_eq!(
        sample.fixture.shown(AssetView::Collection(sample.fashion)),
        set(&[sample.in_tech_and_fashion])
    );
}

#[test]
fn unclassified_and_all_views_skip_the_trash() {
    let sample = sample();

    assert_eq!(
        sample.fixture.shown(AssetView::Unclassified),
        set(&[sample.unclassified])
    );
    assert_eq!(
        sample.fixture.shown(AssetView::All),
        set(&[
            sample.in_brands,
            sample.in_tech,
            sample.in_tech_and_fashion,
            sample.unclassified
        ])
    );
}

#[test]
fn counts_match_what_each_view_shows() {
    let sample = sample();
    let counts = sample.fixture.library.view_counts().expect("counts read");

    for view in [
        AssetView::All,
        AssetView::Unclassified,
        AssetView::Collection(sample.brands),
        AssetView::Collection(sample.tech),
        AssetView::Collection(sample.fashion),
    ] {
        assert_eq!(
            counts.of(view),
            sample.fixture.shown(view).len(),
            "{view:?}"
        );
    }
}

#[test]
fn a_trashed_collection_shows_nothing_and_is_not_counted() {
    let mut sample = sample();
    sample
        .fixture
        .library
        .apply_collection_command(&CollectionCommand::Trash { id: sample.brands })
        .expect("collection trashed");
    let counts = sample.fixture.library.view_counts().expect("counts read");

    assert!(
        sample
            .fixture
            .shown(AssetView::Collection(sample.brands))
            .is_empty()
    );
    assert!(!counts.collections.contains_key(&sample.brands));
    assert_eq!(
        sample.fixture.shown(AssetView::Collection(sample.fashion)),
        set(&[sample.in_tech_and_fashion])
    );
}

#[test]
fn an_empty_collection_counts_zero() {
    let mut fixture = Fixture::new();
    let empty = fixture.collection("Vide", None);
    let counts = fixture.library.view_counts().expect("counts read");

    assert_eq!(counts.of(AssetView::Collection(empty)), 0);
    assert!(fixture.shown(AssetView::Collection(empty)).is_empty());
}
