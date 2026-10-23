use std::ops::ControlFlow;
use std::path::Path;

use pigoune_core::{
    AssetCommand, AssetId, CURRENT_FORMAT_VERSION, CollectionCommand, CollectionId,
    DATABASE_FILE_NAME, Library, SimilarSearch, VisualHash,
};
use rusqlite::{Connection, params};
use tempfile::TempDir;

const DETAIL_CELLS: usize = 256;
const PLAIN_CELL: u8 = 20;
const LIT_CELL: u8 = 230;
const LOOK: u64 = 0x0F0F_0F0F_0F0F_0F0F;

fn asset_id(number: usize) -> AssetId {
    AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
}

fn detail_for(hash: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(DETAIL_CELLS * 4);
    for cell in 0..DETAIL_CELLS {
        let value = if (hash >> (cell % 64)) & 1 == 1 {
            LIT_CELL
        } else {
            PLAIN_CELL
        };
        bytes.extend_from_slice(&[value, value, value, 255]);
    }
    bytes
}

struct Fixture {
    _workspace: TempDir,
    library: Library,
}

impl Fixture {
    fn with_twins(count: usize, detailed: bool) -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        let root = library.root().to_path_buf();
        drop(library);
        insert_twins(&root, count, detailed);
        Self {
            _workspace: workspace,
            library: Library::open(&root).expect("library opens"),
        }
    }

    fn collection(&mut self, name: &str, parent: Option<CollectionId>) -> CollectionId {
        self.library
            .create_collection(name, parent)
            .expect("collection is created")
    }

    fn put(&mut self, number: usize, collection: CollectionId) {
        self.library
            .apply_collection_command(&CollectionCommand::AddAssets {
                collection,
                assets: vec![asset_id(number)],
            })
            .expect("asset is added");
    }

    fn skip(&mut self, id: CollectionId, skip: bool) -> CollectionCommand {
        self.library
            .apply_collection_command(&CollectionCommand::SkipDuplicates { id, skip })
            .expect("setting applies")
    }

    fn groups(&self) -> Vec<Vec<AssetId>> {
        self.library.duplicate_groups().expect("groups are found")
    }

    fn skipped(&self, id: CollectionId) -> bool {
        self.library
            .collection(id)
            .expect("collection is read")
            .expect("collection exists")
            .skip_duplicates
    }
}

fn insert_twins(root: &Path, count: usize, detailed: bool) {
    let mut connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    let details =
        Connection::open(root.join("cache").join("visual-details.db")).expect("details file opens");
    let transaction = connection.transaction().expect("transaction starts");
    for number in 1..=count {
        transaction
            .execute(
                "INSERT INTO assets (id, display_name, original_file_name, stored_path, format,
                                     width, height, byte_size, content_hash, is_animated,
                                     embedded_sizes, added_at_unix_ms, visual_hash, colors)
                 VALUES (?1, ?2, ?3, ?4, 'png', 100, 100, 100, ?5, 0, '', ?6, ?7, NULL)",
                params![
                    asset_id(number).to_string(),
                    format!("asset {number}"),
                    format!("asset-{number}.png"),
                    format!("files/zz/{number}/asset-{number}.png"),
                    format!("hash-{number}"),
                    i64::try_from(number).expect("small"),
                    LOOK.to_be_bytes().to_vec(),
                ],
            )
            .expect("asset is inserted");
        if detailed {
            details
                .execute(
                    "INSERT INTO asset_details (asset_id, detail) VALUES (?1, ?2)",
                    params![asset_id(number).to_string(), detail_for(LOOK)],
                )
                .expect("detail is inserted");
        }
    }
    transaction.commit().expect("transaction commits");
}

fn pair() -> Vec<Vec<AssetId>> {
    vec![vec![asset_id(1), asset_id(2)]]
}

fn sorted(mut groups: Vec<Vec<AssetId>>) -> Vec<Vec<AssetId>> {
    for group in &mut groups {
        group.sort();
    }
    groups.sort();
    groups
}

#[test]
fn without_any_setting_the_twins_are_a_group() {
    let fixture = Fixture::with_twins(2, true);
    assert_eq!(sorted(fixture.groups()), pair());
}

#[test]
fn a_new_collection_is_not_skipped() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    assert!(!fixture.skipped(logos));
}

#[test]
fn twins_that_all_sit_in_a_skipped_collection_are_not_a_group() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    fixture.put(1, logos);
    fixture.put(2, logos);
    fixture.skip(logos, true);
    assert!(fixture.groups().is_empty());
}

#[test]
fn an_asset_in_a_skipped_collection_is_no_longer_compared_with_one_outside_any_collection() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    fixture.put(1, logos);
    fixture.skip(logos, true);
    assert!(fixture.groups().is_empty());
}

#[test]
fn an_asset_with_no_collection_is_always_compared() {
    let mut fixture = Fixture::with_twins(3, true);
    let logos = fixture.collection("Logos", None);
    fixture.put(3, logos);
    fixture.skip(logos, true);
    assert_eq!(sorted(fixture.groups()), pair());
}

#[test]
fn an_asset_kept_by_one_collection_that_is_not_skipped_stays_compared() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    let icons = fixture.collection("Icons", None);
    fixture.put(1, logos);
    fixture.put(1, icons);
    fixture.skip(logos, true);
    assert_eq!(sorted(fixture.groups()), pair());
}

#[test]
fn an_asset_whose_collections_are_all_skipped_is_left_out() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    let icons = fixture.collection("Icons", None);
    fixture.put(1, logos);
    fixture.put(1, icons);
    fixture.skip(logos, true);
    fixture.skip(icons, true);
    assert!(fixture.groups().is_empty());
}

#[test]
fn a_sub_collection_follows_its_skipped_parent_at_every_depth() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    let year = fixture.collection("2025", Some(logos));
    let spring = fixture.collection("Spring", Some(year));
    fixture.put(1, spring);
    fixture.put(2, year);
    fixture.skip(logos, true);
    assert!(fixture.groups().is_empty());
    assert!(
        !fixture.skipped(year),
        "the setting itself is not copied down"
    );
}

#[test]
fn skipping_a_sub_collection_leaves_its_parent_compared() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    let year = fixture.collection("2025", Some(logos));
    fixture.put(1, logos);
    fixture.put(2, year);
    fixture.skip(year, true);
    assert!(
        fixture.groups().is_empty(),
        "the asset of the sub-collection is left out"
    );
    fixture.put(2, logos);
    assert_eq!(sorted(fixture.groups()), pair());
}

#[test]
fn a_collection_in_the_trash_no_longer_keeps_its_assets_compared() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    let old = fixture.collection("Old", None);
    for number in [1, 2] {
        fixture.put(number, logos);
        fixture.put(number, old);
    }
    fixture.skip(logos, true);
    assert_eq!(sorted(fixture.groups()), pair());
    let undo = fixture
        .library
        .apply_collection_command(&CollectionCommand::Trash { id: old })
        .expect("collection is trashed");
    assert!(fixture.groups().is_empty());
    fixture
        .library
        .apply_collection_command(&undo)
        .expect("collection is back");
    assert_eq!(sorted(fixture.groups()), pair());
}

#[test]
fn stopping_the_skip_brings_the_group_back_and_the_inverse_undoes_each_step() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    fixture.put(1, logos);
    fixture.put(2, logos);
    let undo_skip = fixture.skip(logos, true);
    assert!(fixture.skipped(logos));
    assert!(fixture.groups().is_empty());
    let redo_skip = fixture
        .library
        .apply_collection_command(&undo_skip)
        .expect("undo applies");
    assert!(!fixture.skipped(logos));
    assert_eq!(sorted(fixture.groups()), pair());
    fixture
        .library
        .apply_collection_command(&redo_skip)
        .expect("redo applies");
    assert!(fixture.skipped(logos));
    assert!(fixture.groups().is_empty());
}

#[test]
fn the_same_setting_twice_changes_nothing_and_still_has_a_working_inverse() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    fixture.skip(logos, true);
    let undo = fixture.skip(logos, true);
    assert_eq!(
        undo,
        CollectionCommand::SkipDuplicates {
            id: logos,
            skip: true
        }
    );
    assert!(fixture.skipped(logos));
}

#[test]
fn a_collection_that_does_not_exist_is_refused() {
    let mut fixture = Fixture::with_twins(2, true);
    let missing = CollectionId::parse("00000000-0000-7000-8000-00000000ffff").expect("valid id");
    assert!(
        fixture
            .library
            .apply_collection_command(&CollectionCommand::SkipDuplicates {
                id: missing,
                skip: true
            })
            .is_err()
    );
}

#[test]
fn the_setting_is_kept_when_a_deleted_collection_is_brought_back_by_undo() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    fixture.skip(logos, true);
    let restore = fixture
        .library
        .apply_collection_command(&CollectionCommand::Delete { id: logos })
        .expect("collection is deleted");
    fixture
        .library
        .apply_collection_command(&restore)
        .expect("collection is created again");
    assert!(fixture.skipped(logos));
}

#[test]
fn the_setting_survives_closing_and_opening_the_library() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    fixture.skip(logos, true);
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);
    let library = Library::open(&root).expect("library opens");
    assert!(
        library
            .collection(logos)
            .expect("collection is read")
            .expect("collection exists")
            .skip_duplicates
    );
}

#[test]
fn the_search_for_assets_awaiting_a_detail_leaves_out_skipped_ones() {
    let mut fixture = Fixture::with_twins(2, false);
    let all = fixture
        .library
        .duplicate_candidates_awaiting_detail()
        .expect("candidates are found");
    assert_eq!(all.len(), 2);
    let logos = fixture.collection("Logos", None);
    fixture.put(1, logos);
    fixture.put(2, logos);
    fixture.skip(logos, true);
    assert!(
        fixture
            .library
            .duplicate_candidates_awaiting_detail()
            .expect("candidates are found")
            .is_empty()
    );
}

#[test]
fn the_similar_assets_of_one_asset_ignore_the_setting() {
    let mut fixture = Fixture::with_twins(2, true);
    let logos = fixture.collection("Logos", None);
    fixture.put(1, logos);
    fixture.put(2, logos);
    fixture.skip(logos, true);
    let search = fixture
        .library
        .similar_assets(asset_id(1), 10)
        .expect("search runs");
    match search {
        SimilarSearch::Found(similar) => {
            assert_eq!(similar.len(), 1);
            assert_eq!(similar[0].asset, asset_id(2));
        }
        other => panic!("unexpected search {other:?}"),
    }
}

#[test]
fn a_trashed_asset_in_a_skipped_collection_does_not_matter() {
    let mut fixture = Fixture::with_twins(3, true);
    let logos = fixture.collection("Logos", None);
    fixture.put(3, logos);
    fixture.skip(logos, true);
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![asset_id(3)],
            trashed: true,
        })
        .expect("asset is trashed");
    assert_eq!(sorted(fixture.groups()), pair());
}

#[test]
fn a_format_13_library_gains_the_setting_switched_off() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Avant").expect("library is created");
    let logos = library
        .create_collection("Logos", None)
        .expect("collection is created");
    let root = library.root().to_path_buf();
    drop(library);
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .execute("ALTER TABLE collections DROP COLUMN skip_duplicates", [])
        .expect("column is dropped");
    connection
        .pragma_update(None, "user_version", 13)
        .expect("version updated");
    drop(connection);

    let mut library = Library::open(&root).expect("library opens");
    assert_eq!(
        library.format_version().expect("version readable"),
        CURRENT_FORMAT_VERSION
    );
    let before = library
        .collection(logos)
        .expect("collection is read")
        .expect("collection exists");
    assert!(!before.skip_duplicates);
    library
        .apply_collection_command(&CollectionCommand::SkipDuplicates {
            id: logos,
            skip: true,
        })
        .expect("setting applies");
    assert!(
        library
            .collection(logos)
            .expect("collection is read")
            .expect("collection exists")
            .skip_duplicates
    );
}

#[test]
fn the_current_format_is_14() {
    assert_eq!(CURRENT_FORMAT_VERSION, 14);
}

#[test]
fn a_run_without_skipped_collections_still_finds_the_group_with_progress() {
    let fixture = Fixture::with_twins(2, true);
    let plan = fixture.library.duplicate_plan().expect("plan is made");
    let groups = plan
        .run(|_| ControlFlow::Continue(()))
        .expect("run completes");
    assert_eq!(groups.len(), 1);
    let _ = VisualHash::from_bits(LOOK);
}
