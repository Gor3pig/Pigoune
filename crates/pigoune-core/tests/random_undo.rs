use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetFilter, AssetId, AssetView, CollectionCommand, CollectionId,
    DATABASE_FILE_NAME, ImportOutcome, Library, SmartCollectionCommand, TagCommand, TagId,
    TextField,
};
use rusqlite::Connection;
use tempfile::TempDir;

const SEQUENCES: u64 = 150;
const GESTURES_PER_SEQUENCE: usize = 30;
const FILES: [&str; 5] = [
    "red-dot.png",
    "dark-circle.svg",
    "blue-photo.jpg",
    "navy-tile.bmp",
    "green-square.webp",
];

struct Dice(u64);

impl Dice {
    fn below(&mut self, limit: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).expect("a shifted value fits") % limit.max(1)
    }

    fn pick<T: Clone>(&mut self, choices: &[T]) -> T {
        choices[self.below(choices.len())].clone()
    }

    fn some_of<T: Clone>(&mut self, choices: &[T]) -> Vec<T> {
        if choices.is_empty() {
            return Vec::new();
        }
        let count = 1 + self.below(choices.len().min(3));
        (0..count).map(|_| self.pick(choices)).collect()
    }
}

struct Fixture {
    _workspace: TempDir,
    library: Library,
}

impl Fixture {
    fn new(seed: u64) -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
        let first = library.create_collection("C0", None).expect("collection");
        let second = library.create_collection("C1", None).expect("collection");
        let nested = library
            .create_collection("C2", Some(first))
            .expect("collection");
        let deepest = library
            .create_collection("C3", Some(nested))
            .expect("collection");
        let homes = [Some(first), Some(second), Some(nested), None, Some(deepest)];
        let mut dice = Dice(seed);
        let mut assets = Vec::new();
        for (index, file) in FILES.iter().enumerate() {
            let home = homes[(dice.below(homes.len()) + index) % homes.len()];
            match library
                .import_file(&fixture(file), home)
                .expect("import succeeds")
            {
                ImportOutcome::Imported(id) => assets.push(id),
                outcome => panic!("unexpected outcome {outcome:?}"),
            }
        }
        for (assets, name) in [(&assets[..2], "t0"), (&assets[1..3], "t1")] {
            library
                .apply_tag_command(&TagCommand::Add {
                    assets: assets.to_vec(),
                    name: name.to_owned(),
                })
                .expect("tag is added");
        }
        Self {
            _workspace: workspace,
            library,
        }
    }

    fn alive(&self) -> Vec<AssetId> {
        self.assets_in(AssetView::All)
    }

    fn trashed(&self) -> Vec<AssetId> {
        self.assets_in(AssetView::Trash)
    }

    fn assets_in(&self, view: AssetView) -> Vec<AssetId> {
        self.library
            .visible_assets_in(view)
            .expect("assets are listed")
            .into_iter()
            .map(|asset| asset.id)
            .collect()
    }

    fn collections(&self) -> Vec<CollectionId> {
        self.library
            .visible_collections()
            .expect("collections are listed")
            .into_iter()
            .map(|collection| collection.id)
            .collect()
    }

    fn tags(&self) -> Vec<TagId> {
        self.library
            .tags()
            .expect("tags are listed")
            .into_iter()
            .map(|tag| tag.id)
            .collect()
    }

    fn snapshot(&self) -> String {
        let connection =
            Connection::open(self.library.root().join(DATABASE_FILE_NAME)).expect("database opens");
        let mut text = String::new();
        for (title, query) in [
            (
                "assets",
                "SELECT id, display_name, is_favorite, note, trashed_at_unix_ms IS NOT NULL
                 FROM assets ORDER BY id",
            ),
            (
                "collections",
                "SELECT id, name, parent_id, trashed_at_unix_ms IS NOT NULL
                 FROM collections ORDER BY parent_id, position, created_at_unix_ms, id",
            ),
            (
                "links",
                "SELECT asset_id, collection_id FROM asset_collections ORDER BY 1, 2",
            ),
            ("tags", "SELECT id, name FROM tags ORDER BY id"),
            (
                "tag links",
                "SELECT asset_id, tag_id FROM asset_tags ORDER BY 1, 2",
            ),
            (
                "smart collections",
                "SELECT id, name, search_text FROM smart_collections ORDER BY id",
            ),
        ] {
            writeln!(text, "## {title}").expect("text is written");
            let mut statement = connection.prepare(query).expect("query prepares");
            let width = statement.column_count();
            let rows = statement
                .query_map([], |row| {
                    (0..width)
                        .map(|index| row.get::<_, rusqlite::types::Value>(index))
                        .collect::<Result<Vec<_>, _>>()
                })
                .expect("query runs");
            for row in rows {
                writeln!(text, "{:?}", row.expect("row is read")).expect("text is written");
            }
        }
        text
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn trash_or_restore(fixture: &mut Fixture, dice: &mut Dice) {
    let restoring = dice.below(2) == 0;
    let pool = if restoring {
        fixture.trashed()
    } else {
        fixture.alive()
    };
    let assets = dice.some_of(&pool);
    let _ = fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets,
            trashed: !restoring,
        });
}

fn edit_asset(fixture: &mut Fixture, dice: &mut Dice) {
    let alive = fixture.alive();
    if alive.is_empty() {
        return;
    }
    let command = match dice.below(3) {
        0 => AssetCommand::SetFavorite {
            assets: dice.some_of(&alive),
            favorite: dice.below(2) == 0,
        },
        1 => AssetCommand::Rename {
            asset: dice.pick(&alive),
            name: format!("name {}", dice.below(5)),
        },
        _ => AssetCommand::SetText {
            asset: dice.pick(&alive),
            field: TextField::Note,
            value: format!("note {}", dice.below(3)),
        },
    };
    let _ = fixture.library.apply_asset_command(&command);
}

fn arrange_assets(fixture: &mut Fixture, dice: &mut Dice) {
    let collections = fixture.collections();
    let alive = fixture.alive();
    if collections.is_empty() || alive.is_empty() {
        return;
    }
    let assets = dice.some_of(&alive);
    let command = match dice.below(3) {
        0 => CollectionCommand::AddAssets {
            collection: dice.pick(&collections),
            assets,
        },
        1 => CollectionCommand::RemoveAssets {
            collection: dice.pick(&collections),
            assets,
        },
        _ => CollectionCommand::MoveAssets {
            from: dice.pick(&collections),
            to: dice.pick(&collections),
            assets,
        },
    };
    let _ = fixture.library.apply_collection_command(&command);
}

fn edit_collections(fixture: &mut Fixture, dice: &mut Dice) {
    let collections = fixture.collections();
    let parent = if collections.is_empty() || dice.below(2) == 0 {
        None
    } else {
        Some(dice.pick(&collections))
    };
    if collections.is_empty() {
        let _ = fixture
            .library
            .create_collection(&format!("K{}", dice.below(6)), parent);
        return;
    }
    let target = dice.pick(&collections);
    let _ = match dice.below(5) {
        0 => fixture
            .library
            .create_collection(&format!("K{}", dice.below(6)), parent)
            .map(|_| ()),
        1 => fixture
            .library
            .apply_collection_command(&CollectionCommand::Rename {
                id: target,
                name: format!("R{}", dice.below(6)),
            })
            .map(|_| ()),
        2 => fixture
            .library
            .apply_collection_command(&CollectionCommand::Move { id: target, parent })
            .map(|_| ()),
        3 => fixture
            .library
            .apply_collection_command(&CollectionCommand::Trash { id: target })
            .map(|_| ()),
        _ => {
            let mut order = fixture.library.sibling_order(None).expect("order is read");
            order.reverse();
            fixture
                .library
                .apply_collection_command(&CollectionCommand::Arrange {
                    parent: None,
                    order,
                })
                .map(|_| ())
        }
    };
}

fn edit_tags(fixture: &mut Fixture, dice: &mut Dice) {
    let tags = fixture.tags();
    let alive = fixture.alive();
    let assets = dice.some_of(&alive);
    if tags.is_empty() || dice.below(3) == 0 {
        let _ = fixture.library.apply_tag_command(&TagCommand::Add {
            assets,
            name: format!("t{}", dice.below(5)),
        });
        return;
    }
    let tag = dice.pick(&tags);
    let other = dice.pick(&tags);
    let command = match dice.below(5) {
        0 => TagCommand::Link { tag, assets },
        1 => TagCommand::Unlink { tag, assets },
        2 => TagCommand::Rename {
            tag,
            name: format!("z{}", dice.below(4)),
        },
        3 => TagCommand::Merge {
            from: tag,
            into: other,
        },
        _ => TagCommand::Delete { tag },
    };
    let _ = fixture.library.apply_tag_command(&command);
}

fn edit_smart_collections(fixture: &mut Fixture, dice: &mut Dice) {
    let existing = fixture
        .library
        .smart_collections()
        .expect("smart collections are listed");
    if existing.is_empty() || dice.below(2) == 0 {
        let _ = fixture.library.create_smart_collection(
            &format!("S{}", dice.below(4)),
            &AssetFilter::text(&format!("name {}", dice.below(3))),
        );
        return;
    }
    let _ = fixture
        .library
        .apply_smart_collection_command(&SmartCollectionCommand::Delete {
            id: dice.pick(&existing).id,
        });
}

fn gesture(fixture: &mut Fixture, dice: &mut Dice) {
    match dice.below(6) {
        0 => trash_or_restore(fixture, dice),
        1 => edit_asset(fixture, dice),
        2 => arrange_assets(fixture, dice),
        3 => edit_collections(fixture, dice),
        4 => edit_tags(fixture, dice),
        _ => edit_smart_collections(fixture, dice),
    }
}

fn play(seed: u64) -> (Fixture, String, usize) {
    let mut fixture = Fixture::new(seed);
    let before = fixture.snapshot();
    let mut dice = Dice(seed.wrapping_mul(7919));
    let mut recorded = 0;
    for _ in 0..GESTURES_PER_SEQUENCE {
        let stamp = fixture.library.latest_change();
        gesture(&mut fixture, &mut dice);
        if fixture.library.latest_change() != stamp {
            recorded += 1;
        }
    }
    (fixture, before, recorded)
}

#[test]
fn undoing_every_random_gesture_restores_the_starting_state() {
    let mut recorded_total = 0;
    for seed in 1..=SEQUENCES {
        let (mut fixture, before, recorded) = play(seed);
        for step in 0..recorded {
            let undone = fixture
                .library
                .undo()
                .unwrap_or_else(|error| panic!("seed {seed}, undo {step}/{recorded}: {error}"));
            assert!(undone.is_some(), "seed {seed}: history ended at {step}");
        }
        assert_eq!(
            fixture.snapshot(),
            before,
            "seed {seed}: the state differs after undoing {recorded} gestures"
        );
        recorded_total += recorded;
    }
    assert!(
        recorded_total > 1000,
        "the sequences barely did anything: {recorded_total} gestures"
    );
}
