use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetColor, AssetCommand, AssetFilter, AssetFormat, AssetId, AssetView, CollectionCommand,
    ImportOutcome, Library, SmartCollection, SmartCollectionCommand, SmartCollectionError,
    SmartCollectionId, TagCommand,
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

    fn save(&mut self, name: &str, scope: AssetView, filter: &AssetFilter) -> SmartCollectionId {
        self.library
            .create_smart_collection(name, scope, filter)
            .expect("smart collection is saved")
    }

    fn refused(&mut self, name: &str, scope: AssetView) -> SmartCollectionError {
        self.library
            .create_smart_collection(name, scope, &AssetFilter::default())
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

    let id = fixture.save(
        "Rasters",
        AssetView::All,
        &formats(&[AssetFormat::Png, AssetFormat::Webp]),
    );

    assert_eq!(fixture.shown(AssetView::Smart(id)), sorted(vec![png, webp]));
}

#[test]
fn a_resource_imported_later_appears_by_itself() {
    let mut fixture = Fixture::new();
    fixture.import("red-dot.png");
    let id = fixture.save("Vectors", AssetView::All, &formats(&[AssetFormat::Svg]));
    assert!(fixture.shown(AssetView::Smart(id)).is_empty());

    let svg = fixture.import("dark-circle.svg");

    assert_eq!(fixture.shown(AssetView::Smart(id)), vec![svg]);
}

#[test]
fn search_text_favorites_and_scope_all_narrow_the_result() {
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
            assets: vec![red, circle],
        })
        .expect("assets added");
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![red, blinking],
            favorite: true,
        })
        .expect("favorites set");

    let in_logos = fixture.save(
        "Favorite logos",
        AssetView::Collection(collection),
        &AssetFilter {
            favorites_only: true,
            ..AssetFilter::default()
        },
    );
    let named = fixture.save("Dots", AssetView::All, &AssetFilter::text("dot"));

    assert_eq!(fixture.shown(AssetView::Smart(in_logos)), vec![red]);
    assert_eq!(fixture.shown(AssetView::Smart(named)), vec![red]);
}

#[test]
fn a_search_inside_a_smart_collection_narrows_it_further() {
    let mut fixture = Fixture::new();
    let red = fixture.import("red-dot.png");
    fixture.import("blinking.png");
    let id = fixture.save("PNG", AssetView::All, &formats(&[AssetFormat::Png]));

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
    let id = fixture.save("PNG", AssetView::All, &formats(&[AssetFormat::Png]));

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
    let id = fixture.save("PNG", AssetView::All, &formats(&[AssetFormat::Png]));

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
fn a_smart_collection_whose_tag_is_gone_is_simply_empty() {
    let mut fixture = Fixture::new();
    let red = fixture.import("red-dot.png");
    fixture
        .library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![red],
            name: "rouge".to_owned(),
        })
        .expect("tag added");
    let tag = fixture
        .library
        .tag_named("rouge")
        .expect("tag read")
        .expect("tag exists")
        .id;
    let id = fixture.save("Red things", AssetView::Tag(tag), &AssetFilter::default());
    assert_eq!(fixture.shown(AssetView::Smart(id)), vec![red]);

    fixture
        .library
        .apply_tag_command(&TagCommand::Delete { tag })
        .expect("tag deleted");

    assert!(fixture.shown(AssetView::Smart(id)).is_empty());
}

#[test]
fn names_are_trimmed_required_and_unique_whatever_the_case() {
    let mut fixture = Fixture::new();
    fixture.save("  Logos  ", AssetView::All, &AssetFilter::default());

    assert_eq!(fixture.names(), ["Logos"]);
    assert!(matches!(
        fixture.refused("   ", AssetView::All),
        SmartCollectionError::InvalidName
    ));
    assert!(matches!(
        fixture.refused("LOGOS", AssetView::All),
        SmartCollectionError::NameTaken(_)
    ));
}

#[test]
fn the_trash_and_smart_collections_cannot_be_saved_as_a_scope() {
    let mut fixture = Fixture::new();
    let id = fixture.save("Everything", AssetView::All, &AssetFilter::default());

    assert!(matches!(
        fixture.refused("From the trash", AssetView::Trash),
        SmartCollectionError::InvalidScope
    ));
    assert!(matches!(
        fixture.refused("Nested", AssetView::Smart(id)),
        SmartCollectionError::InvalidScope
    ));
}

#[test]
fn smart_collections_are_listed_by_name() {
    let mut fixture = Fixture::new();
    fixture.save("zèbres", AssetView::All, &AssetFilter::default());
    fixture.save("Avions", AssetView::All, &AssetFilter::default());

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
                scope: AssetView::All,
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
    let id = fixture.save("Logos", AssetView::All, &formats(&[AssetFormat::Png]));
    fixture.save("Icons", AssetView::All, &AssetFilter::default());

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
    let id = fixture.save("Logos", AssetView::All, &AssetFilter::default());

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
    };
    let id = fixture.save("Reds", AssetView::Favorites, &filter);
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
    fixture.save("Logos", AssetView::All, &AssetFilter::default());

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
    };
    let id = fixture.save("Logos", AssetView::Unclassified, &filter);
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
    let first = fixture.save("Zèbres", AssetView::All, &AssetFilter::text("z"));
    let second = fixture.save("Avions", AssetView::All, &AssetFilter::text("a"));
    let third = fixture.save("Logos", AssetView::All, &AssetFilter::text("l"));
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
    let first = fixture.save("Zèbres", AssetView::All, &AssetFilter::text("z"));
    let second = fixture.save("Avions", AssetView::All, &AssetFilter::text("a"));

    for order in [vec![first], vec![first, first], vec![second, first, first]] {
        let refused = fixture
            .library
            .apply_smart_collection_command(&SmartCollectionCommand::Arrange { order })
            .expect_err("order refused");
        assert!(matches!(refused, SmartCollectionError::OutdatedOrder));
    }
}
