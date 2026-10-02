use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetId, AssetView, CollectionCommand, CollectionError, CollectionId, DATABASE_FILE_NAME,
    ImportOutcome, Library,
};
use rusqlite::Connection;
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    library: Library,
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    collections: Vec<(String, String, Option<String>, i64, bool)>,
    assets: Vec<(String, bool)>,
    links: Vec<(String, String)>,
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

    fn asset_in(&mut self, fixture_name: &str, collection: Option<CollectionId>) -> AssetId {
        match self
            .library
            .import_file(&fixture(fixture_name), collection)
            .expect("import succeeds")
        {
            ImportOutcome::Imported(id) => id,
            outcome => panic!("unexpected outcome {outcome:?}"),
        }
    }

    fn also_in(&mut self, asset: AssetId, collection: CollectionId, fixture_name: &str) {
        let outcome = self
            .library
            .import_file(&fixture(fixture_name), Some(collection))
            .expect("import succeeds");
        assert_eq!(outcome, ImportOutcome::AddedToCollection(asset));
    }

    fn apply(&mut self, command: &CollectionCommand) -> CollectionCommand {
        self.library
            .apply_collection_command(command)
            .expect("command applies")
    }

    fn refused(&mut self, command: &CollectionCommand) -> CollectionError {
        self.library
            .apply_collection_command(command)
            .expect_err("command is refused")
    }

    fn name_of(&self, id: CollectionId) -> String {
        self.library
            .collection(id)
            .expect("collection is read")
            .expect("collection exists")
            .name
    }

    fn parent_of(&self, id: CollectionId) -> Option<CollectionId> {
        self.library
            .collection(id)
            .expect("collection is read")
            .expect("collection exists")
            .parent
    }

    fn order(&self, parent: Option<CollectionId>) -> Vec<CollectionId> {
        self.library.sibling_order(parent).expect("order is read")
    }

    fn visible(&self) -> Vec<CollectionId> {
        let mut ids: Vec<CollectionId> = self
            .library
            .visible_collections()
            .expect("collections are listed")
            .into_iter()
            .map(|collection| collection.id)
            .collect();
        ids.sort();
        ids
    }

    fn collections_of(&self, asset: AssetId) -> Vec<CollectionId> {
        self.library
            .collections_of(asset)
            .expect("collections are read")
    }

    fn shows(&self, view: AssetView, asset: AssetId) -> bool {
        self.library
            .view_contains(view, asset)
            .expect("view is read")
    }

    fn is_trashed(&self, asset: AssetId) -> bool {
        self.library
            .asset(asset)
            .expect("asset is read")
            .expect("asset exists")
            .trashed_at_unix_ms
            .is_some()
    }

    fn snapshot(&self) -> Snapshot {
        let connection =
            Connection::open(self.library.root().join(DATABASE_FILE_NAME)).expect("database opens");
        let rows = |query: &str| -> Vec<Vec<rusqlite::types::Value>> {
            let mut statement = connection.prepare(query).expect("query prepares");
            let width = statement.column_count();
            statement
                .query_map([], |row| (0..width).map(|index| row.get(index)).collect())
                .expect("query runs")
                .collect::<Result<_, _>>()
                .expect("rows read")
        };
        let text = |value: &rusqlite::types::Value| match value {
            rusqlite::types::Value::Text(text) => text.clone(),
            other => panic!("unexpected value {other:?}"),
        };
        Snapshot {
            collections: rows(
                "SELECT id, name, parent_id, position, trashed_at_unix_ms IS NOT NULL
                 FROM collections ORDER BY id",
            )
            .iter()
            .map(|row| {
                let parent = match &row[2] {
                    rusqlite::types::Value::Null => None,
                    value => Some(text(value)),
                };
                let integer = |value: &rusqlite::types::Value| match value {
                    rusqlite::types::Value::Integer(number) => *number,
                    other => panic!("unexpected value {other:?}"),
                };
                (
                    text(&row[0]),
                    text(&row[1]),
                    parent,
                    integer(&row[3]),
                    integer(&row[4]) == 1,
                )
            })
            .collect(),
            assets: rows("SELECT id, trashed_at_unix_ms IS NOT NULL FROM assets ORDER BY id")
                .iter()
                .map(|row| {
                    (
                        text(&row[0]),
                        matches!(row[1], rusqlite::types::Value::Integer(1)),
                    )
                })
                .collect(),
            links: rows("SELECT asset_id, collection_id FROM asset_collections ORDER BY 1, 2")
                .iter()
                .map(|row| (text(&row[0]), text(&row[1])))
                .collect(),
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn sibling_collections_cannot_share_a_name_whatever_the_case() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    fixture.collection("Tech", Some(brands));

    let result = fixture.library.create_collection("TECH", Some(brands));

    assert!(
        matches!(result, Err(CollectionError::NameTaken(ref name)) if name == "TECH"),
        "{result:?}"
    );
    fixture.collection("Tech", None);
}

#[test]
fn a_new_collection_goes_after_its_siblings() {
    let mut fixture = Fixture::new();
    let first = fixture.collection("Zèbre", None);
    let second = fixture.collection("Avion", None);

    assert_eq!(fixture.order(None), [first, second]);
}

#[test]
fn renaming_can_be_undone() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech", None);
    let before = fixture.snapshot();

    let inverse = fixture.apply(&CollectionCommand::Rename {
        id: tech,
        name: "  Technologie ".to_owned(),
    });
    assert_eq!(fixture.name_of(tech), "Technologie");

    fixture.apply(&inverse);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn renaming_refuses_blank_or_taken_names_but_accepts_a_change_of_case() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech", None);
    fixture.collection("Design", None);

    let blank = fixture.refused(&CollectionCommand::Rename {
        id: tech,
        name: "  ".to_owned(),
    });
    let taken = fixture.refused(&CollectionCommand::Rename {
        id: tech,
        name: "design".to_owned(),
    });
    fixture.apply(&CollectionCommand::Rename {
        id: tech,
        name: "TECH".to_owned(),
    });

    assert!(matches!(blank, CollectionError::InvalidName), "{blank:?}");
    assert!(matches!(taken, CollectionError::NameTaken(_)), "{taken:?}");
    assert_eq!(fixture.name_of(tech), "TECH");
}

#[test]
fn moving_into_another_collection_and_back_restores_everything() {
    let mut fixture = Fixture::new();
    let first = fixture.collection("Premier", None);
    let moved = fixture.collection("Déplacé", None);
    let last = fixture.collection("Dernier", None);
    let target = fixture.collection("Cible", Some(first));
    let before = fixture.snapshot();

    let inverse = fixture.apply(&CollectionCommand::Move {
        id: moved,
        parent: Some(target),
    });
    assert_eq!(fixture.parent_of(moved), Some(target));
    assert_eq!(fixture.order(None), [first, last]);

    fixture.apply(&inverse);
    assert_eq!(fixture.snapshot(), before);
    assert_eq!(fixture.order(None), [first, moved, last]);
}

#[test]
fn a_collection_can_go_back_to_the_root() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    let tech = fixture.collection("Tech", Some(brands));

    fixture.apply(&CollectionCommand::Move {
        id: tech,
        parent: None,
    });

    assert_eq!(fixture.parent_of(tech), None);
    assert_eq!(fixture.order(None), [brands, tech]);
}

#[test]
fn a_collection_cannot_move_into_itself_or_its_descendants() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    let tech = fixture.collection("Tech", Some(brands));
    let old = fixture.collection("Anciens", Some(tech));

    for target in [brands, tech, old] {
        let refused = fixture.refused(&CollectionCommand::Move {
            id: brands,
            parent: Some(target),
        });
        assert!(
            matches!(refused, CollectionError::WouldContainItself),
            "{refused:?}"
        );
    }
}

#[test]
fn moving_next_to_a_namesake_is_refused() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    fixture.collection("Tech", Some(brands));
    let loose = fixture.collection("tech", None);

    let refused = fixture.refused(&CollectionCommand::Move {
        id: loose,
        parent: Some(brands),
    });

    assert!(
        matches!(refused, CollectionError::NameTaken(_)),
        "{refused:?}"
    );
    assert_eq!(fixture.parent_of(loose), None);
}

#[test]
fn a_custom_order_can_be_set_and_undone() {
    let mut fixture = Fixture::new();
    let first = fixture.collection("A", None);
    let second = fixture.collection("B", None);
    let third = fixture.collection("C", None);

    let inverse = fixture.apply(&CollectionCommand::Arrange {
        parent: None,
        order: vec![third, first, second],
    });
    assert_eq!(fixture.order(None), [third, first, second]);

    fixture.apply(&inverse);
    assert_eq!(fixture.order(None), [first, second, third]);
}

#[test]
fn an_outdated_order_is_refused() {
    let mut fixture = Fixture::new();
    let first = fixture.collection("A", None);
    let second = fixture.collection("B", None);
    fixture.collection("C", None);

    let refused = fixture.refused(&CollectionCommand::Arrange {
        parent: None,
        order: vec![second, first],
    });

    assert!(
        matches!(refused, CollectionError::OutdatedOrder),
        "{refused:?}"
    );
}

#[test]
fn trashing_a_collection_takes_its_tree_and_the_resources_found_only_there() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    let tech = fixture.collection("Tech", Some(brands));
    let kept = fixture.collection("Gardée", None);
    let only_inside = fixture.asset_in("red-dot.png", Some(tech));
    let twice_inside = fixture.asset_in("github-mark.svg", Some(brands));
    fixture.also_in(twice_inside, tech, "github-mark.svg");
    let also_outside = fixture.asset_in("spinner.gif", Some(tech));
    fixture.also_in(also_outside, kept, "spinner.gif");
    let unclassified = fixture.asset_in("still.gif", None);

    fixture.apply(&CollectionCommand::Trash { id: brands });

    assert_eq!(fixture.visible(), [kept]);
    assert!(fixture.is_trashed(only_inside));
    assert!(fixture.is_trashed(twice_inside));
    assert!(!fixture.is_trashed(also_outside));
    assert!(!fixture.is_trashed(unclassified));
}

#[test]
fn trashing_can_be_undone_and_redone_exactly() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    let tech = fixture.collection("Tech", Some(brands));
    fixture.asset_in("red-dot.png", Some(tech));
    fixture.asset_in("github-mark.svg", Some(brands));
    let before = fixture.snapshot();

    let undo = fixture.apply(&CollectionCommand::Trash { id: brands });
    let after = fixture.snapshot();
    let redo = fixture.apply(&undo);
    assert_eq!(fixture.snapshot(), before);

    fixture.apply(&redo);
    assert_eq!(fixture.snapshot(), after);
}

#[test]
fn a_trashed_collection_cannot_be_renamed_moved_or_trashed_again() {
    let mut fixture = Fixture::new();
    let gone = fixture.collection("Partie", None);
    fixture.apply(&CollectionCommand::Trash { id: gone });

    for command in [
        CollectionCommand::Rename {
            id: gone,
            name: "Autre".to_owned(),
        },
        CollectionCommand::Move {
            id: gone,
            parent: None,
        },
        CollectionCommand::Trash { id: gone },
    ] {
        let refused = fixture.refused(&command);
        assert!(
            matches!(refused, CollectionError::NotFound(id) if id == gone),
            "{refused:?}"
        );
    }
}

#[test]
fn a_batch_is_undone_in_reverse_order() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    let tech = fixture.collection("Tech", None);
    let before = fixture.snapshot();

    let inverse = fixture.apply(&CollectionCommand::Batch(vec![
        CollectionCommand::Move {
            id: tech,
            parent: Some(brands),
        },
        CollectionCommand::Rename {
            id: tech,
            name: "Technologie".to_owned(),
        },
    ]));
    fixture.apply(&inverse);

    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn a_refused_batch_changes_nothing() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    let tech = fixture.collection("Tech", None);
    let before = fixture.snapshot();

    fixture.refused(&CollectionCommand::Batch(vec![
        CollectionCommand::Rename {
            id: tech,
            name: "Nouveau".to_owned(),
        },
        CollectionCommand::Move {
            id: brands,
            parent: Some(brands),
        },
    ]));

    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn resources_can_be_added_to_a_collection_and_the_addition_undone() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech", None);
    let already_there = fixture.asset_in("red-dot.png", Some(tech));
    let newcomer = fixture.asset_in("github-mark.svg", None);
    let before = fixture.snapshot();

    let undo = fixture.apply(&CollectionCommand::AddAssets {
        collection: tech,
        assets: vec![already_there, newcomer],
    });

    assert_eq!(fixture.collections_of(newcomer), [tech]);
    assert_eq!(
        undo,
        CollectionCommand::RemoveAssets {
            collection: tech,
            assets: vec![newcomer],
        }
    );
    fixture.apply(&undo);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn resources_can_be_removed_from_a_collection_and_the_removal_undone() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech", None);
    let kept = fixture.collection("Gardée", None);
    let inside = fixture.asset_in("red-dot.png", Some(tech));
    fixture.also_in(inside, kept, "red-dot.png");
    let outside = fixture.asset_in("github-mark.svg", None);
    let before = fixture.snapshot();

    let undo = fixture.apply(&CollectionCommand::RemoveAssets {
        collection: tech,
        assets: vec![inside, outside],
    });

    assert_eq!(fixture.collections_of(inside), [kept]);
    assert!(!fixture.is_trashed(inside));
    assert_eq!(
        undo,
        CollectionCommand::AddAssets {
            collection: tech,
            assets: vec![inside],
        }
    );
    fixture.apply(&undo);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn a_trashed_collection_or_resource_cannot_be_linked() {
    let mut fixture = Fixture::new();
    let gone = fixture.collection("Partie", None);
    let tech = fixture.collection("Tech", None);
    let asset = fixture.asset_in("red-dot.png", None);
    let trashed = fixture.asset_in("github-mark.svg", None);
    fixture.apply(&CollectionCommand::Trash { id: gone });
    fixture.apply(&CollectionCommand::SetTrashed {
        collections: vec![],
        assets: vec![trashed],
        trashed: true,
    });
    let before = fixture.snapshot();

    for command in [
        CollectionCommand::AddAssets {
            collection: gone,
            assets: vec![asset],
        },
        CollectionCommand::RemoveAssets {
            collection: gone,
            assets: vec![asset],
        },
    ] {
        let refused = fixture.refused(&command);
        assert!(
            matches!(refused, CollectionError::NotFound(id) if id == gone),
            "{refused:?}"
        );
    }
    let refused = fixture.refused(&CollectionCommand::AddAssets {
        collection: tech,
        assets: vec![asset, trashed],
    });
    assert!(
        matches!(refused, CollectionError::AssetNotFound(id) if id == trashed),
        "{refused:?}"
    );
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn the_collections_of_a_resource_leave_out_trashed_ones() {
    let mut fixture = Fixture::new();
    let gone = fixture.collection("Partie", None);
    let kept = fixture.collection("Gardée", None);
    let asset = fixture.asset_in("red-dot.png", Some(gone));
    fixture.also_in(asset, kept, "red-dot.png");

    fixture.apply(&CollectionCommand::Trash { id: gone });

    assert_eq!(fixture.collections_of(asset), [kept]);
}

#[test]
fn collection_paths_start_from_the_root_and_follow_alphabetical_order() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    let tech = fixture.collection("tech", Some(brands));
    let audio = fixture.collection("Audio", Some(brands));
    let icons = fixture.collection("Icônes", None);
    let gone = fixture.collection("Partie", None);
    fixture.apply(&CollectionCommand::Trash { id: gone });

    let paths: Vec<(CollectionId, Vec<String>)> = fixture
        .library
        .collection_paths()
        .expect("paths are read")
        .into_iter()
        .map(|path| (path.id, path.names))
        .collect();

    let names = |list: &[&str]| list.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(
        paths,
        [
            (icons, names(&["Icônes"])),
            (brands, names(&["Marques"])),
            (audio, names(&["Marques", "Audio"])),
            (tech, names(&["Marques", "tech"])),
        ]
    );
}

#[test]
fn a_view_tells_whether_it_still_shows_a_resource() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Marques", None);
    let tech = fixture.collection("Tech", Some(brands));
    let asset = fixture.asset_in("red-dot.png", None);

    assert!(fixture.shows(AssetView::Unclassified, asset));
    assert!(!fixture.shows(AssetView::Collection(brands), asset));

    fixture.apply(&CollectionCommand::AddAssets {
        collection: tech,
        assets: vec![asset],
    });

    assert!(!fixture.shows(AssetView::Unclassified, asset));
    assert!(fixture.shows(AssetView::Collection(brands), asset));
    assert!(fixture.shows(AssetView::Collection(tech), asset));
    assert!(fixture.shows(AssetView::All, asset));
    assert!(!fixture.shows(AssetView::Favorites, asset));
}
