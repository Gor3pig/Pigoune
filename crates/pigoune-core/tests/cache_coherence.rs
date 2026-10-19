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

    fn reopened(&self) -> Library {
        let copy = tempfile::tempdir().expect("temporary directory");
        let root = copy.path().join("copy.pigoune");
        copy_tree(&self.root(), &root);
        let library = Library::open(&root).expect("a copy of the library opens");
        std::mem::forget(copy);
        library
    }

    fn assert_coherent(&self, context: &str) {
        let live = observe(&self.library);
        let fresh = observe(&self.reopened());
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
