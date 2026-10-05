use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetColor, AssetCommand, AssetFilter, AssetFormat, AssetId, AssetView, CollectionId,
    DominantColor, ImportOutcome, Library, Rgb,
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

    fn color(&mut self, asset: AssetId, families: &[AssetColor]) {
        let colors: Vec<DominantColor> = families
            .iter()
            .map(|family| DominantColor {
                family: *family,
                average: Rgb::new(0, 0, 0),
            })
            .collect();
        self.paint(asset, &colors);
    }

    fn paint(&mut self, asset: AssetId, colors: &[DominantColor]) {
        self.library
            .record_colors(asset, colors)
            .expect("colors are recorded");
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
    let svg = fixture.import("dark-circle.svg", None);

    assert_eq!(
        fixture.found(AssetView::All, &AssetFilter::default()),
        sorted(vec![png, svg])
    );
}

#[test]
fn any_of_the_chosen_types_is_kept() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png", None);
    let svg = fixture.import("dark-circle.svg", None);
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
    fixture.import("dark-circle.svg", None);
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
    let wanted = fixture.import("dark-circle.svg", Some(logos));
    let other_type = fixture.import("red-dot.png", Some(logos));
    let outside = fixture.import("still.gif", None);
    for asset in [wanted, other_type, outside] {
        fixture.favorite(asset);
    }
    let filter = AssetFilter {
        text: "circle".to_owned(),
        formats: vec![AssetFormat::Svg],
        favorites_only: true,
        colors: Vec::new(),
        custom_color: None,
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
        colors: vec![AssetColor::Blue],
        custom_color: Some(Rgb::new(0, 0, 0)),
    };

    assert_eq!(filter.chosen_filters(), 5);
    assert_eq!(AssetFilter::text("logo").chosen_filters(), 0);
    assert!(AssetFilter::text("logo").narrows());
    assert!(!AssetFilter::text("   ").narrows());
}

fn colors(colors: &[AssetColor]) -> AssetFilter {
    AssetFilter {
        colors: colors.to_vec(),
        ..AssetFilter::default()
    }
}

#[test]
fn any_of_the_chosen_colors_is_kept() {
    let mut fixture = Fixture::new();
    let red = fixture.import("red-dot.png", None);
    let navy = fixture.import("navy-tile.bmp", None);
    let teal = fixture.import("teal-column.tiff", None);
    fixture.color(red, &[AssetColor::Red, AssetColor::White]);
    fixture.color(navy, &[AssetColor::Blue]);
    fixture.color(teal, &[AssetColor::Teal]);

    assert_eq!(
        fixture.found(AssetView::All, &colors(&[AssetColor::White])),
        [red]
    );
    assert_eq!(
        fixture.found(
            AssetView::All,
            &colors(&[AssetColor::Red, AssetColor::Blue])
        ),
        sorted(vec![red, navy])
    );
    assert!(
        fixture
            .found(AssetView::All, &colors(&[AssetColor::Green]))
            .is_empty()
    );
}

#[test]
fn a_resource_not_analysed_yet_matches_no_color() {
    let mut fixture = Fixture::new();
    let analysed = fixture.import("red-dot.png", None);
    let waiting = fixture.import("dark-circle.svg", None);
    fixture.color(analysed, &[AssetColor::Red]);

    assert_eq!(
        fixture.library.assets_awaiting_colors(10).expect("listed"),
        [waiting]
    );
    assert_eq!(
        fixture.found(AssetView::All, &colors(&[AssetColor::Red])),
        [analysed]
    );
    assert!(
        fixture
            .found(AssetView::All, &colors(&[AssetColor::Black]))
            .is_empty()
    );
}

#[test]
fn a_resource_without_a_main_color_is_analysed_all_the_same() {
    let mut fixture = Fixture::new();
    let clear = fixture.import("red-dot.png", None);
    fixture.color(clear, &[]);

    assert!(
        fixture
            .library
            .assets_awaiting_colors(10)
            .expect("listed")
            .is_empty()
    );
    assert!(
        fixture
            .found(AssetView::All, &colors(&[AssetColor::Red]))
            .is_empty()
    );
}

#[test]
fn colors_add_up_with_the_other_filters() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png", None);
    let svg = fixture.import("dark-circle.svg", None);
    fixture.color(png, &[AssetColor::Red]);
    fixture.color(svg, &[AssetColor::Red]);
    let filter = AssetFilter {
        formats: vec![AssetFormat::Svg],
        colors: vec![AssetColor::Red],
        ..AssetFilter::default()
    };

    assert_eq!(fixture.found(AssetView::All, &filter), [svg]);
    assert_eq!(filter.chosen_filters(), 2);
    assert!(colors(&[AssetColor::Red]).narrows());
}

fn custom(hex: &str) -> AssetFilter {
    AssetFilter {
        custom_color: Rgb::from_hex(hex),
        ..AssetFilter::default()
    }
}

fn average(family: AssetColor, hex: &str) -> DominantColor {
    DominantColor {
        family,
        average: Rgb::from_hex(hex).expect("valid color"),
    }
}

#[test]
fn a_custom_color_keeps_resources_with_a_close_main_color() {
    let mut fixture = Fixture::new();
    let bright = fixture.import("red-dot.png", None);
    let coral = fixture.import("navy-tile.bmp", None);
    let brick = fixture.import("teal-column.tiff", None);
    fixture.paint(
        bright,
        &[
            average(AssetColor::Red, "e01b24"),
            average(AssetColor::White, "ffffff"),
        ],
    );
    fixture.paint(coral, &[average(AssetColor::Red, "f66151")]);
    fixture.paint(
        brick,
        &[
            average(AssetColor::Gray, "5e5c64"),
            average(AssetColor::Red, "b5482f"),
        ],
    );

    assert_eq!(
        fixture.found(AssetView::All, &custom("c0392b")),
        sorted(vec![bright, brick])
    );
}

#[test]
fn a_custom_color_adds_up_with_the_chosen_swatches() {
    let mut fixture = Fixture::new();
    let brick = fixture.import("red-dot.png", None);
    let navy = fixture.import("navy-tile.bmp", None);
    let teal = fixture.import("teal-column.tiff", None);
    fixture.paint(brick, &[average(AssetColor::Red, "b5482f")]);
    fixture.paint(navy, &[average(AssetColor::Blue, "1a3a6b")]);
    fixture.paint(teal, &[average(AssetColor::Teal, "2190a4")]);
    let filter = AssetFilter {
        colors: vec![AssetColor::Teal],
        custom_color: Rgb::from_hex("c0392b"),
        ..AssetFilter::default()
    };

    assert_eq!(
        fixture.found(AssetView::All, &filter),
        sorted(vec![brick, teal])
    );
    assert_eq!(filter.chosen_filters(), 2);
    assert!(custom("c0392b").narrows());
}
