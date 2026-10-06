use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetColor, AssetCommand, AssetFilter, AssetFormat, AssetId, AssetShape, AssetView,
    CollectionCommand, Dimensions, ImportOutcome, Library, Rgb, SmartCollection,
    SmartCollectionCommand, SmartCollectionError, SmartCollectionId,
};
use tempfile::TempDir;

struct Fixture {
    workspace: TempDir,
    library: Library,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        Self { workspace, library }
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

    fn save(&mut self, name: &str, filter: &AssetFilter) -> SmartCollectionId {
        self.library
            .create_smart_collection(name, filter)
            .expect("smart collection is saved")
    }

    fn refused(&mut self, name: &str) -> SmartCollectionError {
        self.library
            .create_smart_collection(name, &AssetFilter::default())
            .expect_err("smart collection is refused")
    }

    fn shown(&self, view: AssetView) -> Vec<AssetId> {
        let mut ids: Vec<AssetId> = self
            .library
            .visible_assets_in(view)
            .expect("assets listed")
            .iter()
            .map(|asset| asset.id)
            .collect();
        ids.sort();
        ids
    }

    fn names(&self) -> Vec<String> {
        self.library
            .smart_collections()
            .expect("smart collections listed")
            .into_iter()
            .map(|collection| collection.name)
            .collect()
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn formats(formats: &[AssetFormat]) -> AssetFilter {
    AssetFilter {
        formats: formats.to_vec(),
        ..AssetFilter::default()
    }
}

fn sorted(mut ids: Vec<AssetId>) -> Vec<AssetId> {
    ids.sort();
    ids
}

#[test]
fn a_smart_collection_shows_the_resources_matching_its_criteria() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    fixture.import("dark-circle.svg");
    let webp = fixture.import("green-square.webp");

    let id = fixture.save("Rasters", &formats(&[AssetFormat::Png, AssetFormat::Webp]));

    assert_eq!(fixture.shown(AssetView::Smart(id)), sorted(vec![png, webp]));
}

#[test]
fn a_resource_imported_later_appears_by_itself() {
    let mut fixture = Fixture::new();
    fixture.import("red-dot.png");
    let id = fixture.save("Vectors", &formats(&[AssetFormat::Svg]));
    assert!(fixture.shown(AssetView::Smart(id)).is_empty());

    let svg = fixture.import("dark-circle.svg");

    assert_eq!(fixture.shown(AssetView::Smart(id)), vec![svg]);
}

#[test]
fn search_text_and_favorites_narrow_the_whole_library() {
    let mut fixture = Fixture::new();
    let red = fixture.import("red-dot.png");
    let blinking = fixture.import("blinking.png");
    let circle = fixture.import("dark-circle.svg");
    let collection = fixture
        .library
        .create_collection("Logos", None)
        .expect("collection created");
    fixture
        .library
        .apply_collection_command(&CollectionCommand::AddAssets {
            collection,
            assets: vec![circle],
        })
        .expect("assets added");
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![red, blinking],
            favorite: true,
        })
        .expect("favorites set");

    let favorites = fixture.save(
        "Favorites",
        &AssetFilter {
            favorites_only: true,
            ..AssetFilter::default()
        },
    );
    let named = fixture.save("Dots", &AssetFilter::text("dot"));

    assert_eq!(
        fixture.shown(AssetView::Smart(favorites)),
        sorted(vec![red, blinking])
    );
    assert_eq!(fixture.shown(AssetView::Smart(named)), vec![red]);
}

#[test]
fn a_search_inside_a_smart_collection_narrows_it_further() {
    let mut fixture = Fixture::new();
    let red = fixture.import("red-dot.png");
    fixture.import("blinking.png");
    let id = fixture.save("PNG", &formats(&[AssetFormat::Png]));

    let found = fixture
        .library
        .find_assets_in(AssetView::Smart(id), &AssetFilter::text("red"))
        .expect("search runs");

    assert_eq!(
        found.iter().map(|asset| asset.id).collect::<Vec<_>>(),
        vec![red]
    );
}

#[test]
fn the_view_counts_include_each_smart_collection() {
    let mut fixture = Fixture::new();
    fixture.import("red-dot.png");
    fixture.import("blinking.png");
    fixture.import("dark-circle.svg");
    let id = fixture.save("PNG", &formats(&[AssetFormat::Png]));

    let counts = fixture.library.view_counts().expect("counts computed");

    assert_eq!(counts.of(AssetView::Smart(id)), 2);
    assert!(
        fixture
            .library
            .view_contains(AssetView::Smart(id), fixture.shown(AssetView::Smart(id))[0])
            .expect("checked")
    );
}

#[test]
fn trashed_resources_leave_smart_collections() {
    let mut fixture = Fixture::new();
    let red = fixture.import("red-dot.png");
    let id = fixture.save("PNG", &formats(&[AssetFormat::Png]));

    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![red],
            trashed: true,
        })
        .expect("resource trashed");

    assert!(fixture.shown(AssetView::Smart(id)).is_empty());
}

#[test]
fn names_are_trimmed_required_and_unique_whatever_the_case() {
    let mut fixture = Fixture::new();
    fixture.save("  Logos  ", &AssetFilter::default());

    assert_eq!(fixture.names(), ["Logos"]);
    assert!(matches!(
        fixture.refused("   "),
        SmartCollectionError::InvalidName
    ));
    assert!(matches!(
        fixture.refused("LOGOS"),
        SmartCollectionError::NameTaken(_)
    ));
}

#[test]
fn smart_collections_are_listed_by_name() {
    let mut fixture = Fixture::new();
    fixture.save("zèbres", &AssetFilter::default());
    fixture.save("Avions", &AssetFilter::default());

    assert_eq!(fixture.names(), ["Avions", "zèbres"]);
}

fn update(
    fixture: &mut Fixture,
    id: SmartCollectionId,
    name: &str,
    filter: AssetFilter,
) -> Result<SmartCollectionCommand, SmartCollectionError> {
    fixture
        .library
        .apply_smart_collection_command(&SmartCollectionCommand::Update {
            collection: SmartCollection {
                id,
                name: name.to_owned(),
                filter,
                position: 0,
                created_at_unix_ms: 0,
            },
        })
}

#[test]
fn updating_changes_the_name_and_criteria_and_can_be_undone() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let svg = fixture.import("dark-circle.svg");
    let id = fixture.save("Logos", &formats(&[AssetFormat::Png]));
    fixture.save("Icons", &AssetFilter::default());

    let taken =
        update(&mut fixture, id, "icons", formats(&[AssetFormat::Svg])).expect_err("name refused");
    assert!(matches!(taken, SmartCollectionError::NameTaken(_)));

    update(&mut fixture, id, "Vectors", formats(&[AssetFormat::Svg])).expect("updated");
    assert_eq!(fixture.names(), ["Icons", "Vectors"]);
    assert_eq!(fixture.shown(AssetView::Smart(id)), vec![svg]);

    fixture.library.undo().expect("undo works");
    assert_eq!(fixture.names(), ["Icons", "Logos"]);
    assert_eq!(fixture.shown(AssetView::Smart(id)), vec![png]);
}

#[test]
fn an_update_keeps_its_own_name_whatever_the_case() {
    let mut fixture = Fixture::new();
    let id = fixture.save("Logos", &AssetFilter::default());

    update(&mut fixture, id, "LOGOS", AssetFilter::text("logo")).expect("updated");

    assert_eq!(fixture.names(), ["LOGOS"]);
}

#[test]
fn deleting_keeps_the_resources_and_undo_restores_the_same_collection() {
    let mut fixture = Fixture::new();
    let red = fixture.import("red-dot.png");
    let filter = AssetFilter {
        text: "red".to_owned(),
        formats: vec![AssetFormat::Png],
        favorites_only: false,
        colors: vec![AssetColor::Red],
        custom_color: None,
        ..AssetFilter::default()
    };
    let id = fixture.save("Reds", &filter);
    let before = fixture
        .library
        .smart_collection(id)
        .expect("read")
        .expect("exists");

    fixture
        .library
        .apply_smart_collection_command(&SmartCollectionCommand::Delete { id })
        .expect("deleted");
    assert!(fixture.names().is_empty());
    assert_eq!(fixture.shown(AssetView::All), vec![red]);

    fixture.library.undo().expect("undo works");
    assert_eq!(
        fixture.library.smart_collection(id).expect("read"),
        Some(before)
    );
}

#[test]
fn undoing_a_creation_removes_the_smart_collection() {
    let mut fixture = Fixture::new();
    fixture.save("Logos", &AssetFilter::default());

    fixture.library.undo().expect("undo works");

    assert!(fixture.names().is_empty());
}

#[test]
fn smart_collections_survive_reopening_the_library() {
    let mut fixture = Fixture::new();
    let filter = AssetFilter {
        text: "logo".to_owned(),
        formats: vec![AssetFormat::Svg, AssetFormat::Ico],
        favorites_only: true,
        colors: vec![AssetColor::Blue, AssetColor::White],
        custom_color: Some(Rgb::new(0xc0, 0x39, 0x2b)),
        shapes: vec![AssetShape::Portrait, AssetShape::Square],
        fits_screen: true,
        screen: None,
    };
    let id = fixture.save("Logos", &filter);
    let saved = fixture.library.smart_collection(id).expect("read");
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);

    let reopened = Library::open(&root).expect("library reopens");

    assert_eq!(reopened.smart_collection(id).expect("read"), saved);
    drop(fixture.workspace);
}

fn arranged(fixture: &Fixture) -> Vec<String> {
    let mut collections = fixture
        .library
        .smart_collections()
        .expect("smart collections listed");
    collections.sort_by_key(|collection| collection.position);
    collections
        .into_iter()
        .map(|collection| collection.name)
        .collect()
}

#[test]
fn new_smart_collections_come_last_and_can_be_arranged_with_undo() {
    let mut fixture = Fixture::new();
    let first = fixture.save("Zèbres", &AssetFilter::text("z"));
    let second = fixture.save("Avions", &AssetFilter::text("a"));
    let third = fixture.save("Logos", &AssetFilter::text("l"));
    assert_eq!(arranged(&fixture), ["Zèbres", "Avions", "Logos"]);

    fixture
        .library
        .apply_smart_collection_command(&SmartCollectionCommand::Arrange {
            order: vec![third, first, second],
        })
        .expect("arranged");
    assert_eq!(arranged(&fixture), ["Logos", "Zèbres", "Avions"]);

    fixture.library.undo().expect("undo works");
    assert_eq!(arranged(&fixture), ["Zèbres", "Avions", "Logos"]);
}

#[test]
fn an_order_that_no_longer_matches_the_library_is_refused() {
    let mut fixture = Fixture::new();
    let first = fixture.save("Zèbres", &AssetFilter::text("z"));
    let second = fixture.save("Avions", &AssetFilter::text("a"));

    for order in [vec![first], vec![first, first], vec![second, first, first]] {
        let refused = fixture
            .library
            .apply_smart_collection_command(&SmartCollectionCommand::Arrange { order })
            .expect_err("order refused");
        assert!(matches!(refused, SmartCollectionError::OutdatedOrder));
    }
}

#[test]
fn a_smart_collection_fits_the_screen_the_library_is_shown_on() {
    let mut fixture = Fixture::new();
    let small = fixture.import("red-dot.png");
    let large = fixture.import("navy-tile.bmp");
    let filter = AssetFilter {
        fits_screen: true,
        ..AssetFilter::default()
    };
    let id = fixture.save("Wallpapers", &filter);

    fixture.library.set_screen(Dimensions::new(4, 3));
    assert_eq!(fixture.shown(AssetView::Smart(id)), vec![large]);

    fixture.library.set_screen(Dimensions::new(2, 2));
    assert_eq!(
        sorted(fixture.shown(AssetView::Smart(id))),
        sorted(vec![small, large])
    );
}
