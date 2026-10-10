use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetFilter, AssetFormat, AssetId, AssetView, CollectionCommand, ImportOutcome,
    Library, TagCommand,
};
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    library: Library,
    views: Vec<AssetView>,
}

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
        outcome => panic!("unexpected outcome {outcome:?}"),
    }
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
        let red = import(&mut library, "red-dot.png");
        let circle = import(&mut library, "dark-circle.svg");
        let blinking = import(&mut library, "blinking.png");
        let photo = import(&mut library, "blue-photo.jpg");
        library
            .apply_asset_command(&AssetCommand::SetFavorite {
                assets: vec![red, photo],
                favorite: true,
            })
            .expect("favorites are set");
        let tech = library
            .create_collection("Tech", None)
            .expect("collection is created");
        library
            .apply_collection_command(&CollectionCommand::AddAssets {
                collection: tech,
                assets: vec![red, circle],
            })
            .expect("assets are filed");
        library
            .apply_tag_command(&TagCommand::Add {
                assets: vec![circle, blinking],
                name: "Brands/Logo".to_owned(),
            })
            .expect("assets are tagged");
        let logo = library
            .tags()
            .expect("tags are read")
            .into_iter()
            .find(|tag| tag.name == "Logo")
            .expect("tag exists");
        let brands = logo.parent.expect("Logo has a parent");
        let png = library
            .create_smart_collection(
                "PNG",
                &AssetFilter {
                    formats: vec![AssetFormat::Png],
                    ..AssetFilter::default()
                },
            )
            .expect("smart collection is saved");
        library
            .apply_asset_command(&AssetCommand::SetTrashed {
                assets: vec![photo],
                trashed: true,
            })
            .expect("asset is trashed");
        let views = vec![
            AssetView::All,
            AssetView::Favorites,
            AssetView::Unclassified,
            AssetView::Trash,
            AssetView::Collection(tech),
            AssetView::Tag(logo.id),
            AssetView::Tag(brands),
            AssetView::Smart(png),
        ];
        Self {
            _workspace: workspace,
            library,
            views,
        }
    }

    fn assert_counts_follow(&self, filter: &AssetFilter) {
        let counts = self
            .library
            .view_counts_matching(filter)
            .expect("counts are computed");
        for view in &self.views {
            let shown = self
                .library
                .find_assets_in(*view, filter)
                .expect("assets are found")
                .len();
            assert_eq!(counts.of(*view), shown, "{view:?} with {filter:?}");
        }
    }
}

#[test]
fn counts_follow_a_text_search_in_every_view() {
    let fixture = Fixture::new();
    for text in ["red", "dark", "o", "zzz", "logo", "brands"] {
        fixture.assert_counts_follow(&AssetFilter::text(text));
    }
}

#[test]
fn counts_follow_the_format_and_favorite_filters_in_every_view() {
    let fixture = Fixture::new();
    fixture.assert_counts_follow(&AssetFilter {
        formats: vec![AssetFormat::Png],
        ..AssetFilter::default()
    });
    fixture.assert_counts_follow(&AssetFilter {
        favorites_only: true,
        ..AssetFilter::default()
    });
    fixture.assert_counts_follow(&AssetFilter {
        text: "dot".to_owned(),
        formats: vec![AssetFormat::Png, AssetFormat::Svg],
        favorites_only: true,
        ..AssetFilter::default()
    });
}

#[test]
fn without_a_filter_the_counts_are_the_plain_ones() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture
            .library
            .view_counts_matching(&AssetFilter::default())
            .expect("counts are computed"),
        fixture.library.view_counts().expect("counts are computed")
    );
}
