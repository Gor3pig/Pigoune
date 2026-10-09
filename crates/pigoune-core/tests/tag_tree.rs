use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetFilter, AssetId, AssetView, CURRENT_FORMAT_VERSION, DATABASE_FILE_NAME, ImportOutcome,
    Library, TagCommand, TagError, TagId,
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

    fn add(&mut self, assets: &[AssetId], path: &str) {
        self.library
            .apply_tag_command(&TagCommand::Add {
                assets: assets.to_vec(),
                name: path.to_owned(),
            })
            .expect("tag is added");
    }

    fn apply(&mut self, command: &TagCommand) {
        self.library
            .apply_tag_command(command)
            .expect("command applies");
    }

    fn refused(&mut self, command: &TagCommand) -> TagError {
        self.library
            .apply_tag_command(command)
            .expect_err("command is refused")
    }

    fn at(&self, path: &str) -> TagId {
        let mut parent: Option<TagId> = None;
        for name in path.split('/') {
            let found = match parent {
                None => self.library.tag_named(name),
                Some(parent) => self.library.tag_named_in(parent, name),
            }
            .expect("tag is read")
            .unwrap_or_else(|| panic!("no tag at {path}"));
            parent = Some(found.id);
        }
        parent.expect("path is not empty")
    }

    fn paths(&self) -> BTreeSet<String> {
        let tags = self.library.tags().expect("tags are listed");
        tags.iter()
            .map(|tag| {
                self.library
                    .tag_path(tag.id)
                    .expect("path is read")
                    .iter()
                    .map(|step| step.name.as_str())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect()
    }

    fn shown(&self, view: AssetView) -> BTreeSet<AssetId> {
        self.library
            .visible_assets_in(view)
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

    fn undo(&mut self) {
        self.library
            .undo()
            .expect("undo succeeds")
            .expect("something to undo");
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn paths(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|path| (*path).to_owned()).collect()
}

fn set(ids: &[AssetId]) -> BTreeSet<AssetId> {
    ids.iter().copied().collect()
}

#[test]
fn adding_a_path_creates_the_missing_levels_and_undoing_removes_only_those() {
    let mut fixture = Fixture::new();
    let goat = fixture.import("red-dot.png");
    fixture.add(&[goat], "subject");
    fixture.add(&[goat], "Subject/animals/goat");

    assert_eq!(
        fixture.paths(),
        paths(&["subject", "subject/animals", "subject/animals/goat"])
    );
    assert_eq!(
        fixture
            .library
            .tags_of(goat)
            .expect("tags")
            .into_iter()
            .map(|tag| tag.name)
            .collect::<Vec<_>>(),
        ["goat", "subject"]
    );

    fixture.undo();

    assert_eq!(fixture.paths(), paths(&["subject"]));
}

#[test]
fn two_branches_can_use_the_same_name_but_two_siblings_cannot() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    fixture.add(&[asset], "style/water");
    fixture.add(&[asset], "subject/water");

    assert_eq!(
        fixture.paths(),
        paths(&["style", "style/water", "subject", "subject/water"])
    );

    let style_water = fixture.at("style/water");
    let subject = fixture.at("subject");
    let style = fixture.at("style");
    fixture.apply(&TagCommand::Add {
        assets: vec![],
        name: "style/sea".to_owned(),
    });
    let sea = fixture.at("style/sea");
    assert!(matches!(
        fixture.refused(&TagCommand::Rename {
            tag: sea,
            name: "Water".to_owned()
        }),
        TagError::NameTaken(_)
    ));
    assert!(matches!(
        fixture.refused(&TagCommand::Move {
            tag: style_water,
            parent: Some(subject)
        }),
        TagError::NameTaken(_)
    ));
    fixture.apply(&TagCommand::Move {
        tag: sea,
        parent: Some(subject),
    });
    assert_eq!(fixture.at("subject/sea"), sea);
    assert!(
        fixture
            .library
            .tag_named_in(style, "sea")
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_parent_shows_and_counts_the_assets_of_its_sub_tags_once() {
    let mut fixture = Fixture::new();
    let goat = fixture.import("red-dot.png");
    let cat = fixture.import("dark-circle.svg");
    let both = fixture.import("blue-photo.jpg");
    fixture.add(&[goat, both], "animals/goat");
    fixture.add(&[cat, both], "animals/cat");
    fixture.add(&[both], "animals");

    let animals = fixture.at("animals");
    assert_eq!(
        fixture.shown(AssetView::Tag(animals)),
        set(&[goat, cat, both])
    );
    assert_eq!(fixture.count(animals), 3);
    assert_eq!(fixture.count(fixture.at("animals/goat")), 2);
    assert_eq!(
        fixture.shown(AssetView::Tag(fixture.at("animals/cat"))),
        set(&[cat, both])
    );
}

#[test]
fn a_tag_cannot_be_moved_into_itself_or_below_itself_and_a_move_can_be_undone() {
    let mut fixture = Fixture::new();
    fixture.add(&[], "a/b/c");
    fixture.add(&[], "other");
    let a = fixture.at("a");
    let c = fixture.at("a/b/c");
    let other = fixture.at("other");

    assert!(matches!(
        fixture.refused(&TagCommand::Move {
            tag: a,
            parent: Some(c)
        }),
        TagError::Cycle
    ));
    assert!(matches!(
        fixture.refused(&TagCommand::Move {
            tag: a,
            parent: Some(a)
        }),
        TagError::Cycle
    ));

    fixture.apply(&TagCommand::Move {
        tag: c,
        parent: Some(other),
    });
    assert_eq!(fixture.paths(), paths(&["a", "a/b", "other", "other/c"]));
    fixture.apply(&TagCommand::Move {
        tag: c,
        parent: None,
    });
    assert_eq!(fixture.paths(), paths(&["a", "a/b", "other", "c"]));

    fixture.undo();
    fixture.undo();
    assert_eq!(fixture.paths(), paths(&["a", "a/b", "a/b/c", "other"]));
}

#[test]
fn deleting_a_tag_removes_its_sub_tags_and_undoing_restores_the_whole_branch() {
    let mut fixture = Fixture::new();
    let goat = fixture.import("red-dot.png");
    let cat = fixture.import("dark-circle.svg");
    fixture.add(&[goat], "animals/goat");
    fixture.add(&[cat], "animals/cat/kitten");
    let animals = fixture.at("animals");

    fixture.apply(&TagCommand::Delete { tag: animals });
    assert!(fixture.paths().is_empty());
    assert!(fixture.library.tags_of(goat).expect("tags").is_empty());

    fixture.undo();

    assert_eq!(
        fixture.paths(),
        paths(&[
            "animals",
            "animals/goat",
            "animals/cat",
            "animals/cat/kitten"
        ])
    );
    assert_eq!(
        fixture.shown(AssetView::Tag(fixture.at("animals"))),
        set(&[goat, cat])
    );
}

#[test]
fn dissolving_a_tag_lifts_its_sub_tags_one_level_and_undoing_puts_them_back() {
    let mut fixture = Fixture::new();
    let goat = fixture.import("red-dot.png");
    fixture.add(&[goat], "subject/animals/goat");
    fixture.add(&[goat], "subject/animals");
    let animals = fixture.at("subject/animals");

    fixture.apply(&TagCommand::Dissolve { tag: animals });
    assert_eq!(fixture.paths(), paths(&["subject", "subject/goat"]));
    assert_eq!(
        fixture.shown(AssetView::Tag(fixture.at("subject/goat"))),
        set(&[goat])
    );

    fixture.undo();
    assert_eq!(
        fixture.paths(),
        paths(&["subject", "subject/animals", "subject/animals/goat"])
    );
    assert_eq!(fixture.library.tags_of(goat).expect("tags").len(), 2);
}

#[test]
fn dissolving_is_refused_when_a_lifted_tag_would_clash_with_a_sibling() {
    let mut fixture = Fixture::new();
    fixture.add(&[], "subject/goat");
    fixture.add(&[], "subject/animals/goat");
    let animals = fixture.at("subject/animals");

    assert!(matches!(
        fixture.refused(&TagCommand::Dissolve { tag: animals }),
        TagError::NameTaken(_)
    ));
    assert_eq!(
        fixture.paths(),
        paths(&[
            "subject",
            "subject/goat",
            "subject/animals",
            "subject/animals/goat"
        ])
    );
}

#[test]
fn merging_moves_the_sub_tags_and_the_assets_and_can_be_undone() {
    let mut fixture = Fixture::new();
    let goat = fixture.import("red-dot.png");
    let cat = fixture.import("dark-circle.svg");
    fixture.add(&[goat], "beasts/goat");
    fixture.add(&[cat], "animals");
    fixture.add(&[cat], "beasts");
    let beasts = fixture.at("beasts");
    let animals = fixture.at("animals");

    fixture.apply(&TagCommand::Merge {
        from: beasts,
        into: animals,
    });
    assert_eq!(fixture.paths(), paths(&["animals", "animals/goat"]));
    assert_eq!(
        fixture.shown(AssetView::Tag(fixture.at("animals"))),
        set(&[goat, cat])
    );

    fixture.undo();
    assert_eq!(
        fixture.paths(),
        paths(&["animals", "beasts", "beasts/goat"])
    );
    assert_eq!(
        fixture.shown(AssetView::Tag(fixture.at("animals"))),
        set(&[cat])
    );

    let goat_tag = fixture.at("beasts/goat");
    assert!(matches!(
        fixture.refused(&TagCommand::Merge {
            from: fixture.at("beasts"),
            into: goat_tag
        }),
        TagError::Cycle
    ));
}

#[test]
fn searching_a_parent_name_finds_the_assets_of_its_sub_tags() {
    let mut fixture = Fixture::new();
    let goat = fixture.import("red-dot.png");
    let other = fixture.import("dark-circle.svg");
    fixture.add(&[goat], "animals/goat");
    fixture.add(&[other], "logo");

    let found: Vec<AssetId> = fixture
        .library
        .find_assets_in(AssetView::All, &AssetFilter::text("animals"))
        .expect("search succeeds")
        .into_iter()
        .map(|asset| asset.id)
        .collect();

    assert_eq!(found, [goat]);
}

#[test]
fn a_format_8_library_keeps_its_tags_as_top_level_tags() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    fixture.add(&[asset], "logo");
    fixture.add(&[asset], "social");
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .execute_batch(
            "CREATE TABLE old_tags (
                 id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL,
                 normalized_name TEXT NOT NULL UNIQUE) STRICT;
             INSERT INTO old_tags SELECT id, name, normalized_name FROM tags;
             CREATE TABLE old_asset_tags (
                 asset_id TEXT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
                 tag_id TEXT NOT NULL REFERENCES old_tags (id) ON DELETE CASCADE,
                 PRIMARY KEY (asset_id, tag_id)) STRICT, WITHOUT ROWID;
             INSERT INTO old_asset_tags SELECT asset_id, tag_id FROM asset_tags;
             DROP TABLE asset_tags;
             DROP TABLE tags;
             ALTER TABLE old_tags RENAME TO tags;
             ALTER TABLE old_asset_tags RENAME TO asset_tags;
             PRAGMA user_version = 8;",
        )
        .expect("old format is rebuilt");
    drop(connection);

    let mut library = Library::open(&root).expect("library opens");

    assert_eq!(
        library.format_version().expect("version readable"),
        CURRENT_FORMAT_VERSION
    );
    let tags = library.tags_of(asset).expect("tags are read");
    assert_eq!(
        tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        ["logo", "social"]
    );
    assert!(tags.iter().all(|tag| tag.parent.is_none()));
    library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![asset],
            name: "logo/flat".to_owned(),
        })
        .expect("sub-tags work after the upgrade");
}

fn rebuild_as_format_8(root: &Path, renames: &str) {
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .execute_batch(&format!(
            "CREATE TABLE old_tags (
                 id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL,
                 normalized_name TEXT NOT NULL UNIQUE) STRICT;
             INSERT INTO old_tags SELECT id, name, normalized_name FROM tags;
             CREATE TABLE old_asset_tags (
                 asset_id TEXT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
                 tag_id TEXT NOT NULL REFERENCES old_tags (id) ON DELETE CASCADE,
                 PRIMARY KEY (asset_id, tag_id)) STRICT, WITHOUT ROWID;
             INSERT INTO old_asset_tags SELECT asset_id, tag_id FROM asset_tags;
             DROP TABLE asset_tags;
             DROP TABLE tags;
             ALTER TABLE old_tags RENAME TO tags;
             ALTER TABLE old_asset_tags RENAME TO asset_tags;
             {renames}
             PRAGMA user_version = 8;"
        ))
        .expect("old format is rebuilt");
}

#[test]
fn an_old_tag_with_a_slash_gets_a_hyphen_and_joins_a_twin_tag() {
    let mut fixture = Fixture::new();
    let first = fixture.import("red-dot.png");
    let second = fixture.import("blue-photo.jpg");
    let third = fixture.import("green-square.webp");
    fixture.add(&[first], "noir-blanc");
    fixture.add(&[first, second], "noirXblanc");
    fixture.add(&[third], "aXbXc");
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);
    rebuild_as_format_8(
        &root,
        "UPDATE tags SET name = replace(name, 'X', '/'), normalized_name = replace(normalized_name, 'x', '/')
             WHERE name LIKE '%X%';",
    );

    let library = Library::open(&root).expect("library opens");

    let tags = library.tags().expect("tags are read");
    let mut names: Vec<&str> = tags.iter().map(|tag| tag.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["a-b-c", "noir-blanc"]);
    assert!(tags.iter().all(|tag| tag.parent.is_none()));
    let twin = tags
        .iter()
        .find(|tag| tag.name == "noir-blanc")
        .expect("tag kept");
    assert_eq!(
        library.view_count(AssetView::Tag(twin.id)).expect("count"),
        2
    );
    assert_eq!(library.tags_of(third).expect("tags")[0].name, "a-b-c");
}

#[test]
fn a_tag_cannot_be_renamed_with_a_slash() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    fixture.add(&[asset], "logo");
    let logo = fixture.at("logo");

    let refused = fixture.refused(&TagCommand::Rename {
        tag: logo,
        name: "logo/flat".to_owned(),
    });

    assert!(matches!(refused, TagError::ContainsSlash));
    assert_eq!(fixture.paths(), paths(&["logo"]));
}

#[test]
fn dissolving_a_tag_lifts_a_sub_tag_of_the_same_name_and_undoing_puts_it_back() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    fixture.add(&[asset], "x/x/leaf");
    let outer = fixture.at("x");

    fixture.apply(&TagCommand::Dissolve { tag: outer });

    assert_eq!(fixture.paths(), paths(&["x", "x/leaf"]));
    assert_eq!(
        fixture.shown(AssetView::Tag(fixture.at("x/leaf"))),
        set(&[asset])
    );
    fixture.undo();
    assert_eq!(fixture.paths(), paths(&["x", "x/x", "x/x/leaf"]));
}

#[test]
fn merging_a_tag_whose_sub_tag_has_its_name_and_undoing_restores_both() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    fixture.add(&[asset], "b/red/red");
    let from = fixture.at("b/red");
    let into = fixture.at("b");

    fixture.apply(&TagCommand::Merge { from, into });

    assert_eq!(fixture.paths(), paths(&["b", "b/red"]));
    assert_eq!(
        fixture.shown(AssetView::Tag(fixture.at("b/red"))),
        set(&[asset])
    );
    fixture.undo();
    assert_eq!(fixture.paths(), paths(&["b", "b/red", "b/red/red"]));
}

#[test]
fn a_smart_collection_searching_a_parent_name_follows_its_sub_tags() {
    let mut fixture = Fixture::new();
    let goat = fixture.import("red-dot.png");
    let other = fixture.import("dark-circle.svg");
    fixture.add(&[goat], "animals/goat");
    fixture.add(&[other], "logo");
    let id = fixture
        .library
        .create_smart_collection("Beasts", &AssetFilter::text("animals"))
        .expect("smart collection saved");

    assert_eq!(fixture.shown(AssetView::Smart(id)), set(&[goat]));
    assert_eq!(
        fixture
            .library
            .view_counts()
            .expect("counts")
            .of(AssetView::Smart(id)),
        1
    );
}
