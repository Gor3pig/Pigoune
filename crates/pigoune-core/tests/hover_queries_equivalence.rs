use std::collections::BTreeMap;
use std::path::Path;

use pigoune_core::{
    AssetCommand, AssetFilter, AssetId, AssetView, CollectionCommand, CollectionId,
    DATABASE_FILE_NAME, Library, TagCommand, TagId,
};
use rusqlite::{Connection, params};
use tempfile::TempDir;

const ASSETS: usize = 90;
const SEQUENCES: u64 = 10;
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

fn selections(fixture: &Fixture, dice: &mut Dice) -> Vec<Vec<AssetId>> {
    let alive = fixture.alive();
    let trashed = fixture.trashed();
    let mut chosen = vec![
        Vec::new(),
        vec![fixture.assets[0]],
        fixture.assets.clone(),
        alive,
        trashed,
        vec![asset_id(ASSETS + 500), fixture.assets[1]],
    ];
    for _ in 0..4 {
        chosen.push(dice.some_of(&fixture.assets));
    }
    let mut repeated = dice.some_of(&fixture.assets);
    repeated.extend(repeated.clone());
    chosen.push(repeated);
    chosen
}

fn reference_removals(
    library: &Library,
    assets: &[AssetId],
    kept: Option<CollectionId>,
) -> Vec<CollectionCommand> {
    let mut by_collection: BTreeMap<CollectionId, Vec<AssetId>> = BTreeMap::new();
    for asset in assets {
        for collection in library
            .collections_of(*asset)
            .expect("collections are read")
        {
            if Some(collection) != kept {
                by_collection.entry(collection).or_default().push(*asset);
            }
        }
    }
    by_collection
        .into_iter()
        .map(|(collection, assets)| CollectionCommand::RemoveAssets { collection, assets })
        .collect()
}

fn reference_unclassify(library: &Library, assets: &[AssetId]) -> Option<CollectionCommand> {
    let removals = reference_removals(library, assets, None);
    if removals.is_empty() {
        return None;
    }
    Some(CollectionCommand::Batch(removals))
}

fn reference_move(
    library: &Library,
    assets: &[AssetId],
    target: CollectionId,
) -> Option<CollectionCommand> {
    let mut commands = reference_removals(library, assets, Some(target));
    let mut newcomers = Vec::new();
    for asset in assets {
        if !library
            .collections_of(*asset)
            .expect("collections are read")
            .contains(&target)
        {
            newcomers.push(*asset);
        }
    }
    if commands.is_empty() && newcomers.is_empty() {
        return None;
    }
    commands.push(CollectionCommand::AddAssets {
        collection: target,
        assets: newcomers,
    });
    Some(CollectionCommand::Batch(commands))
}

fn check_hover_queries(fixture: &Fixture, dice: &mut Dice, context: &str) {
    for selection in selections(fixture, dice) {
        assert_eq!(
            fixture
                .library
                .unclassify_command(&selection)
                .expect("command is prepared"),
            reference_unclassify(&fixture.library, &selection),
            "unclassify differs ({context})"
        );
        assert_eq!(
            fixture
                .library
                .all_are_favorite(&selection)
                .expect("favorites are read"),
            selection.iter().all(|asset| fixture
                .library
                .asset(*asset)
                .is_ok_and(|found| found.is_some_and(|found| found.is_favorite))),
            "favorites differ ({context})"
        );
        for tag in fixture.tags() {
            assert_eq!(
                fixture
                    .library
                    .all_have_tag(&selection, tag)
                    .expect("tags are read"),
                selection.iter().all(|asset| fixture
                    .library
                    .tags_of(*asset)
                    .is_ok_and(|tags| tags.iter().any(|carried| carried.id == tag))),
                "tagged differs ({context})"
            );
        }
        for target in fixture.collections() {
            assert_eq!(
                fixture
                    .library
                    .all_in_collection(&selection, target)
                    .expect("collections are read"),
                selection.iter().all(|asset| fixture
                    .library
                    .collections_of(*asset)
                    .is_ok_and(|held| held.contains(&target))),
                "membership differs ({context})"
            );
            assert_eq!(
                fixture
                    .library
                    .move_to_collection_command(&selection, target)
                    .expect("command is prepared"),
                reference_move(&fixture.library, &selection, target),
                "move differs ({context})"
            );
        }
    }
}

#[test]
fn hover_commands_match_the_per_asset_reference_after_random_gestures_undo_and_redo() {
    for seed in 1..=SEQUENCES {
        let mut fixture = Fixture::new();
        let mut dice = Dice(seed);
        check_hover_queries(&fixture, &mut dice, &format!("seed {seed}, start"));
        for step in 0..GESTURES {
            gesture(&mut fixture, &mut dice);
            check_hover_queries(&fixture, &mut dice, &format!("seed {seed}, gesture {step}"));
            if step % 5 == 4 {
                for undone in 0..3 {
                    let _ = fixture.library.undo();
                    check_hover_queries(
                        &fixture,
                        &mut dice,
                        &format!("seed {seed}, undo {undone} after {step}"),
                    );
                }
                for redone in 0..2 {
                    let _ = fixture.library.redo();
                    check_hover_queries(
                        &fixture,
                        &mut dice,
                        &format!("seed {seed}, redo {redone} after {step}"),
                    );
                }
            }
        }
    }
}

#[test]
fn selections_that_are_entirely_in_the_target_are_recognised() {
    let mut fixture = Fixture::new();
    let mut dice = Dice(7);
    let everything = fixture.assets.clone();
    let target = fixture.collections()[0];
    fixture
        .library
        .apply_collection_command(&CollectionCommand::AddAssets {
            collection: target,
            assets: everything.clone(),
        })
        .expect("assets are filed");
    fixture
        .library
        .apply_tag_command(&TagCommand::Add {
            assets: everything.clone(),
            name: "Alpha/Beta".to_owned(),
        })
        .expect("assets are tagged");
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: everything.clone(),
            favorite: true,
        })
        .expect("favorites are set");
    check_hover_queries(&fixture, &mut dice, "everything is in the target");
    assert!(
        fixture
            .library
            .all_in_collection(&everything, target)
            .expect("collections are read")
    );
    assert!(
        fixture
            .library
            .all_are_favorite(&everything)
            .expect("favorites are read")
    );
    let tag = fixture
        .library
        .tags()
        .expect("tags are listed")
        .into_iter()
        .find(|tag| tag.name == "Beta")
        .expect("the leaf tag exists")
        .id;
    assert!(
        fixture
            .library
            .all_have_tag(&everything, tag)
            .expect("tags are read")
    );
    fixture
        .library
        .apply_collection_command(&CollectionCommand::Trash { id: target })
        .expect("collection is trashed");
    check_hover_queries(&fixture, &mut dice, "the target collection is trashed");
}
