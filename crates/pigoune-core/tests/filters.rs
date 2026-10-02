use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetFilter, AssetFormat, AssetId, AssetView, CollectionId, ImportOutcome,
    Library,
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

    fn favorite(&mut self, asset: AssetId) {
        self.library
            .apply_asset_command(&AssetCommand::SetFavorite {
                assets: vec![asset],
                favorite: true,
            })
            .expect("favorite is set");
    }

    fn found(&self, view: AssetView, filter: &AssetFilter) -> Vec<AssetId> {
        let mut ids: Vec<AssetId> = self
            .library
            .find_assets_in(view, filter)
            .expect("search succeeds")
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

fn sorted(mut ids: Vec<AssetId>) -> Vec<AssetId> {
    ids.sort();
    ids
}

#[test]
fn no_filter_keeps_the_whole_view() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png", None);
    let svg = fixture.import("github-mark.svg", None);

    assert_eq!(
        fixture.found(AssetView::All, &AssetFilter::default()),
        sorted(vec![png, svg])
    );
}

#[test]
fn any_of_the_chosen_types_is_kept() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png", None);
    let svg = fixture.import("github-mark.svg", None);
    fixture.import("still.gif", None);
    let filter = AssetFilter {
        formats: vec![AssetFormat::Png, AssetFormat::Svg],
        ..AssetFilter::default()
    };

    assert_eq!(
        fixture.found(AssetView::All, &filter),
        sorted(vec![png, svg])
    );
}

#[test]
fn only_favorites_can_be_kept() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png", None);
    fixture.import("github-mark.svg", None);
    fixture.favorite(png);
    let filter = AssetFilter {
        favorites_only: true,
        ..AssetFilter::default()
    };

    assert_eq!(fixture.found(AssetView::All, &filter), [png]);
}

#[test]
fn filters_combine_with_the_text_and_the_view() {
    let mut fixture = Fixture::new();
    let logos = fixture
        .library
        .create_collection("Logos", None)
        .expect("collection is created");
    let wanted = fixture.import("github-mark.svg", Some(logos));
    let other_type = fixture.import("red-dot.png", Some(logos));
    let outside = fixture.import("still.gif", None);
    for asset in [wanted, other_type, outside] {
        fixture.favorite(asset);
    }
    let filter = AssetFilter {
        text: "mark".to_owned(),
        formats: vec![AssetFormat::Svg],
        favorites_only: true,
    };

    assert_eq!(
        fixture.found(AssetView::Collection(logos), &filter),
        [wanted]
    );
    assert!(
        fixture
            .found(
                AssetView::Collection(logos),
                &AssetFilter {
                    formats: vec![AssetFormat::Gif],
                    ..AssetFilter::default()
                }
            )
            .is_empty()
    );
}

#[test]
fn the_chosen_filters_are_counted_without_the_text() {
    let filter = AssetFilter {
        text: "logo".to_owned(),
        formats: vec![AssetFormat::Png, AssetFormat::Svg],
        favorites_only: true,
    };

    assert_eq!(filter.chosen_filters(), 3);
    assert_eq!(AssetFilter::text("logo").chosen_filters(), 0);
    assert!(AssetFilter::text("logo").narrows());
    assert!(!AssetFilter::text("   ").narrows());
}
