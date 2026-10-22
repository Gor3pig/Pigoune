use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, BLUR, Geometry, Options, Rect, Shape,
};
use pigoune_core::{
    Asset, AssetColor, AssetCommand, AssetFilter, AssetId, AssetView, CollectionCommand,
    CollectionId, DATABASE_FILE_NAME, DominantColor, ImportControl, Library, Rgb,
    SmartCollectionCommand, SmartCollectionId, Tag, TagCommand, TagId, TextField, ViewCounts,
};
use rusqlite::{Connection, params};
use tempfile::TempDir;

const ASSETS: usize = 70;
const SEQUENCES: u64 = 10;
const GESTURES: usize = 36;
const TAG_PATHS: [&str; 8] = [
    "Alpha",
    "Alpha/Beta",
    "Alpha/Beta/Gamma",
    "Alpha/Delta",
    "Epsilon",
    "Epsilon/Zeta",
    "Eta",
    "Theta/Iota",
];
const WORDS: [&str; 4] = ["logo", "photo", "icon", "banner"];
const FIXTURES: [&str; 5] = [
    "red-dot.png",
    "dark-circle.svg",
    "blue-photo.jpg",
    "navy-tile.bmp",
    "green-square.webp",
];
const FAMILIES: [AssetColor; 4] = [
    AssetColor::Red,
    AssetColor::Blue,
    AssetColor::Green,
    AssetColor::Gray,
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
        let count = 1 + self.below(choices.len().min(10));
        (0..count).map(|_| self.pick(choices)).collect()
    }
}

fn asset_id(number: usize) -> AssetId {
    AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
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

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("folder is created");
    for entry in fs::read_dir(from).expect("folder is read") {
        let entry = entry.expect("entry is read");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type is read").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("file is copied");
        }
    }
}

#[derive(Debug, PartialEq)]
struct Observation {
    all: Vec<Asset>,
    trash: Vec<Asset>,
    found: Vec<(String, Vec<Asset>)>,
    counts: Vec<(String, ViewCounts)>,
    smart: Vec<(SmartCollectionId, Vec<AssetId>)>,
    tags: Vec<Tag>,
}

fn filters() -> Vec<(&'static str, AssetFilter)> {
    vec![
        ("none", AssetFilter::default()),
        ("text logo", AssetFilter::text("logo")),
        ("text tag", AssetFilter::text("alpha")),
        ("text note", AssetFilter::text("remarque")),
        (
            "favorites",
            AssetFilter {
                favorites_only: true,
                ..AssetFilter::default()
            },
        ),
        (
            "red",
            AssetFilter {
                colors: vec![AssetColor::Red],
                ..AssetFilter::default()
            },
        ),
        (
            "blue photo",
            AssetFilter {
                colors: vec![AssetColor::Blue],
                ..AssetFilter::text("photo")
            },
        ),
        (
            "custom colour",
            AssetFilter {
                custom_color: Some(Rgb::new(200, 30, 30)),
                ..AssetFilter::default()
            },
        ),
    ]
}

fn observe(library: &Library) -> Observation {
    let mut found = Vec::new();
    let mut counts = Vec::new();
    for (label, filter) in filters() {
        for view in [AssetView::All, AssetView::Trash] {
            found.push((
                format!("{label} in {view:?}"),
                library
                    .find_assets_in(view, &filter)
                    .expect("assets are searched"),
            ));
        }
        counts.push((
            label.to_owned(),
            library
                .view_counts_matching(&filter)
                .expect("counts are computed"),
        ));
    }
    let smart = library
        .smart_collections()
        .expect("smart collections are listed")
        .iter()
        .map(|collection| {
            (
                collection.id,
                library
                    .visible_assets_in(AssetView::Smart(collection.id))
                    .expect("smart collection is listed")
                    .into_iter()
                    .map(|asset| asset.id)
                    .collect(),
            )
        })
        .collect();
    Observation {
        all: library
            .visible_assets_in(AssetView::All)
            .expect("assets are listed"),
        trash: library
            .visible_assets_in(AssetView::Trash)
            .expect("trash is listed"),
        found,
        counts,
        smart,
        tags: library.tags().expect("tags are listed"),
    }
}

struct Fixture {
    _workspace: TempDir,
    library: Library,
    assets: Vec<AssetId>,
    fixtures_imported: usize,
}

impl Fixture {
    fn new() -> Self {
        Self::prepared(|_| {})
    }

    fn prepared(prepare: impl FnOnce(&Path)) -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        let root = library.root().to_path_buf();
        drop(library);
        insert_assets(&root);
        prepare(&root);
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
                "Red things",
                &AssetFilter {
                    colors: vec![AssetColor::Red],
                    ..AssetFilter::default()
                },
            )
            .expect("smart collection");
        library
            .create_smart_collection("Tagged", &AssetFilter::text("alpha"))
            .expect("smart collection");
        Self {
            _workspace: workspace,
            library,
            assets: (1..=ASSETS).map(asset_id).collect(),
            fixtures_imported: 0,
        }
    }

    fn root(&self) -> PathBuf {
        self.library.root().to_path_buf()
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

    fn smart_collections(&self) -> Vec<SmartCollectionId> {
        self.library
            .smart_collections()
            .expect("smart collections are listed")
            .into_iter()
            .map(|collection| collection.id)
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

    fn reopened(&self) -> (TempDir, Library) {
        let copy = tempfile::tempdir().expect("temporary directory");
        let root = copy.path().join("copy.pigoune");
        copy_tree(&self.root(), &root);
        let library = Library::open(&root).expect("a copy of the library opens");
        (copy, library)
    }

    fn assert_coherent(&self, context: &str) {
        let live = observe(&self.library);
        let (_copy, reopened) = self.reopened();
        let fresh = observe(&reopened);
        assert_eq!(live.tags, fresh.tags, "tags differ ({context})");
        assert_eq!(
            live.all, fresh.all,
            "the listing of everything differs ({context})"
        );
        assert_eq!(live.trash, fresh.trash, "the trash differs ({context})");
        assert_eq!(live.found, fresh.found, "a search differs ({context})");
        assert_eq!(live.counts, fresh.counts, "counters differ ({context})");
        assert_eq!(
            live.smart, fresh.smart,
            "smart collections differ ({context})"
        );
    }
}

fn some_colors(dice: &mut Dice) -> Vec<DominantColor> {
    (0..=dice.below(3))
        .map(|_| {
            let family = dice.pick(&FAMILIES);
            DominantColor {
                family,
                average: Rgb::new(
                    u8::try_from(dice.below(256)).expect("a byte"),
                    u8::try_from(dice.below(256)).expect("a byte"),
                    u8::try_from(dice.below(256)).expect("a byte"),
                ),
            }
        })
        .collect()
}

fn blur_document(x: f32) -> AnnotationDocument {
    let blur = Shape::new(
        "blur",
        Geometry::Box(Rect {
            x,
            y: 2.0,
            width: 30.0,
            height: 40.0,
        }),
        Options::defaults(BLUR.options),
    );
    AnnotationDocument::new(None, vec![AnnotationObject::new(1, blur)])
}

fn item_gesture(fixture: &mut Fixture, dice: &mut Dice) -> &'static str {
    let everything = fixture.assets.clone();
    let alive = fixture.alive();
    let trashed = fixture.trashed();
    match dice.below(6) {
        0 => {
            let _ = fixture
                .library
                .apply_asset_command(&AssetCommand::SetFavorite {
                    assets: dice.some_of(&everything),
                    favorite: dice.below(2) == 0,
                });
            "favorites"
        }
        1 => {
            let _ = fixture.library.apply_asset_command(&AssetCommand::Rename {
                asset: dice.pick(&everything),
                name: format!("{} renamed {}", dice.pick(&WORDS), dice.below(100)),
            });
            "rename"
        }
        2 => {
            let _ = fixture.library.apply_asset_command(&AssetCommand::SetText {
                asset: dice.pick(&everything),
                field: TextField::Note,
                value: format!("remarque {}", dice.below(10)),
            });
            "note"
        }
        3 if !alive.is_empty() => {
            let _ = fixture
                .library
                .apply_asset_command(&AssetCommand::SetTrashed {
                    assets: dice.some_of(&alive),
                    trashed: true,
                });
            "trash"
        }
        4 if !trashed.is_empty() => {
            let _ = fixture
                .library
                .apply_asset_command(&AssetCommand::SetTrashed {
                    assets: dice.some_of(&trashed),
                    trashed: false,
                });
            "restore"
        }
        _ => {
            let _ = fixture
                .library
                .apply_asset_command(&AssetCommand::Batch(vec![
                    AssetCommand::SetFavorite {
                        assets: dice.some_of(&everything),
                        favorite: true,
                    },
                    AssetCommand::Rename {
                        asset: asset_id(ASSETS + 900),
                        name: "unknown".to_owned(),
                    },
                ]));
            "failed batch"
        }
    }
}

fn collection_gesture(fixture: &mut Fixture, dice: &mut Dice) -> &'static str {
    let collections = fixture.collections();
    let everything = fixture.assets.clone();
    match dice.below(6) {
        0 if !collections.is_empty() => {
            let _ = fixture
                .library
                .apply_collection_command(&CollectionCommand::AddAssets {
                    collection: dice.pick(&collections),
                    assets: dice.some_of(&everything),
                });
            "collection add"
        }
        1 if !collections.is_empty() => {
            let _ = fixture
                .library
                .apply_collection_command(&CollectionCommand::RemoveAssets {
                    collection: dice.pick(&collections),
                    assets: dice.some_of(&everything),
                });
            "collection remove"
        }
        2 if collections.len() > 1 => {
            let _ = fixture
                .library
                .apply_collection_command(&CollectionCommand::MoveAssets {
                    from: dice.pick(&collections),
                    to: dice.pick(&collections),
                    assets: dice.some_of(&everything),
                });
            "collection move"
        }
        3 if collections.len() > 2 => {
            let _ = fixture
                .library
                .apply_collection_command(&CollectionCommand::Trash {
                    id: dice.pick(&collections),
                });
            "collection trash"
        }
        4 if !collections.is_empty() => {
            let _ = fixture
                .library
                .apply_collection_command(&CollectionCommand::Rename {
                    id: dice.pick(&collections),
                    name: format!("Renamed {}", dice.below(1000)),
                });
            "collection rename"
        }
        _ => {
            let _ = fixture
                .library
                .create_collection(&format!("New {}", dice.below(1000)), None);
            "collection create"
        }
    }
}

fn tag_and_smart_gesture(fixture: &mut Fixture, dice: &mut Dice) -> &'static str {
    let tags = fixture.tags();
    let smart = fixture.smart_collections();
    let everything = fixture.assets.clone();
    match dice.below(7) {
        0 | 1 => {
            let _ = fixture.library.apply_tag_command(&TagCommand::Add {
                assets: dice.some_of(&everything),
                name: dice.pick(&TAG_PATHS).to_owned(),
            });
            "tag add"
        }
        2 if !tags.is_empty() => {
            let _ = fixture.library.apply_tag_command(&TagCommand::Unlink {
                tag: dice.pick(&tags),
                assets: dice.some_of(&everything),
            });
            "tag unlink"
        }
        3 if !tags.is_empty() => {
            let _ = fixture.library.apply_tag_command(&TagCommand::Rename {
                tag: dice.pick(&tags),
                name: format!("Tag{}", dice.below(1000)),
            });
            "tag rename"
        }
        4 if tags.len() > 3 => {
            let _ = fixture.library.apply_tag_command(&TagCommand::Delete {
                tag: dice.pick(&tags),
            });
            "tag delete"
        }
        5 if !smart.is_empty() => {
            let _ =
                fixture
                    .library
                    .apply_smart_collection_command(&SmartCollectionCommand::Delete {
                        id: dice.pick(&smart),
                    });
            "smart delete"
        }
        _ => {
            let _ = fixture.library.create_smart_collection(
                &format!("Smart {}", dice.below(1000)),
                &AssetFilter {
                    colors: vec![dice.pick(&FAMILIES)],
                    ..AssetFilter::text(dice.pick(&WORDS))
                },
            );
            "smart create"
        }
    }
}

fn background_and_import_gesture(fixture: &mut Fixture, dice: &mut Dice) -> &'static str {
    let everything = fixture.assets.clone();
    match dice.below(6) {
        0 => {
            let _ = fixture.library.record_colors_batch(
                &dice
                    .some_of(&everything)
                    .into_iter()
                    .map(|asset| (asset, some_colors(dice)))
                    .collect::<Vec<_>>(),
            );
            "background colours"
        }
        1 => {
            let _ = fixture.library.record_visual_analysis_batch(
                &dice
                    .some_of(&everything)
                    .into_iter()
                    .map(|asset| (asset, None))
                    .collect::<Vec<_>>(),
            );
            "background visual analysis"
        }
        2 => {
            let _ = fixture.library.record_visual_details_batch(
                &dice
                    .some_of(&everything)
                    .into_iter()
                    .map(|asset| (asset, None))
                    .collect::<Vec<_>>(),
            );
            "background visual details"
        }
        3 => {
            let shift = f32::from(u8::try_from(dice.below(20)).expect("a small number"));
            let _ = fixture
                .library
                .save_annotations(dice.pick(&everything), &blur_document(shift));
            "annotation"
        }
        4 if fixture.fixtures_imported < FIXTURES.len() => {
            let name = FIXTURES[fixture.fixtures_imported];
            fixture.fixtures_imported += 1;
            let _ = fixture.library.import_file(&fixture_path(name), None);
            "import"
        }
        _ => {
            let good = fixture_path(FIXTURES[dice.below(FIXTURES.len())]);
            let _ = fixture.library.import_paths(
                &[fixture_path("missing-file.png"), good],
                None,
                |_| true,
                |_| ImportControl::Continue,
            );
            "partial import"
        }
    }
}

fn gesture(fixture: &mut Fixture, dice: &mut Dice) -> &'static str {
    match dice.below(4) {
        0 => item_gesture(fixture, dice),
        1 => collection_gesture(fixture, dice),
        2 => tag_and_smart_gesture(fixture, dice),
        _ => background_and_import_gesture(fixture, dice),
    }
}

#[test]
fn cached_reads_match_a_reopened_library_after_random_gestures_undo_and_redo() {
    for seed in 1..=SEQUENCES {
        let mut fixture = Fixture::new();
        let mut dice = Dice(seed);
        fixture.assert_coherent(&format!("seed {seed}, start"));
        for step in 0..GESTURES {
            let _ = observe(&fixture.library);
            let kind = gesture(&mut fixture, &mut dice);
            fixture.assert_coherent(&format!("seed {seed}, gesture {step} ({kind})"));
            if step % 6 == 5 {
                for undone in 0..3 {
                    let _ = observe(&fixture.library);
                    let _ = fixture.library.undo();
                    fixture.assert_coherent(&format!("seed {seed}, undo {undone} after {step}"));
                }
                for redone in 0..2 {
                    let _ = observe(&fixture.library);
                    let _ = fixture.library.redo();
                    fixture.assert_coherent(&format!("seed {seed}, redo {redone} after {step}"));
                }
            }
        }
    }
}

#[test]
fn every_kind_of_write_changes_what_the_next_cached_read_shows() {
    let mut fixture = Fixture::new();
    let first = fixture.assets[0];
    let second = fixture.assets[1];
    let _ = observe(&fixture.library);
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![first],
            favorite: true,
        })
        .expect("favorite is set");
    let after_favorite = observe(&fixture.library);
    assert!(
        after_favorite
            .all
            .iter()
            .any(|asset| asset.id == first && asset.is_favorite)
    );
    fixture.assert_coherent("a favorite");

    fixture
        .library
        .record_colors_batch(&[(
            second,
            vec![DominantColor {
                family: AssetColor::Red,
                average: Rgb::new(210, 20, 20),
            }],
        )])
        .expect("colours are recorded");
    let after_colours = observe(&fixture.library);
    let red = after_colours
        .found
        .iter()
        .find(|(label, _)| label == "red in All")
        .expect("the red search exists");
    assert!(
        red.1.iter().any(|asset| asset.id == second),
        "the red filter sees the new colour"
    );
    assert!(
        after_colours
            .smart
            .iter()
            .any(|(_, members)| members.contains(&second)),
        "a smart collection on colour sees it"
    );
    fixture.assert_coherent("new colours");

    fixture
        .library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![first],
            name: "Alpha/Beta".to_owned(),
        })
        .expect("a tag is added");
    let after_tag = observe(&fixture.library);
    let tagged = after_tag
        .found
        .iter()
        .find(|(label, _)| label == "text tag in All")
        .expect("the tag search exists");
    assert!(
        tagged.1.iter().any(|asset| asset.id == first),
        "the tag name is searchable at once"
    );
    fixture.assert_coherent("a tag");

    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![first],
            trashed: true,
        })
        .expect("an asset is trashed");
    let after_trash = observe(&fixture.library);
    assert!(after_trash.trash.iter().any(|asset| asset.id == first));
    assert!(!after_trash.all.iter().any(|asset| asset.id == first));
    fixture.assert_coherent("the trash");
}

#[test]
fn a_failed_command_and_a_half_finished_background_write_leave_the_caches_coherent() {
    let mut fixture = Fixture::new();
    let first = fixture.assets[0];
    let _ = observe(&fixture.library);
    let failed = fixture
        .library
        .apply_asset_command(&AssetCommand::Batch(vec![
            AssetCommand::SetFavorite {
                assets: vec![first],
                favorite: true,
            },
            AssetCommand::Rename {
                asset: asset_id(ASSETS + 900),
                name: "unknown".to_owned(),
            },
        ]));
    assert!(failed.is_err(), "the second command cannot succeed");
    assert!(
        !observe(&fixture.library)
            .all
            .iter()
            .any(|asset| asset.id == first && asset.is_favorite),
        "nothing of a failed command is shown"
    );
    fixture.assert_coherent("a failed batch");

    let details_path = fixture
        .root()
        .join(pigoune_core::CACHE_DIR_NAME)
        .join("visual-details.db");
    let outside = Connection::open(&details_path).expect("the details file opens");
    let definition: String = outside
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'asset_details'",
            [],
            |row| row.get(0),
        )
        .expect("the details table exists");
    let _ = observe(&fixture.library);
    outside
        .execute_batch("DROP TABLE asset_details")
        .expect("the details table is dropped");
    let half_done = fixture
        .library
        .record_visual_analysis_batch(&[(first, None), (fixture.assets[1], None)]);
    outside
        .execute_batch(&definition)
        .expect("the details table is restored");
    assert!(
        half_done.is_err(),
        "the second step of the analysis write must fail"
    );
    fixture.assert_coherent("a half finished analysis write");

    let _ = observe(&fixture.library);
    let partial = fixture.library.import_paths(
        &[
            fixture_path("missing-file.png"),
            fixture_path("red-dot.png"),
        ],
        None,
        |_| true,
        |_| ImportControl::Continue,
    );
    assert!(partial.is_ok());
    fixture.assert_coherent("a partial import");
}

#[test]
fn visual_analysis_writes_in_every_order_keep_the_reads_equal_to_a_reopened_library() {
    let mut fixture = Fixture::new();
    let batch: Vec<AssetId> = fixture.assets.iter().take(12).copied().collect();
    let analyses: Vec<(AssetId, Option<pigoune_core::VisualAnalysis>)> =
        batch.iter().map(|asset| (*asset, None)).collect();
    let details: Vec<(AssetId, Option<pigoune_core::VisualDetail>)> =
        batch.iter().map(|asset| (*asset, None)).collect();
    let colours: Vec<(AssetId, Vec<DominantColor>)> = batch
        .iter()
        .map(|asset| {
            (
                *asset,
                vec![DominantColor {
                    family: AssetColor::Red,
                    average: Rgb::new(210, 30, 30),
                }],
            )
        })
        .collect();

    let _ = observe(&fixture.library);
    fixture
        .library
        .record_visual_analysis_batch(&analyses)
        .expect("the hash is recorded");
    fixture.assert_coherent("the hash only");

    let _ = observe(&fixture.library);
    fixture
        .library
        .record_visual_details_batch(&details)
        .expect("the details are recorded");
    fixture.assert_coherent("the details only");

    let _ = observe(&fixture.library);
    fixture
        .library
        .record_visual_analysis_batch(&analyses)
        .expect("the hash is recorded");
    fixture
        .library
        .record_visual_details_batch(&details)
        .expect("the details are recorded");
    fixture.assert_coherent("the hash then the details");

    let _ = observe(&fixture.library);
    fixture
        .library
        .record_visual_analysis_batch(&analyses)
        .expect("the hash is recorded");
    fixture
        .library
        .record_colors_batch(&colours)
        .expect("colours are recorded");
    fixture.assert_coherent("the hash then the colours");

    let _ = observe(&fixture.library);
    fixture
        .library
        .record_colors_batch(&colours[..3])
        .expect("colours are recorded");
    fixture
        .library
        .record_visual_details_batch(&details)
        .expect("the details are recorded");
    fixture.assert_coherent("the colours then the details");

    let _ = observe(&fixture.library);
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: batch.clone(),
            favorite: true,
        })
        .expect("favorites are set");
    fixture
        .library
        .record_visual_analysis_batch(&analyses)
        .expect("the hash is recorded");
    fixture.assert_coherent("a favorite then the hash with a stale cache");

    let _ = observe(&fixture.library);
    fixture
        .library
        .record_visual_analysis_batch(&analyses)
        .expect("the hash is recorded");
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: batch,
            trashed: true,
        })
        .expect("assets are trashed");
    fixture.assert_coherent("the hash then the trash");
}

fn membership_steps(fixture: &Fixture) -> Vec<(&'static str, CollectionCommand)> {
    let everything = fixture.assets.clone();
    let collections = fixture.collections();
    let [first, second, nested, deepest] = collections[..4] else {
        panic!("four collections");
    };
    let mut with_duplicates = everything[..20].to_vec();
    with_duplicates.extend_from_slice(&everything[10..30]);
    vec![
        (
            "mass add to a top collection",
            CollectionCommand::AddAssets {
                collection: first,
                assets: everything.clone(),
            },
        ),
        (
            "add again, everything already there",
            CollectionCommand::AddAssets {
                collection: first,
                assets: everything.clone(),
            },
        ),
        (
            "empty selection",
            CollectionCommand::AddAssets {
                collection: second,
                assets: Vec::new(),
            },
        ),
        (
            "duplicates in the selection",
            CollectionCommand::AddAssets {
                collection: nested,
                assets: with_duplicates.clone(),
            },
        ),
        (
            "mass move into a nested collection",
            CollectionCommand::MoveAssets {
                from: first,
                to: deepest,
                assets: everything.clone(),
            },
        ),
        (
            "move with duplicates",
            CollectionCommand::MoveAssets {
                from: deepest,
                to: second,
                assets: with_duplicates.clone(),
            },
        ),
        (
            "mass removal",
            CollectionCommand::RemoveAssets {
                collection: second,
                assets: everything.clone(),
            },
        ),
        (
            "removal of assets that are not there",
            CollectionCommand::RemoveAssets {
                collection: second,
                assets: everything[..5].to_vec(),
            },
        ),
    ]
}

fn membership_failure_steps(fixture: &Fixture) -> Vec<(&'static str, CollectionCommand)> {
    let everything = fixture.assets.clone();
    let collections = fixture.collections();
    let [first, second, nested, _] = collections[..4] else {
        panic!("four collections");
    };
    let unknown = asset_id(ASSETS + 700);
    let mut with_duplicates = everything[..20].to_vec();
    with_duplicates.extend_from_slice(&everything[10..30]);
    vec![
        (
            "failure: an unknown asset in the middle",
            CollectionCommand::AddAssets {
                collection: second,
                assets: vec![everything[0], unknown, everything[1]],
            },
        ),
        (
            "failure: a batch whose second command fails",
            CollectionCommand::Batch(vec![
                CollectionCommand::AddAssets {
                    collection: first,
                    assets: everything[..10].to_vec(),
                },
                CollectionCommand::RemoveAssets {
                    collection: first,
                    assets: vec![unknown],
                },
                CollectionCommand::AddAssets {
                    collection: first,
                    assets: vec![unknown],
                },
            ]),
        ),
        (
            "a batch of memberships",
            CollectionCommand::Batch(vec![
                CollectionCommand::AddAssets {
                    collection: nested,
                    assets: everything[..15].to_vec(),
                },
                CollectionCommand::MoveAssets {
                    from: nested,
                    to: first,
                    assets: everything[5..12].to_vec(),
                },
            ]),
        ),
    ]
}

#[test]
fn membership_commands_at_every_scale_stay_equal_to_a_reopened_library() {
    let mut fixture = Fixture::new();
    let mut steps = membership_steps(&fixture);
    steps.extend(membership_failure_steps(&fixture));
    for (label, command) in &steps {
        let _ = observe(&fixture.library);
        let _ = fixture.library.apply_collection_command(command);
        fixture.assert_coherent(label);
    }
    for round in 0..6 {
        let _ = observe(&fixture.library);
        let _ = fixture.library.undo();
        fixture.assert_coherent(&format!("undo {round}"));
    }
    for round in 0..6 {
        let _ = observe(&fixture.library);
        let _ = fixture.library.redo();
        fixture.assert_coherent(&format!("redo {round}"));
    }
}

type Step = Box<dyn Fn(&mut Fixture)>;

fn interleaved_steps(fixture: &Fixture) -> Vec<(&'static str, Step)> {
    let everything = fixture.assets.clone();
    let collections = fixture.collections();
    let [first, second, nested, _] = collections[..4] else {
        panic!("four collections");
    };
    let membership = |collection: CollectionId, from: usize, to: usize| -> Step {
        let assets = everything[from..to].to_vec();
        Box::new(move |fixture: &mut Fixture| {
            let _ = fixture
                .library
                .apply_collection_command(&CollectionCommand::AddAssets {
                    collection,
                    assets: assets.clone(),
                });
        })
    };
    vec![
        ("add to first", membership(first, 0, 40)),
        (
            "favorite",
            Box::new(|fixture| {
                let assets = fixture.assets[..8].to_vec();
                let _ = fixture
                    .library
                    .apply_asset_command(&AssetCommand::SetFavorite {
                        assets,
                        favorite: true,
                    });
            }),
        ),
        ("add to second right after", membership(second, 10, 50)),
        (
            "trash assets",
            Box::new(|fixture| {
                let assets = fixture.assets[..6].to_vec();
                let _ = fixture
                    .library
                    .apply_asset_command(&AssetCommand::SetTrashed {
                        assets,
                        trashed: true,
                    });
            }),
        ),
        ("add to nested", membership(nested, 0, 30)),
        (
            "restore assets",
            Box::new(|fixture| {
                let assets = fixture.assets[..6].to_vec();
                let _ = fixture
                    .library
                    .apply_asset_command(&AssetCommand::SetTrashed {
                        assets,
                        trashed: false,
                    });
            }),
        ),
    ]
}

fn interleaved_late_steps(fixture: &Fixture) -> Vec<(&'static str, Step)> {
    let everything = fixture.assets.clone();
    let collections = fixture.collections();
    let [first, second, nested, _] = collections[..4] else {
        panic!("four collections");
    };
    let tags = fixture.library.tags().expect("tags are listed");
    let named = |name: &str| tags.iter().find(|tag| tag.name == name).expect("tag").id;
    let (alpha, beta, epsilon) = (named("Alpha"), named("Beta"), named("Epsilon"));
    let membership = |collection: CollectionId, from: usize, to: usize| -> Step {
        let assets = everything[from..to].to_vec();
        Box::new(move |fixture: &mut Fixture| {
            let _ = fixture
                .library
                .apply_collection_command(&CollectionCommand::AddAssets {
                    collection,
                    assets: assets.clone(),
                });
        })
    };
    vec![
        (
            "merge tags",
            Box::new(move |fixture| {
                let _ = fixture.library.apply_tag_command(&TagCommand::Merge {
                    from: epsilon,
                    into: beta,
                });
            }),
        ),
        ("add after the merge", membership(first, 40, 60)),
        (
            "dissolve a tag",
            Box::new(move |fixture| {
                let _ = fixture
                    .library
                    .apply_tag_command(&TagCommand::Dissolve { tag: alpha });
            }),
        ),
        (
            "remove from second",
            Box::new(move |fixture| {
                let _ =
                    fixture
                        .library
                        .apply_collection_command(&CollectionCommand::RemoveAssets {
                            collection: second,
                            assets: fixture.assets[..25].to_vec(),
                        });
            }),
        ),
        (
            "delete a tag",
            Box::new(move |fixture| {
                let _ = fixture
                    .library
                    .apply_tag_command(&TagCommand::Delete { tag: beta });
            }),
        ),
        (
            "trash a collection with assets",
            Box::new(move |fixture| {
                let _ = fixture
                    .library
                    .apply_collection_command(&CollectionCommand::Trash { id: nested });
            }),
        ),
        (
            "collection command trashing assets",
            Box::new(move |fixture| {
                let assets = fixture.assets[30..36].to_vec();
                let _ = fixture
                    .library
                    .apply_collection_command(&CollectionCommand::SetTrashed {
                        collections: Vec::new(),
                        assets,
                        trashed: true,
                    });
            }),
        ),
        (
            "a batch of a membership and a trashing",
            Box::new(move |fixture| {
                let assets = fixture.assets[40..46].to_vec();
                let _ = fixture
                    .library
                    .apply_collection_command(&CollectionCommand::Batch(vec![
                        CollectionCommand::AddAssets {
                            collection: first,
                            assets: assets.clone(),
                        },
                        CollectionCommand::SetTrashed {
                            collections: Vec::new(),
                            assets,
                            trashed: true,
                        },
                    ]));
            }),
        ),
        ("add once more", membership(first, 0, 70)),
    ]
}

#[test]
fn neutral_and_non_neutral_writes_interleaved_never_leave_a_stale_read() {
    let mut fixture = Fixture::new();
    let everything = fixture.assets.clone();
    let _ = fixture.library.apply_tag_command(&TagCommand::Add {
        assets: everything[..30].to_vec(),
        name: "Alpha/Beta".to_owned(),
    });
    let _ = fixture.library.apply_tag_command(&TagCommand::Add {
        assets: everything[20..50].to_vec(),
        name: "Epsilon".to_owned(),
    });
    let mut steps = interleaved_steps(&fixture);
    steps.extend(interleaved_late_steps(&fixture));
    for (label, step) in &steps {
        let _ = observe(&fixture.library);
        step(&mut fixture);
        fixture.assert_coherent(label);
    }
    for round in 0..steps.len() {
        let _ = observe(&fixture.library);
        let _ = fixture.library.undo();
        fixture.assert_coherent(&format!("undo {round}"));
    }
    for round in 0..steps.len() {
        let _ = observe(&fixture.library);
        let _ = fixture.library.redo();
        fixture.assert_coherent(&format!("redo {round}"));
    }
}

struct Sources {
    folder: TempDir,
    made: usize,
}

impl Sources {
    fn new() -> Self {
        Self {
            folder: tempfile::tempdir().expect("temporary directory"),
            made: 0,
        }
    }

    fn file_in(&mut self, folder: &Path) -> PathBuf {
        self.made += 1;
        fs::create_dir_all(folder).expect("folder is created");
        let mut bytes = fs::read(fixture_path("red-dot.png")).expect("sample is read");
        bytes.extend_from_slice(format!("#{}", self.made).as_bytes());
        let path = folder.join(format!("import-{}.png", self.made));
        fs::write(&path, bytes).expect("source is written");
        path
    }

    fn files(&mut self, count: usize) -> Vec<PathBuf> {
        let folder = self.folder.path().to_path_buf();
        (0..count).map(|_| self.file_in(&folder)).collect()
    }
}

fn import_with_a_warm_cache(
    fixture: &mut Fixture,
    paths: &[PathBuf],
    target: Option<CollectionId>,
) -> pigoune_core::ImportSummary {
    let _ = observe(&fixture.library);
    fixture
        .library
        .import_paths(paths, target, |_| true, |_| ImportControl::Continue)
        .expect("the import runs")
}

fn listing_is_in_the_database_order(fixture: &Fixture) {
    let shown = fixture
        .library
        .visible_assets_in(AssetView::All)
        .expect("assets are listed");
    for pair in shown.windows(2) {
        assert!(
            (pair[0].added_at_unix_ms, pair[0].id) > (pair[1].added_at_unix_ms, pair[1].id),
            "the listing is not newest first"
        );
    }
}

fn new_and_known_imports(fixture: &mut Fixture, sources: &mut Sources, label: &str) {
    let one = sources.files(1);
    import_with_a_warm_cache(fixture, &one, None);
    fixture.assert_coherent(&format!("{label}: one new item"));
    listing_is_in_the_database_order(fixture);

    let several = sources.files(3);
    import_with_a_warm_cache(fixture, &several, None);
    fixture.assert_coherent(&format!("{label}: several new items"));
    listing_is_in_the_database_order(fixture);

    let summary = import_with_a_warm_cache(
        fixture,
        &[one[0].clone(), sources.folder.path().join("missing.png")],
        None,
    );
    assert_eq!(summary.already_present, 1);
    fixture.assert_coherent(&format!("{label}: a duplicate and a missing file"));

    let collection = fixture.collections()[0];
    let into = sources.files(2);
    import_with_a_warm_cache(fixture, &into, Some(collection));
    fixture.assert_coherent(&format!("{label}: into a collection"));
    let again = import_with_a_warm_cache(fixture, &into, Some(collection));
    assert_eq!(again.already_present, 2);
    fixture.assert_coherent(&format!("{label}: the same items into the same collection"));
    let other = fixture.collections()[1];
    let moved = import_with_a_warm_cache(fixture, &into, Some(other));
    assert_eq!(moved.added_to_collection, 2);
    fixture.assert_coherent(&format!("{label}: known items added to another collection"));

    let trashed = import_with_a_warm_cache(fixture, &sources.files(1), None).imported[0];
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![trashed],
            trashed: true,
        })
        .expect("trashed");
    fixture.assert_coherent(&format!("{label}: an item in the trash"));
    let restored = import_with_a_warm_cache(
        fixture,
        &[sources
            .folder
            .path()
            .join(format!("import-{}.png", sources.made))],
        None,
    );
    assert_eq!(restored.restored_from_trash, 1);
    fixture.assert_coherent(&format!("{label}: restored from the trash"));
}

fn folder_batched_cancelled_and_failed_imports(
    fixture: &mut Fixture,
    sources: &mut Sources,
    label: &str,
) {
    let nested = sources.folder.path().join("album");
    sources.file_in(&nested);
    sources.file_in(&nested.join("deeper"));
    import_with_a_warm_cache(fixture, std::slice::from_ref(&nested), None);
    fixture.assert_coherent(&format!("{label}: a folder creating collections"));

    let slow = sources.files(4);
    let batches = fixture
        .library
        .import_paths(
            &slow,
            None,
            |_| true,
            |progress| {
                if progress.done > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(450));
                }
                ImportControl::Continue
            },
        )
        .expect("the import runs");
    assert_eq!(batches.imported.len(), 4);
    fixture.assert_coherent(&format!("{label}: several batches"));
    listing_is_in_the_database_order(fixture);

    let cancelled = sources.files(4);
    let _ = observe(&fixture.library);
    let summary = fixture
        .library
        .import_paths(
            &cancelled,
            None,
            |_| true,
            |progress| {
                if progress.done == 2 {
                    ImportControl::Cancel
                } else {
                    ImportControl::Continue
                }
            },
        )
        .expect("the import runs");
    assert_eq!(summary.imported.len(), 2);
    fixture.assert_coherent(&format!("{label}: cancelled halfway"));

    let broken = sources.files(3);
    let files = fixture.root().join("files");
    let away = fixture.root().join("files-away");
    let _ = observe(&fixture.library);
    let summary = fixture
        .library
        .import_paths(
            &broken,
            None,
            |_| true,
            |progress| {
                if progress.done == 1 {
                    fs::rename(&files, &away).expect("the folder moves");
                    std::os::unix::fs::symlink(&away, &files).expect("a link takes its place");
                }
                ImportControl::Continue
            },
        )
        .expect("the import runs");
    assert!(summary.imported.is_empty());
    fs::remove_file(&files).expect("the link goes");
    fs::rename(&away, &files).expect("the folder returns");
    fixture.assert_coherent(&format!("{label}: interrupted after some work"));
}

fn imports_of_every_shape(mut fixture: Fixture, label: &str) {
    let mut sources = Sources::new();
    new_and_known_imports(&mut fixture, &mut sources, label);
    folder_batched_cancelled_and_failed_imports(&mut fixture, &mut sources, label);
}

fn later_changes_on_imported_items(mut fixture: Fixture, label: &str) {
    let mut sources = Sources::new();
    let files = sources.files(3);
    let imported = import_with_a_warm_cache(&mut fixture, &files, None).imported;
    let first = imported[0];
    let second = imported[1];

    for (step, gesture) in [
        "favorite",
        "rename",
        "tag",
        "colours",
        "visual analysis",
        "visual details",
        "annotation",
        "trash",
    ]
    .iter()
    .enumerate()
    {
        let _ = observe(&fixture.library);
        match step {
            0 => fixture
                .library
                .apply_asset_command(&AssetCommand::SetFavorite {
                    assets: vec![first],
                    favorite: true,
                })
                .map(|_| ())
                .expect("favorite"),
            1 => fixture
                .library
                .apply_asset_command(&AssetCommand::Rename {
                    asset: second,
                    name: "logo renamed".to_owned(),
                })
                .map(|_| ())
                .expect("rename"),
            2 => fixture
                .library
                .apply_tag_command(&TagCommand::Add {
                    assets: vec![first, second],
                    name: "Alpha/Beta".to_owned(),
                })
                .map(|_| ())
                .expect("tag"),
            3 => fixture
                .library
                .record_colors_batch(&[(first, some_colors(&mut Dice(7)))])
                .expect("colours"),
            4 => fixture
                .library
                .record_visual_analysis_batch(&[(first, None), (second, None)])
                .expect("analysis"),
            5 => fixture
                .library
                .record_visual_details_batch(&[(second, None)])
                .expect("details"),
            6 => fixture
                .library
                .save_annotations(first, &blur_document(3.0))
                .expect("annotation"),
            _ => fixture
                .library
                .apply_asset_command(&AssetCommand::SetTrashed {
                    assets: vec![second],
                    trashed: true,
                })
                .map(|_| ())
                .expect("trash"),
        }
        fixture.assert_coherent(&format!("{label}: {gesture} after an import"));
    }
}

#[test]
fn imports_of_every_shape_keep_the_cached_reads_equal_to_a_reopened_library() {
    imports_of_every_shape(Fixture::new(), "plain");
}

#[test]
fn imports_among_newer_and_older_items_keep_the_cached_reads_equal_to_a_reopened_library() {
    let mixed = Fixture::prepared(|root| {
        let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
        connection
            .execute(
                "UPDATE assets SET added_at_unix_ms = added_at_unix_ms + 99999999990000
                 WHERE CAST(substr(content_hash, 6) AS INTEGER) % 2 = 0",
                [],
            )
            .expect("some items are dated in the future");
    });
    imports_of_every_shape(mixed, "mixed dates");
}

#[test]
fn the_deferred_writes_and_later_changes_after_an_import_stay_equal_to_a_reopened_library() {
    later_changes_on_imported_items(Fixture::new(), "plain");
}

#[test]
fn an_import_into_a_cache_that_was_never_filled_is_equal_to_a_reopened_library() {
    let mut fixture = Fixture::new();
    let mut sources = Sources::new();
    let files = sources.files(2);
    fixture
        .library
        .import_paths(&files, None, |_| true, |_| ImportControl::Continue)
        .expect("the import runs");
    fixture.assert_coherent("an import with no cache");
}

fn tinted(family: AssetColor, level: u8) -> Vec<DominantColor> {
    vec![DominantColor {
        family,
        average: Rgb::new(level, 30, 200 - level.min(190)),
    }]
}

fn colours_stored_in(root: &Path) -> (usize, usize) {
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    let filled: i64 = connection
        .query_row(
            "SELECT count(*) FROM assets WHERE colors IS NOT NULL AND colors != ''",
            [],
            |row| row.get(0),
        )
        .expect("filled colours are counted");
    let rows: i64 = connection
        .query_row("SELECT count(*) FROM assets", [], |row| row.get(0))
        .expect("rows are counted");
    (
        usize::try_from(filled).expect("a count"),
        usize::try_from(rows).expect("a count"),
    )
}

fn colours_seen_by_a_reopened_library(fixture: &Fixture) -> usize {
    let (_copy, reopened) = fixture.reopened();
    [AssetView::All, AssetView::Trash]
        .into_iter()
        .map(|view| {
            reopened
                .visible_assets_in(view)
                .expect("assets are listed")
                .iter()
                .filter(|asset| !asset.colors.is_empty())
                .count()
        })
        .sum()
}

#[test]
fn colour_writes_keep_the_warm_caches_and_the_reads_equal_to_a_reopened_library() {
    let mut fixture = Fixture::new();
    let mut sources = Sources::new();
    let alive = fixture.alive();
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: alive[..6].to_vec(),
            trashed: true,
        })
        .expect("some items are trashed");
    let alive = fixture.alive();
    let trashed = fixture.trashed();
    let missing = asset_id(ASSETS + 900);

    let _ = observe(&fixture.library);
    fixture
        .library
        .record_colors_batch(&[(alive[0], tinted(AssetColor::Red, 210))])
        .expect("one colour is recorded");
    fixture.assert_coherent("one active item");

    let _ = observe(&fixture.library);
    fixture
        .library
        .record_colors_batch(&[
            (alive[1], tinted(AssetColor::Red, 200)),
            (trashed[0], tinted(AssetColor::Red, 205)),
            (alive[2], tinted(AssetColor::Blue, 20)),
            (trashed[1], tinted(AssetColor::Blue, 25)),
        ])
        .expect("several colours are recorded");
    fixture.assert_coherent("active and trashed items");

    let _ = observe(&fixture.library);
    fixture
        .library
        .record_colors_batch(&[
            (alive[3], tinted(AssetColor::Red, 220)),
            (alive[3], tinted(AssetColor::Green, 90)),
            (missing, tinted(AssetColor::Red, 100)),
            (alive[0], Vec::new()),
            (trashed[2], Vec::new()),
        ])
        .expect("a mixed batch is recorded");
    fixture.assert_coherent("repeated, absent and emptied");

    let _ = observe(&fixture.library);
    for round in 0..3_u8 {
        fixture
            .library
            .record_colors_batch(&[
                (
                    alive[4 + usize::from(round)],
                    tinted(AssetColor::Red, 200 + round),
                ),
                (alive[10], tinted(AssetColor::Gray, 100 + round)),
            ])
            .expect("successive batches are recorded");
    }
    fixture.assert_coherent("successive batches");

    let _ = fixture.library.visible_assets_in(AssetView::All);
    fixture
        .library
        .record_colors_batch(&[
            (trashed[3], tinted(AssetColor::Red, 215)),
            (alive[5], tinted(AssetColor::Red, 216)),
        ])
        .expect("only the listing is warm");
    fixture.assert_coherent("only the listing of everything is warm");

    colour_writes_around_imports_and_ordinary_writes(&mut fixture, &mut sources, &alive);
}

fn colour_writes_around_imports_and_ordinary_writes(
    fixture: &mut Fixture,
    sources: &mut Sources,
    alive: &[AssetId],
) {
    let files = sources.files(2);
    let summary = import_with_a_warm_cache(fixture, &files, None);
    let imported: Vec<_> = summary
        .imported
        .iter()
        .map(|id| (*id, tinted(AssetColor::Red, 230)))
        .collect();
    fixture
        .library
        .record_colors_batch(&imported)
        .expect("imported items are coloured");
    fixture.assert_coherent("colours of imported items");
    let more = sources.files(1);
    import_with_a_warm_cache(fixture, &more, None);
    fixture.assert_coherent("an import after the colours");

    let _ = observe(&fixture.library);
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![alive[7]],
            favorite: true,
        })
        .expect("a favorite is set");
    fixture
        .library
        .record_colors_batch(&[(alive[7], tinted(AssetColor::Red, 212))])
        .expect("colours after an ordinary write");
    fixture.assert_coherent("an ordinary write then a colour write");
    fixture
        .library
        .record_colors_batch(&[(alive[8], tinted(AssetColor::Red, 213))])
        .expect("a colour write");
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![alive[8]],
            favorite: true,
        })
        .expect("a favorite is set");
    fixture.assert_coherent("a colour write then an ordinary write");

    let (filled, rows) = colours_stored_in(&fixture.root());
    assert_eq!(filled, colours_seen_by_a_reopened_library(fixture));
    assert_eq!(rows, fixture.alive().len() + fixture.trashed().len());
    assert!(rows <= ASSETS + 3);
}
