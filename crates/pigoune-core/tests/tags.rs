use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetId, AssetView, DATABASE_FILE_NAME, ImportOutcome, Library, TagCommand, TagError, TagId,
};
use rusqlite::Connection;
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    library: Library,
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    tags: Vec<(String, String)>,
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

    fn apply(&mut self, command: &TagCommand) -> TagCommand {
        self.library
            .apply_tag_command(command)
            .expect("command applies")
    }

    fn refused(&mut self, command: &TagCommand) -> TagError {
        self.library
            .apply_tag_command(command)
            .expect_err("command is refused")
    }

    fn tag(&mut self, assets: &[AssetId], name: &str) -> TagId {
        self.apply(&TagCommand::Add {
            assets: assets.to_vec(),
            name: name.to_owned(),
        });
        self.library
            .tag_named(name)
            .expect("tag is read")
            .expect("tag exists")
            .id
    }

    fn names_of(&self, asset: AssetId) -> Vec<String> {
        self.library
            .tags_of(asset)
            .expect("tags are read")
            .into_iter()
            .map(|tag| tag.name)
            .collect()
    }

    fn all_names(&self) -> Vec<String> {
        self.library
            .tags()
            .expect("tags are listed")
            .into_iter()
            .map(|tag| tag.name)
            .collect()
    }

    fn shown(&self, tag: TagId) -> BTreeSet<AssetId> {
        self.library
            .visible_assets_in(AssetView::Tag(tag))
            .expect("assets are listed")
            .into_iter()
            .map(|asset| asset.id)
            .collect()
    }

    fn count(&self, tag: TagId) -> usize {
        self.library
            .view_counts()
            .expect("counts read")
            .of(AssetView::Tag(tag))
    }

    fn snapshot(&self) -> Snapshot {
        let connection =
            Connection::open(self.library.root().join(DATABASE_FILE_NAME)).expect("database opens");
        let pairs = |query: &str| -> Vec<(String, String)> {
            let mut statement = connection.prepare(query).expect("query prepares");
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("query runs")
                .collect::<Result<_, _>>()
                .expect("rows read")
        };
        Snapshot {
            tags: pairs("SELECT id, name FROM tags ORDER BY id"),
            links: pairs("SELECT asset_id, tag_id FROM asset_tags ORDER BY 1, 2"),
        }
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

#[test]
fn a_tag_is_shared_whatever_the_case_and_keeps_its_first_writing() {
    let mut fixture = Fixture::new();
    let first = fixture.import("red-dot.png");
    let second = fixture.import("dark-circle.svg");

    let logo = fixture.tag(&[first], "  Logo ");
    let again = fixture.tag(&[second], "logo");

    assert_eq!(logo, again);
    assert_eq!(fixture.all_names(), ["Logo"]);
    assert_eq!(fixture.names_of(second), ["Logo"]);
    assert_eq!(fixture.shown(logo), set(&[first, second]));
    assert_eq!(fixture.count(logo), 2);
}

#[test]
fn a_resource_lists_its_tags_alphabetically() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    fixture.tag(&[asset], "rouge");
    fixture.tag(&[asset], "Brillant");

    assert_eq!(fixture.names_of(asset), ["Brillant", "rouge"]);
}

#[test]
fn an_unused_tag_stays_with_a_count_of_zero() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    let tag = fixture.tag(&[asset], "rouge");

    fixture.apply(&TagCommand::Unlink {
        tag,
        assets: vec![asset],
    });

    assert_eq!(fixture.all_names(), ["rouge"]);
    assert_eq!(fixture.count(tag), 0);
    assert!(fixture.names_of(asset).is_empty());
}

#[test]
fn adding_can_be_undone_including_the_new_tag() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    let before = fixture.snapshot();

    let undo = fixture.apply(&TagCommand::Add {
        assets: vec![asset],
        name: "rouge".to_owned(),
    });
    let after = fixture.snapshot();
    let redo = fixture.apply(&undo);
    assert_eq!(fixture.snapshot(), before);

    fixture.apply(&redo);
    assert_eq!(fixture.snapshot(), after);
}

#[test]
fn adding_an_existing_tag_can_be_undone_without_losing_the_tag() {
    let mut fixture = Fixture::new();
    let first = fixture.import("red-dot.png");
    let second = fixture.import("dark-circle.svg");
    fixture.tag(&[first], "logo");
    let before = fixture.snapshot();

    let undo = fixture.apply(&TagCommand::Add {
        assets: vec![first, second],
        name: "LOGO".to_owned(),
    });
    fixture.apply(&undo);

    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn renaming_can_be_undone_and_refuses_a_taken_name() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    let red = fixture.tag(&[asset], "rouge");
    let logo = fixture.tag(&[asset], "logo");
    let before = fixture.snapshot();

    let undo = fixture.apply(&TagCommand::Rename {
        tag: red,
        name: "Rouge-vif".to_owned(),
    });
    assert_eq!(fixture.names_of(asset), ["logo", "Rouge-vif"]);
    fixture.apply(&undo);
    assert_eq!(fixture.snapshot(), before);

    let taken = fixture.refused(&TagCommand::Rename {
        tag: red,
        name: "LOGO".to_owned(),
    });
    assert!(
        matches!(taken, TagError::NameTaken(existing) if existing == logo),
        "{taken:?}"
    );
}

#[test]
fn merging_joins_the_resources_and_can_be_undone() {
    let mut fixture = Fixture::new();
    let first = fixture.import("red-dot.png");
    let both = fixture.import("dark-circle.svg");
    let second = fixture.import("spinner.gif");
    let logos = fixture.tag(&[first, both], "logos");
    let logo = fixture.tag(&[both, second], "logo");
    let before = fixture.snapshot();

    let undo = fixture.apply(&TagCommand::Merge {
        from: logos,
        into: logo,
    });
    assert_eq!(fixture.all_names(), ["logo"]);
    assert_eq!(fixture.shown(logo), set(&[first, both, second]));

    fixture.apply(&undo);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn deleting_removes_the_tag_from_every_resource_and_can_be_undone() {
    let mut fixture = Fixture::new();
    let first = fixture.import("red-dot.png");
    let second = fixture.import("dark-circle.svg");
    let tag = fixture.tag(&[first, second], "logo");
    let before = fixture.snapshot();

    let undo = fixture.apply(&TagCommand::Delete { tag });
    assert!(fixture.all_names().is_empty());
    assert!(fixture.names_of(first).is_empty());
    assert!(fixture.library.asset(first).expect("read").is_some());

    fixture.apply(&undo);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn blank_names_trashed_resources_and_unknown_tags_are_refused_without_change() {
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
    let tag = fixture.tag(&[kept], "logo");
    fixture.apply(&TagCommand::Delete { tag });
    let before = fixture.snapshot();

    let blank = fixture.refused(&TagCommand::Add {
        assets: vec![kept],
        name: "  ".to_owned(),
    });
    let on_trash = fixture.refused(&TagCommand::Add {
        assets: vec![kept, trashed],
        name: "rouge".to_owned(),
    });
    let unknown = fixture.refused(&TagCommand::Rename {
        tag,
        name: "autre".to_owned(),
    });

    assert!(matches!(blank, TagError::InvalidName), "{blank:?}");
    assert!(
        matches!(on_trash, TagError::AssetNotFound(id) if id == trashed),
        "{on_trash:?}"
    );
    assert!(
        matches!(unknown, TagError::NotFound(id) if id == tag),
        "{unknown:?}"
    );
    assert_eq!(fixture.snapshot(), before);
}

fn insert_old_tag(fixture: &Fixture, name: &str) -> TagId {
    let id = TagId::parse("00000000-0000-7000-8000-000000000001").expect("valid id");
    let connection =
        Connection::open(fixture.library.root().join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .execute(
            "INSERT INTO tags (id, name, normalized_name) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, name, name.to_lowercase()],
        )
        .expect("old tag inserted");
    id
}

#[test]
fn a_new_tag_is_one_word_of_at_most_twenty_characters() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    let add = |name: &str| TagCommand::Add {
        assets: vec![asset],
        name: name.to_owned(),
    };

    fixture.apply(&add("vingt-caracteres-ici"));
    fixture.apply(&add("d'écran"));
    fixture.apply(&add("œuvre_2"));
    let before = fixture.snapshot();

    assert!(matches!(
        fixture.refused(&add("vingt-et-un-caracteres")),
        TagError::TooLong
    ));
    for name in ["fond d'écran", "deux\tmots", "ligne\nsuivante", "a\u{a0}b"] {
        assert!(
            matches!(fixture.refused(&add(name)), TagError::NotOneWord),
            "{name:?}"
        );
    }
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn characters_are_counted_not_bytes() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");

    fixture.tag(&[asset], &"é".repeat(20));
    fixture.tag(&[asset], &"猫".repeat(20));

    assert!(matches!(
        fixture.refused(&TagCommand::Add {
            assets: vec![asset],
            name: "é".repeat(21),
        }),
        TagError::TooLong
    ));
}

#[test]
fn renaming_must_follow_the_same_rule() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    let red = fixture.tag(&[asset], "rouge");
    let before = fixture.snapshot();

    for (name, expected_too_long) in [("rouge vif", false), (&"x".repeat(21)[..], true)] {
        let refused = fixture.refused(&TagCommand::Rename {
            tag: red,
            name: name.to_owned(),
        });
        assert_eq!(matches!(refused, TagError::TooLong), expected_too_long);
        assert_eq!(matches!(refused, TagError::NotOneWord), !expected_too_long);
    }
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn an_old_tag_that_breaks_the_rule_stays_usable() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    let other = fixture.import("dark-circle.svg");
    let old = insert_old_tag(&fixture, "illustration vectorielle très détaillée");

    let undo = fixture.apply(&TagCommand::Add {
        assets: vec![asset],
        name: "Illustration vectorielle très détaillée".to_owned(),
    });
    assert_eq!(fixture.shown(old), set(&[asset]));
    fixture.apply(&TagCommand::Link {
        tag: old,
        assets: vec![other],
    });
    let recreate = fixture.apply(&TagCommand::Delete { tag: old });
    fixture.apply(&recreate);
    fixture.apply(&undo);

    assert_eq!(fixture.shown(old), set(&[other]));
}

#[test]
fn composed_and_decomposed_accents_name_the_same_tag() {
    let mut fixture = Fixture::new();
    let first = fixture.import("red-dot.png");
    let second = fixture.import("dark-circle.svg");

    let composed = fixture.tag(&[first], "caf\u{e9}");
    let decomposed = fixture.tag(&[second], "cafe\u{301}");

    assert_eq!(composed, decomposed);
    assert_eq!(fixture.all_names().len(), 1);
    assert_eq!(fixture.count(composed), 2);
}
