use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetColor, AssetCommand, AssetId, CURRENT_FORMAT_VERSION, ColorShare, DATABASE_FILE_NAME,
    DominantColor, ImportOutcome, Library, Rgb, TagCommand,
};
use rusqlite::Connection;

fn sample_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn import(library: &mut Library, name: &str) -> AssetId {
    match library
        .import_file(&sample_file(name), None)
        .expect("import succeeds")
    {
        ImportOutcome::Imported(id) => id,
        other => panic!("unexpected outcome {other:?}"),
    }
}

#[test]
fn an_empty_library_has_an_empty_overview() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let library = Library::create(workspace.path(), "Vide").expect("library is created");

    let overview = library.overview().expect("overview is read");

    assert_eq!(overview.resources, 0);
    assert_eq!(overview.collections, 0);
    assert_eq!(overview.tags, 0);
    assert_eq!(overview.format_version, CURRENT_FORMAT_VERSION);
}

#[test]
fn the_overview_counts_what_the_library_holds() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
    let svg = import(&mut library, "dark-circle.svg");
    let gif = import(&mut library, "spinner.gif");
    let png = import(&mut library, "red-dot.png");
    let trashed = import(&mut library, "blue-photo.jpg");
    library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![svg, gif],
            favorite: true,
        })
        .expect("favorites set");
    library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![png],
            name: "rouge".to_owned(),
        })
        .expect("tag added");
    library
        .create_collection("Logos", None)
        .expect("collection created");
    Connection::open(library.root().join(DATABASE_FILE_NAME))
        .expect("database opens")
        .execute(
            "UPDATE assets SET trashed_at_unix_ms = 1 WHERE id = ?1",
            [trashed.to_string()],
        )
        .expect("asset trashed");

    let overview = library.overview().expect("overview is read");

    assert_eq!(overview.resources, 3);
    assert_eq!(overview.favorites, 2);
    assert_eq!(overview.animated, 1);
    assert_eq!(overview.vectors, 1);
    assert_eq!(overview.in_trash, 1);
    assert_eq!(overview.collections, 1);
    assert_eq!(overview.tags, 1);

    let mut shares = library.format_shares().expect("shares are read");
    shares.sort_by_key(|share| share.format.code());
    let summary: Vec<(&str, usize, u64)> = shares
        .iter()
        .map(|share| (share.format.code(), share.count, share.bytes))
        .collect();
    let size_of = |name: &str| fs::metadata(sample_file(name)).expect("fixture").len();
    assert_eq!(
        summary,
        [
            ("gif", 1, size_of("spinner.gif")),
            ("png", 1, size_of("red-dot.png")),
            ("svg", 1, size_of("dark-circle.svg")),
        ]
    );

    let storage = library.storage_use().expect("storage is read");
    assert_eq!(
        storage.resources,
        size_of("spinner.gif") + size_of("red-dot.png") + size_of("dark-circle.svg")
    );
    assert_eq!(storage.trash, size_of("blue-photo.jpg"));
    assert!(storage.database > 0);
    assert_eq!(
        storage.total(),
        storage.resources + storage.trash + storage.thumbnails + storage.database
    );
}

#[test]
fn the_records_name_the_heaviest_largest_newest_and_oldest_resources() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Records").expect("library is created");
    assert_eq!(library.records().expect("records read").heaviest, None);
    let names = [
        "dark-circle.svg",
        "red-dot.png",
        "blue-photo.jpg",
        "navy-tile.bmp",
        "still.gif",
    ];
    let ids: Vec<AssetId> = names
        .iter()
        .map(|name| import(&mut library, name))
        .collect();
    let database = Connection::open(library.root().join(DATABASE_FILE_NAME)).expect("database");
    for (rank, id) in (1_i64..).zip(&ids) {
        database
            .execute(
                "UPDATE assets SET added_at_unix_ms = ?1 WHERE id = ?2",
                rusqlite::params![rank * 1000, id.to_string()],
            )
            .expect("date set");
    }
    database
        .execute(
            "UPDATE assets SET width = 9000, height = 9000 WHERE id = ?1",
            [ids[0].to_string()],
        )
        .expect("svg declared huge");
    database
        .execute(
            "UPDATE assets SET trashed_at_unix_ms = 1 WHERE id = ?1",
            [ids[4].to_string()],
        )
        .expect("gif trashed");

    let records = library.records().expect("records read");

    let heaviest = names
        .iter()
        .take(4)
        .max_by_key(|name| fs::metadata(sample_file(name)).expect("fixture").len())
        .expect("a heaviest fixture");
    let name_of = |asset: Option<pigoune_core::Asset>| asset.expect("a record").original_file_name;
    assert_eq!(name_of(records.heaviest), *heaviest);
    assert_eq!(name_of(records.largest), "navy-tile.bmp");
    assert_eq!(name_of(records.newest), "navy-tile.bmp");
    assert_eq!(name_of(records.oldest), "dark-circle.svg");
}

fn paint(library: &mut Library, asset: AssetId, families: &[AssetColor]) {
    let colors: Vec<DominantColor> = families
        .iter()
        .map(|family| DominantColor {
            family: *family,
            average: Rgb::new(10, 20, 30),
        })
        .collect();
    library
        .record_colors(asset, &colors)
        .expect("colors are recorded");
}

#[test]
fn colors_are_counted_once_per_resource_most_frequent_first() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Couleurs").expect("library is created");
    let first = import(&mut library, "dark-circle.svg");
    let second = import(&mut library, "red-dot.png");
    import(&mut library, "spinner.gif");
    let trashed = import(&mut library, "blue-photo.jpg");
    paint(
        &mut library,
        first,
        &[AssetColor::Red, AssetColor::Blue, AssetColor::Red],
    );
    paint(&mut library, second, &[AssetColor::Red]);
    paint(
        &mut library,
        trashed,
        &[AssetColor::Blue, AssetColor::Green],
    );
    Connection::open(library.root().join(DATABASE_FILE_NAME))
        .expect("database opens")
        .execute(
            "UPDATE assets SET trashed_at_unix_ms = 1 WHERE id = ?1",
            [trashed.to_string()],
        )
        .expect("asset trashed");

    let shares = library.color_shares().expect("color shares are read");

    assert_eq!(
        shares,
        vec![
            ColorShare {
                color: AssetColor::Red,
                count: 2
            },
            ColorShare {
                color: AssetColor::Blue,
                count: 1
            },
        ]
    );
}

#[test]
fn a_library_without_analysed_colors_has_no_color_share() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Vide").expect("library is created");
    import(&mut library, "red-dot.png");

    assert!(
        library
            .color_shares()
            .expect("color shares are read")
            .is_empty()
    );
}
