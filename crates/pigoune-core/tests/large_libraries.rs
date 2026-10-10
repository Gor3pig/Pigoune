use std::path::Path;

use pigoune_core::{
    AssetCommand, AssetId, AssetView, CollectionCommand, DATABASE_FILE_NAME, Library, TagCommand,
};
use rusqlite::{Connection, params};
use tempfile::TempDir;

const ASSET_COUNT: usize = 40_000;

fn asset_id(number: usize) -> AssetId {
    AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
}

fn insert_assets(root: &Path) {
    let mut connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    let transaction = connection.transaction().expect("transaction starts");
    for number in 1..=ASSET_COUNT {
        transaction
            .execute(
                "INSERT INTO assets (id, display_name, original_file_name, stored_path, format,
                                     width, height, byte_size, content_hash, is_animated,
                                     embedded_sizes, added_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, 'png', 8, 8, 100, ?5, 0, '', ?6)",
                params![
                    asset_id(number).to_string(),
                    format!("asset {number}"),
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

fn library_with_many_assets() -> (TempDir, Library, Vec<AssetId>) {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let library = Library::create(workspace.path(), "Essai").expect("library is created");
    let root = library.root().to_path_buf();
    drop(library);
    insert_assets(&root);
    let library = Library::open(&root).expect("library opens");
    let ids = (1..=ASSET_COUNT).map(asset_id).collect();
    (workspace, library, ids)
}

#[test]
fn one_command_can_trash_and_restore_more_assets_than_sql_variables_allow() {
    let (_workspace, mut library, ids) = library_with_many_assets();

    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: ids.clone(),
            trashed: true,
        })
        .expect("assets are trashed");
    assert_eq!(
        library
            .visible_assets_in(AssetView::Trash)
            .expect("listed")
            .len(),
        ASSET_COUNT
    );
    library
        .undo()
        .expect("undo runs")
        .expect("something is undone");

    assert_eq!(
        library
            .visible_assets_in(AssetView::All)
            .expect("listed")
            .len(),
        ASSET_COUNT
    );
}

#[test]
fn one_command_can_favorite_tag_and_file_more_assets_than_sql_variables_allow() {
    let (_workspace, mut library, ids) = library_with_many_assets();

    library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: ids.clone(),
            favorite: true,
        })
        .expect("assets are favorited");
    library
        .apply_tag_command(&TagCommand::Add {
            assets: ids.clone(),
            name: "everything".to_owned(),
        })
        .expect("assets are tagged");
    let collection = library
        .create_collection("All of it", None)
        .expect("collection is created");
    library
        .apply_collection_command(&CollectionCommand::AddAssets {
            collection,
            assets: ids,
        })
        .expect("assets are filed");

    let counts = library.view_counts().expect("counts are computed");
    assert_eq!(counts.of(AssetView::Favorites), ASSET_COUNT);
    assert_eq!(counts.of(AssetView::Collection(collection)), ASSET_COUNT);
}
