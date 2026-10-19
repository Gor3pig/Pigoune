use std::collections::HashMap;
use std::path::Path;

use pigoune_core::{
    AssetCommand, AssetFilter, AssetId, AssetView, CollectionCommand, CollectionId,
    DATABASE_FILE_NAME, Library, TagCommand, TagId, ViewCounts,
};
use rusqlite::{Connection, params, params_from_iter};
use tempfile::TempDir;

const ASSETS: usize = 90;
const SEQUENCES: u64 = 40;
const GESTURES: usize = 25;
const TAG_PATHS: [&str; 9] = [
    "Alpha",
    "Alpha/Beta",
    "Alpha/Beta/Gamma",
    "Alpha/Delta",
    "Epsilon",
    "Epsilon/Zeta",
    "Eta",
    "Theta/Iota",
    "Theta/Iota/Kappa",
];
const WORDS: [&str; 4] = ["logo", "photo", "icon", "banner"];

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
        let count = 1 + self.below(choices.len().min(12));
        (0..count).map(|_| self.pick(choices)).collect()
    }
}

fn asset_id(number: usize) -> AssetId {
    AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
}

fn insert_assets(root: &Path) {
    let mut connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    let transaction = connection.transaction().expect("transaction starts");
    for number in 1..=ASSETS {
        transaction
            .execute(
                "INSERT INTO assets (id, display_name, original_file_name, stored_path, format,
                                     width, height, byte_size, content_hash, is_animated,
                                     embedded_sizes, added_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, 'png', 8, 8, 100, ?5, 0, '', ?6)",
                params![
                    asset_id(number).to_string(),
                    format!("{} {number}", WORDS[number % WORDS.len()]),
                    format!("asset-{number}.png"),
                    format!("files/zz/{number}/asset-{number}.png"),
                    format!("hash-{number}"),
                    i64::try_from(number).expect("small"),
                ],
            )
            .expect("asset is inserted");
    }
    transaction.commit().expect("transaction commits");
}

struct Fixture {
    _workspace: TempDir,
    library: Library,
    assets: Vec<AssetId>,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        let root = library.root().to_path_buf();
        drop(library);
        insert_assets(&root);
        let mut library = Library::open(&root).expect("library opens");
        let first = library.create_collection("C0", None).expect("collection");
        let second = library.create_collection("C1", None).expect("collection");
        let nested = library
            .create_collection("C2", Some(first))
            .expect("collection");
        library
            .create_collection("C3", Some(nested))
            .expect("collection");
        library
            .create_collection("C4", Some(second))
            .expect("collection");
        library
            .create_smart_collection("Logos", &AssetFilter::text("logo"))
            .expect("smart collection");
        library
            .create_smart_collection(
                "Favorite icons",
                &AssetFilter {
                    favorites_only: true,
                    ..AssetFilter::text("icon")
                },
            )
            .expect("smart collection");
        Self {
            _workspace: workspace,
            library,
            assets: (1..=ASSETS).map(asset_id).collect(),
        }
    }

    fn connection(&self) -> Connection {
        Connection::open(self.library.root().join(DATABASE_FILE_NAME)).expect("database opens")
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

    fn alive(&self) -> Vec<AssetId> {
        self.library
            .visible_assets_in(AssetView::All)
            .expect("assets are listed")
            .into_iter()
            .map(|asset| asset.id)
            .collect()
    }

    fn trashed(&self) -> Vec<AssetId> {
        self.library
            .visible_assets_in(AssetView::Trash)
            .expect("trash is listed")
            .into_iter()
            .map(|asset| asset.id)
            .collect()
    }
}

struct LegacyCounts {
    collections: HashMap<String, usize>,
    tags: HashMap<String, usize>,
}

fn legacy_counts(connection: &Connection, ids: Option<&str>) -> LegacyCounts {
    let restriction = if ids.is_some() {
        " AND assets.id IN (SELECT value FROM json_each(?1))"
    } else {
        ""
    };
    let collections_query = format!(
        "WITH RECURSIVE ancestry(collection_id, ancestor_id) AS (
             SELECT id, id FROM collections WHERE trashed_at_unix_ms IS NULL
             UNION
             SELECT ancestry.collection_id, collections.parent_id FROM ancestry
             JOIN collections ON collections.id = ancestry.ancestor_id
             WHERE collections.parent_id IS NOT NULL
         )
         SELECT ancestry.ancestor_id, count(DISTINCT assets.id) FROM ancestry
         JOIN asset_collections ON asset_collections.collection_id = ancestry.collection_id
         JOIN assets ON assets.id = asset_collections.asset_id
         WHERE assets.trashed_at_unix_ms IS NULL{restriction}
         GROUP BY ancestry.ancestor_id"
    );
    let tags_query = format!(
        "WITH RECURSIVE reach(tag_id, ancestor_id) AS (
             SELECT id, id FROM tags
             UNION
             SELECT reach.tag_id, tags.parent_id FROM reach
             JOIN tags ON tags.id = reach.ancestor_id
             WHERE tags.parent_id IS NOT NULL
         )
         SELECT reach.ancestor_id, count(DISTINCT assets.id) FROM reach
         LEFT JOIN asset_tags ON asset_tags.tag_id = reach.tag_id
         LEFT JOIN assets ON assets.id = asset_tags.asset_id
             AND assets.trashed_at_unix_ms IS NULL{restriction}
         GROUP BY reach.ancestor_id"
    );
    let read = |query: &str| -> HashMap<String, usize> {
        let mut statement = connection.prepare(query).expect("query prepares");
        statement
            .query_map(params_from_iter(ids), |row| {
                let total: i64 = row.get(1)?;
                Ok((
                    row.get::<_, String>(0)?,
                    usize::try_from(total).expect("a count is not negative"),
                ))
            })
            .expect("query runs")
            .collect::<Result<_, _>>()
            .expect("rows are read")
    };
    LegacyCounts {
        collections: read(&collections_query),
        tags: read(&tags_query),
    }
}

fn names<K: ToString>(counts: &HashMap<K, usize>) -> HashMap<String, usize> {
    counts
        .iter()
        .map(|(id, total)| (id.to_string(), *total))
        .collect()
}

fn filters() -> Vec<(&'static str, AssetFilter)> {
    vec![
        ("none", AssetFilter::default()),
        ("text", AssetFilter::text("logo")),
        ("one word", AssetFilter::text("banner 7")),
        ("nothing", AssetFilter::text("zzzz")),
        (
            "favorites",
            AssetFilter {
                favorites_only: true,
                ..AssetFilter::default()
            },
        ),
        (
            "favorite text",
            AssetFilter {
                favorites_only: true,
                ..AssetFilter::text("photo")
            },
        ),
    ]
}

fn check_counts(fixture: &Fixture, context: &str) {
    let connection = fixture.connection();
    for (label, filter) in filters() {
        let counts: ViewCounts = fixture
            .library
            .view_counts_matching(&filter)
            .expect("counts are computed");
        let ids = if filter.narrows() {
            let matching: Vec<String> = fixture
                .library
                .find_assets_in(AssetView::All, &filter)
                .expect("assets are searched")
                .iter()
                .map(|asset| format!("\"{}\"", asset.id))
                .collect();
            Some(format!("[{}]", matching.join(",")))
        } else {
            None
        };
        let legacy = legacy_counts(&connection, ids.as_deref());
        assert_eq!(
            names(&counts.collections),
            legacy.collections,
            "collection counts differ ({label}, {context})"
        );
        assert_eq!(
            names(&counts.tags),
            legacy.tags,
            "tag counts differ ({label}, {context})"
        );
    }
    let counts = fixture.library.view_counts().expect("counts are computed");
    for tag in fixture.tags() {
        assert_eq!(
            counts.of(AssetView::Tag(tag)),
            fixture
                .library
                .view_count(AssetView::Tag(tag))
                .expect("tag is counted"),
            "tag count differs from the view ({context})"
        );
    }
    for collection in fixture.collections() {
        assert_eq!(
            counts.of(AssetView::Collection(collection)),
            fixture
                .library
                .view_count(AssetView::Collection(collection))
                .expect("collection is counted"),
            "collection count differs from the view ({context})"
        );
    }
    assert_eq!(
        counts.all,
        fixture.alive().len(),
        "all count differs ({context})"
    );
    assert_eq!(
        counts.trash,
        fixture.trashed().len(),
        "trash count differs ({context})"
    );
}

fn gesture(fixture: &mut Fixture, dice: &mut Dice) {
    let collections = fixture.collections();
    let tags = fixture.tags();
    let alive = fixture.alive();
    let trashed = fixture.trashed();
    match dice.below(10) {
        0 | 1 if !collections.is_empty() => {
            let _ = fixture
                .library
                .apply_collection_command(&CollectionCommand::AddAssets {
                    collection: dice.pick(&collections),
                    assets: dice.some_of(&fixture.assets),
                });
        }
        2 if !collections.is_empty() => {
            let _ = fixture
                .library
                .apply_collection_command(&CollectionCommand::RemoveAssets {
                    collection: dice.pick(&collections),
                    assets: dice.some_of(&fixture.assets),
                });
        }
        3 | 4 => {
            let _ = fixture.library.apply_tag_command(&TagCommand::Add {
                assets: dice.some_of(&fixture.assets),
                name: dice.pick(&TAG_PATHS).to_owned(),
            });
        }
        5 if !tags.is_empty() => {
            let _ = fixture.library.apply_tag_command(&TagCommand::Unlink {
                tag: dice.pick(&tags),
                assets: dice.some_of(&fixture.assets),
            });
        }
        6 => {
            let _ = fixture
                .library
                .apply_asset_command(&AssetCommand::SetFavorite {
                    assets: dice.some_of(&fixture.assets),
                    favorite: dice.below(2) == 0,
                });
        }
        7 if !alive.is_empty() => {
            let _ = fixture
                .library
                .apply_asset_command(&AssetCommand::SetTrashed {
                    assets: dice.some_of(&alive),
                    trashed: true,
                });
        }
        8 if !trashed.is_empty() => {
            let _ = fixture
                .library
                .apply_asset_command(&AssetCommand::SetTrashed {
                    assets: dice.some_of(&trashed),
                    trashed: false,
                });
        }
        9 if collections.len() > 2 => {
            let _ = fixture
                .library
                .apply_collection_command(&CollectionCommand::Trash {
                    id: dice.pick(&collections),
                });
        }
        _ => {}
    }
}

#[test]
fn counts_match_the_reference_queries_after_random_gestures_undo_and_redo() {
    for seed in 1..=SEQUENCES {
        let mut fixture = Fixture::new();
        let mut dice = Dice(seed);
        check_counts(&fixture, &format!("seed {seed}, start"));
        for step in 0..GESTURES {
            gesture(&mut fixture, &mut dice);
            check_counts(&fixture, &format!("seed {seed}, gesture {step}"));
            if step % 5 == 4 {
                for undone in 0..3 {
                    let _ = fixture.library.undo();
                    check_counts(
                        &fixture,
                        &format!("seed {seed}, undo {undone} after {step}"),
                    );
                }
                for redone in 0..2 {
                    let _ = fixture.library.redo();
                    check_counts(
                        &fixture,
                        &format!("seed {seed}, redo {redone} after {step}"),
                    );
                }
            }
        }
    }
}

#[test]
fn a_tag_whose_assets_are_all_trashed_or_that_holds_none_is_still_counted_as_zero() {
    let mut fixture = Fixture::new();
    let first = fixture.assets[0];
    let second = fixture.assets[1];
    fixture
        .library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![first, second],
            name: "Alpha/Beta".to_owned(),
        })
        .expect("assets are tagged");
    fixture
        .library
        .apply_tag_command(&TagCommand::Add {
            assets: Vec::new(),
            name: "Empty/Leaf".to_owned(),
        })
        .expect("an empty tag is created");
    check_counts(&fixture, "tags with and without assets");
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![first, second],
            trashed: true,
        })
        .expect("assets are trashed");
    check_counts(&fixture, "every tagged asset is trashed");
    let counts = fixture.library.view_counts().expect("counts are computed");
    let tags = fixture.tags();
    assert!(!tags.is_empty());
    assert!(
        tags.iter()
            .all(|tag| counts.tags.get(tag).copied() == Some(0)),
        "every tag keeps an entry counted as zero"
    );
}

#[test]
fn a_link_to_a_missing_asset_cannot_exist_so_the_cheaper_query_stays_exact() {
    let mut fixture = Fixture::new();
    fixture
        .library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![fixture.assets[0]],
            name: "Solo".to_owned(),
        })
        .expect("a tag is created");
    let tag = fixture.tags().first().copied().expect("a tag exists");
    let connection = fixture.connection();
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("keys are enforced");
    let refused = connection
        .execute(
            "INSERT INTO asset_tags (asset_id, tag_id) VALUES (?1, ?2)",
            params!["00000000-0000-7000-8000-999999999999", tag.to_string()],
        )
        .expect_err("the database refuses a link to nothing");
    assert!(
        refused.to_string().contains("FOREIGN KEY"),
        "unexpected refusal: {refused}"
    );
}
