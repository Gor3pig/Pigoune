use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetId, AssetView, Change, CollectionCommand, CollectionError, CollectionId,
    HISTORY_LIMIT, ImportOutcome, Library, TagCommand, TextField, UndoError,
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

    fn apply(&mut self, command: &AssetCommand) {
        self.library
            .apply_asset_command(command)
            .expect("command applies");
    }

    fn undo(&mut self) -> Option<Change> {
        self.library.undo().expect("undo succeeds")
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

    fn name_of(&self, asset: AssetId) -> String {
        self.library
            .asset(asset)
            .expect("asset is read")
            .expect("asset exists")
            .display_name
    }

    fn tag_names(&self) -> Vec<String> {
        self.library
            .tags()
            .expect("tags are listed")
            .into_iter()
            .map(|tag| tag.name)
            .collect()
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn trash(assets: &[AssetId]) -> AssetCommand {
    AssetCommand::SetTrashed {
        assets: assets.to_vec(),
        trashed: true,
    }
}

#[test]
fn a_new_library_has_nothing_to_undo() {
    let mut fixture = Fixture::new();

    assert!(!fixture.library.can_undo());
    assert_eq!(fixture.undo(), None);
}

#[test]
fn undoing_a_trashing_brings_the_resources_back() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    fixture.apply(&trash(&[dot]));

    let undone = fixture.undo();

    assert_eq!(undone, Some(Change::Asset(trash(&[dot]))));
    assert_eq!(fixture.shown(AssetView::All), vec![dot]);
    assert!(fixture.shown(AssetView::Trash).is_empty());
    assert!(!fixture.library.can_undo());
}

#[test]
fn successive_undos_go_back_from_the_latest_change() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    let rename = AssetCommand::Rename {
        asset: dot,
        name: "Point".to_owned(),
    };
    let note = AssetCommand::SetText {
        asset: dot,
        field: TextField::Note,
        value: "Rouge".to_owned(),
    };
    let favorite = AssetCommand::SetFavorite {
        assets: vec![dot],
        favorite: true,
    };
    fixture.apply(&rename);
    fixture.apply(&note);
    fixture.apply(&favorite);

    assert_eq!(fixture.undo(), Some(Change::Asset(favorite)));
    assert_eq!(fixture.undo(), Some(Change::Asset(note)));
    assert_eq!(fixture.undo(), Some(Change::Asset(rename)));

    let asset = fixture
        .library
        .asset(dot)
        .expect("asset is read")
        .expect("asset exists");
    assert_eq!(asset.display_name, "red-dot");
    assert_eq!(asset.note, "");
    assert!(!asset.is_favorite);
    assert!(!fixture.library.can_undo());
}

#[test]
fn a_change_that_changed_nothing_is_not_recorded() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    let favorite = AssetCommand::SetFavorite {
        assets: vec![dot],
        favorite: true,
    };
    fixture.apply(&favorite);
    fixture.apply(&favorite);
    fixture.apply(&AssetCommand::Rename {
        asset: dot,
        name: "red-dot".to_owned(),
    });

    assert_eq!(fixture.undo(), Some(Change::Asset(favorite)));
    assert!(!fixture.library.can_undo());
}

#[test]
fn undoing_tags_and_collections_restores_them() {
    let mut fixture = Fixture::new();
    let logos = fixture.collection("Logos");
    let icons = fixture.collection("Icônes");
    let dot = fixture.import("red-dot.png", Some(logos));
    fixture
        .library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![dot],
            name: "rouge".to_owned(),
        })
        .expect("tag is added");
    fixture
        .library
        .apply_collection_command(&CollectionCommand::MoveAssets {
            from: logos,
            to: icons,
            assets: vec![dot],
        })
        .expect("resource is moved");

    fixture.undo();
    assert_eq!(fixture.shown(AssetView::Collection(logos)), vec![dot]);
    assert!(fixture.shown(AssetView::Collection(icons)).is_empty());

    fixture.undo();
    assert!(fixture.tag_names().is_empty());
}

#[test]
fn undoing_a_collection_deletion_brings_back_its_resources() {
    let mut fixture = Fixture::new();
    let logos = fixture.collection("Logos");
    let dot = fixture.import("red-dot.png", Some(logos));
    fixture
        .library
        .apply_collection_command(&CollectionCommand::Trash { id: logos })
        .expect("collection is deleted");

    fixture.undo();

    assert!(fixture.library.collection(logos).expect("read").is_some());
    assert_eq!(fixture.shown(AssetView::Collection(logos)), vec![dot]);
    assert!(fixture.shown(AssetView::Trash).is_empty());
}

#[test]
fn emptying_the_trash_forgets_the_history() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    fixture.apply(&trash(&[dot]));

    fixture.library.empty_trash().expect("trash is emptied");

    assert!(!fixture.library.can_undo());
    assert_eq!(fixture.undo(), None);
}

#[test]
fn tagging_imported_resources_is_not_recorded() {
    let mut fixture = Fixture::new();
    let creation = TagCommand::Add {
        assets: vec![],
        name: "importés".to_owned(),
    };
    fixture
        .library
        .apply_tag_command(&creation)
        .expect("tag is created");
    let tag = fixture
        .library
        .tag_named("importés")
        .expect("tag is read")
        .expect("tag exists")
        .id;
    let svg = fixture.import("dark-circle.svg", None);

    fixture
        .library
        .tag_imported(tag, &[svg])
        .expect("imported resources are tagged");

    assert_eq!(fixture.shown(AssetView::Tag(tag)), vec![svg]);
    assert_eq!(fixture.undo(), Some(Change::Tag(creation)));
    assert!(!fixture.library.can_undo());
}

#[test]
fn the_history_keeps_only_the_latest_changes() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    for index in 0..=HISTORY_LIMIT {
        fixture.apply(&AssetCommand::Rename {
            asset: dot,
            name: format!("Point {index}"),
        });
    }

    let mut undone = 0;
    while fixture.undo().is_some() {
        undone += 1;
    }

    assert_eq!(undone, HISTORY_LIMIT);
    assert_eq!(fixture.name_of(dot), "Point 0");
}

#[test]
fn a_refused_undo_is_forgotten_and_changes_nothing() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    fixture.apply(&AssetCommand::Rename {
        asset: dot,
        name: "Point".to_owned(),
    });
    let logos = fixture.collection("Logos");
    fixture
        .library
        .apply_collection_command(&CollectionCommand::Rename {
            id: logos,
            name: "Marques".to_owned(),
        })
        .expect("collection is renamed");
    fixture.collection("Logos");

    let refused = fixture.library.undo();

    assert!(matches!(
        refused,
        Err(UndoError::Collection(CollectionError::NameTaken(_)))
    ));
    let renamed = fixture
        .library
        .collection(logos)
        .expect("read")
        .expect("collection exists");
    assert_eq!(renamed.name, "Marques");
    assert!(fixture.undo().is_some());
    assert_eq!(fixture.name_of(dot), "red-dot");
}

#[test]
fn each_recorded_change_gets_a_new_stamp() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    assert_eq!(fixture.library.latest_change(), None);

    fixture.apply(&trash(&[dot]));
    let first = fixture.library.latest_change();
    fixture.apply(&trash(&[dot]));
    assert_eq!(fixture.library.latest_change(), first);

    fixture.undo();
    fixture.apply(&trash(&[dot]));
    let second = fixture.library.latest_change();

    assert!(first.is_some());
    assert!(second.is_some());
    assert_ne!(first, second);
}

#[test]
fn a_change_is_undone_by_its_stamp_only_while_it_is_the_latest() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    fixture.apply(&trash(&[dot]));
    let trashing = fixture
        .library
        .latest_change()
        .expect("trashing is recorded");
    let svg = fixture.import("dark-circle.svg", None);
    let favorite = AssetCommand::SetFavorite {
        assets: vec![svg],
        favorite: true,
    };
    fixture.apply(&favorite);

    let stale = fixture
        .library
        .undo_change(trashing)
        .expect("stale undo is harmless");
    assert_eq!(stale, None);
    assert_eq!(fixture.shown(AssetView::Trash), vec![dot]);

    fixture.undo();
    let undone = fixture
        .library
        .undo_change(trashing)
        .expect("latest change is undone");
    assert_eq!(undone, Some(Change::Asset(trash(&[dot]))));
    assert_eq!(fixture.shown(AssetView::Trash), Vec::<AssetId>::new());
}
