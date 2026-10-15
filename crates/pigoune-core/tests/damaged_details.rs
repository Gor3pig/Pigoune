use std::fs;
use std::path::Path;

use pigoune_core::{
    AssetCommand, AssetId, DATABASE_FILE_NAME, Library, VisualAnalysis, VisualDetail, VisualHash,
};
use rusqlite::{Connection, params};
use tempfile::TempDir;

const DETAIL_BYTES: usize = 1024;
const PAGE: usize = 4096;

fn id(number: usize) -> AssetId {
    AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
}

fn details_file(root: &Path) -> std::path::PathBuf {
    root.join("cache").join("visual-details.db")
}

fn library_of(count: usize) -> (TempDir, Library) {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let library = Library::create(workspace.path(), "Essai").expect("library is created");
    let root = library.root().to_path_buf();
    drop(library);
    let mut connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    let details = Connection::open(details_file(&root)).expect("details file opens");
    let transaction = connection.transaction().expect("transaction starts");
    for number in 1..=count {
        transaction
            .execute(
                "INSERT INTO assets (id, display_name, original_file_name, stored_path, format,
                                     width, height, byte_size, content_hash, is_animated,
                                     embedded_sizes, added_at_unix_ms, visual_hash)
                 VALUES (?1, ?2, ?3, ?4, 'png', 100, 100, 100, ?5, 0, '', ?6, ?7)",
                params![
                    id(number).to_string(),
                    format!("asset {number}"),
                    format!("asset-{number}.png"),
                    format!("files/zz/{number}/asset-{number}.png"),
                    format!("hash-{number}"),
                    i64::try_from(number).expect("small"),
                    9_u64.to_be_bytes().to_vec(),
                ],
            )
            .expect("asset is inserted");
        details
            .execute(
                "INSERT INTO asset_details (asset_id, detail) VALUES (?1, ?2)",
                params![id(number).to_string(), vec![7_u8; DETAIL_BYTES]],
            )
            .expect("detail is inserted");
    }
    transaction.commit().expect("transaction commits");
    drop(details);
    (workspace, Library::open(&root).expect("library opens"))
}

fn damage_data_pages(root: &Path) {
    let path = details_file(root);
    let mut bytes = fs::read(&path).expect("details file is read");
    assert!(bytes.len() > 3 * PAGE);
    bytes[PAGE..].fill(0);
    fs::write(&path, bytes).expect("details file is damaged");
}

fn trash(library: &mut Library, numbers: &[usize]) {
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: numbers.iter().map(|number| id(*number)).collect(),
            trashed: true,
        })
        .expect("trashed");
}

fn rows(root: &Path) -> Vec<String> {
    let details = Connection::open(details_file(root)).expect("details file opens");
    details
        .prepare("SELECT asset_id FROM asset_details ORDER BY asset_id")
        .expect("prepared")
        .query_map([], |row| row.get(0))
        .expect("queried")
        .collect::<Result<_, _>>()
        .expect("rows")
}

fn analysis() -> VisualAnalysis {
    VisualAnalysis {
        hash: VisualHash::from_bits(9),
        detail: VisualDetail::from_bytes(&[20; 1024]).expect("a detail"),
    }
}

#[test]
fn a_details_file_damaged_beyond_its_first_page_is_made_again_on_opening() {
    let (_workspace, library) = library_of(300);
    let root = library.root().to_path_buf();
    drop(library);
    damage_data_pages(&root);

    let library = Library::open(&root).expect("the library still opens");

    assert_eq!(
        library.asset(id(1)).expect("read").map(|a| a.id),
        Some(id(1))
    );
    drop(library);
    assert!(rows(&root).is_empty());
}

#[test]
fn emptying_the_trash_works_when_the_details_file_is_damaged_meanwhile() {
    let (_workspace, mut library) = library_of(300);
    let root = library.root().to_path_buf();
    trash(&mut library, &[1, 2, 3]);
    damage_data_pages(&root);

    let removed = library.empty_trash().expect("the trash is emptied");

    assert_eq!(removed, 3);
    assert!(library.asset(id(1)).expect("read").is_none());
    assert!(library.asset(id(4)).expect("read").is_some());
}

#[test]
fn removing_the_record_of_a_missing_file_works_when_the_details_file_is_damaged() {
    let (_workspace, mut library) = library_of(300);
    let root = library.root().to_path_buf();
    damage_data_pages(&root);

    library
        .remove_record_of_missing_file(id(1))
        .expect("the record is removed");

    assert!(library.asset(id(1)).expect("read").is_none());
    assert!(library.asset(id(2)).expect("read").is_some());
}

#[test]
fn a_hash_is_kept_even_when_its_detail_cannot_be_written() {
    let (_workspace, mut library) = library_of(300);
    let root = library.root().to_path_buf();
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .execute(
            "UPDATE assets SET visual_hash = NULL WHERE id = ?1",
            [id(1).to_string()],
        )
        .expect("hash forgotten");
    drop(connection);
    let _ = library.assets_awaiting_visual_hash(10);
    damage_data_pages(&root);

    let _ = library.record_visual_analysis_batch(&[(id(1), Some(analysis()))]);

    assert!(
        library
            .assets_awaiting_visual_hash(10)
            .expect("listed")
            .is_empty()
    );
}

#[test]
fn a_detail_recorded_for_a_deleted_asset_is_not_kept() {
    let (_workspace, mut library) = library_of(3);
    let root = library.root().to_path_buf();
    trash(&mut library, &[1]);
    library.empty_trash().expect("emptied");

    library
        .record_visual_details_batch(&[(id(1), VisualDetail::from_bytes(&[20; 1024]))])
        .expect("recorded");
    library
        .record_visual_analysis_batch(&[(id(1), Some(analysis()))])
        .expect("recorded");

    drop(library);
    assert_eq!(rows(&root), [id(2).to_string(), id(3).to_string()]);
}

#[test]
fn details_left_behind_by_deleted_assets_are_forgotten_on_opening() {
    let (_workspace, library) = library_of(3);
    let root = library.root().to_path_buf();
    drop(library);
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .execute("DELETE FROM assets WHERE id = ?1", [id(2).to_string()])
        .expect("asset deleted");
    drop(connection);

    let library = Library::open(&root).expect("opens");

    drop(library);
    assert_eq!(rows(&root), [id(1).to_string(), id(3).to_string()]);
}
