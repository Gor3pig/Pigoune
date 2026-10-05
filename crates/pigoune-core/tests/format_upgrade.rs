use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetColor, AssetCommand, AssetFilter, AssetId, AssetView, CACHE_DIR_NAME,
    CURRENT_FORMAT_VERSION, CollectionCommand, CollectionLook, DATABASE_FILE_NAME, DominantColor,
    FILES_DIR_NAME, ImportOutcome, Library, Rgb,
};
use rusqlite::Connection;
use tempfile::TempDir;

const FORMAT_BEFORE_ANIMATED_PNG_AND_WEBP: u32 = 1;
const FORMAT_BEFORE_BUCKETS: u32 = 2;
const THUMBNAIL_PIXELS: u32 = 256;
const FORMAT_BEFORE_SMART_COLLECTIONS: u32 = 3;
const FORMAT_BEFORE_COLLECTION_LOOKS: u32 = 4;
const FORMAT_BEFORE_COLORS: u32 = 5;
const FORMAT_BEFORE_COLOR_AVERAGES: u32 = 6;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn import(library: &mut Library, name: &str) -> AssetId {
    match library
        .import_file(&fixture(name), None)
        .expect("import succeeds")
    {
        ImportOutcome::Imported(id) => id,
        other => panic!("unexpected outcome {other:?}"),
    }
}

fn library_as_saved_by_format_1(workspace: &TempDir, names: &[&str]) -> (PathBuf, Vec<AssetId>) {
    let mut library = Library::create(workspace.path(), "Ancienne").expect("library is created");
    let ids = names
        .iter()
        .map(|name| import(&mut library, name))
        .collect();
    let root = library.root().to_path_buf();
    drop(library);

    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .execute(
            "UPDATE assets SET is_animated = 0 WHERE format IN ('png', 'webp')",
            [],
        )
        .expect("animations forgotten");
    connection
        .pragma_update(None, "user_version", FORMAT_BEFORE_ANIMATED_PNG_AND_WEBP)
        .expect("version updated");
    forget_smart_collections(&connection);
    forget_collection_looks(&connection);
    forget_asset_colors(&connection);
    (root, ids)
}

fn forget_smart_collections(connection: &Connection) {
    connection
        .execute_batch("DROP TABLE smart_collections")
        .expect("smart collections forgotten");
}

fn forget_asset_colors(connection: &Connection) {
    connection
        .execute_batch("ALTER TABLE assets DROP COLUMN colors;")
        .expect("asset colors forgotten");
}

fn forget_smart_collection_colors(connection: &Connection) {
    forget_custom_colors(connection);
    connection
        .execute_batch("ALTER TABLE smart_collections DROP COLUMN colors;")
        .expect("smart collection colors forgotten");
}

fn forget_custom_colors(connection: &Connection) {
    connection
        .execute_batch("ALTER TABLE smart_collections DROP COLUMN custom_color;")
        .expect("custom colors forgotten");
}

fn forget_collection_looks(connection: &Connection) {
    connection
        .execute_batch(
            "ALTER TABLE collections DROP COLUMN icon;
             ALTER TABLE collections DROP COLUMN color;",
        )
        .expect("collection looks forgotten");
}

fn is_animated(library: &Library, id: AssetId) -> bool {
    library
        .asset(id)
        .expect("asset readable")
        .expect("asset exists")
        .is_animated
}

#[test]
fn opening_a_format_1_library_finds_its_animated_png_and_webp() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let (root, ids) = library_as_saved_by_format_1(
        &workspace,
        &[
            "blinking.png",
            "blinking.webp",
            "red-dot.png",
            "green-square.webp",
            "spinner.gif",
        ],
    );

    let library = Library::open(&root).expect("library opens");

    assert_eq!(
        library.format_version().expect("version readable"),
        CURRENT_FORMAT_VERSION
    );
    let animated: Vec<bool> = ids.iter().map(|id| is_animated(&library, *id)).collect();
    assert_eq!(animated, [true, true, false, false, true]);
}

#[test]
fn a_missing_file_does_not_prevent_the_upgrade() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let (root, ids) = library_as_saved_by_format_1(&workspace, &["blinking.png", "blinking.webp"]);
    let stored_path: String = Connection::open(root.join(DATABASE_FILE_NAME))
        .expect("database opens")
        .query_row(
            "SELECT stored_path FROM assets WHERE id = ?1",
            [ids[0].to_string()],
            |row| row.get(0),
        )
        .expect("stored path readable");
    fs::remove_file(root.join(stored_path)).expect("file removed");

    let library = Library::open(&root).expect("library opens");

    assert!(!is_animated(&library, ids[0]));
    assert!(is_animated(&library, ids[1]));
}

struct FlatLibrary {
    root: PathBuf,
    ids: Vec<AssetId>,
}

impl FlatLibrary {
    fn saved_by_format_2(workspace: &TempDir, names: &[&str]) -> Self {
        let mut library = Library::create(workspace.path(), "Plate").expect("library is created");
        let ids: Vec<AssetId> = names
            .iter()
            .map(|name| import(&mut library, name))
            .collect();
        let root = library.root().to_path_buf();
        drop(library);

        let flat = Self { root, ids };
        for (id, name) in flat.ids.iter().zip(names) {
            flat.flatten(*id, name);
        }
        let connection =
            Connection::open(flat.root.join(DATABASE_FILE_NAME)).expect("database opens");
        connection
            .pragma_update(None, "user_version", FORMAT_BEFORE_BUCKETS)
            .expect("version updated");
        forget_smart_collections(&connection);
        forget_collection_looks(&connection);
        forget_asset_colors(&connection);
        flat
    }

    fn flatten(&self, id: AssetId, name: &str) {
        fs::rename(
            self.bucketed_folder(id),
            self.files_dir().join(id.to_string()),
        )
        .expect("folder moved out of its bucket");
        Connection::open(self.root.join(DATABASE_FILE_NAME))
            .expect("database opens")
            .execute(
                "UPDATE assets SET stored_path = ?1 WHERE id = ?2",
                [format!("{FILES_DIR_NAME}/{id}/{name}"), id.to_string()],
            )
            .expect("stored path flattened");
        let thumbnails = self.thumbnails_dir();
        fs::create_dir_all(&thumbnails).expect("thumbnail folder created");
        fs::write(thumbnails.join(format!("{id}.png")), id.to_string())
            .expect("flat thumbnail written");
    }

    fn files_dir(&self) -> PathBuf {
        self.root.join(FILES_DIR_NAME)
    }

    fn thumbnails_dir(&self) -> PathBuf {
        self.root
            .join(CACHE_DIR_NAME)
            .join("thumbnails")
            .join(THUMBNAIL_PIXELS.to_string())
    }

    fn bucketed_folder(&self, id: AssetId) -> PathBuf {
        self.files_dir().join(bucket_of(id)).join(id.to_string())
    }

    fn top_level_assets(&self) -> Vec<AssetId> {
        [self.files_dir(), self.thumbnails_dir()]
            .iter()
            .flat_map(|folder| fs::read_dir(folder).expect("folder listed"))
            .filter_map(|entry| {
                let name = entry.expect("entry read").file_name();
                let name = name.to_string_lossy();
                AssetId::parse(name.strip_suffix(".png").unwrap_or(&name))
            })
            .collect()
    }
}

fn bucket_of(id: AssetId) -> String {
    let text = id.to_string();
    text[text.len() - 2..].to_owned()
}

fn stored_file(library: &Library, id: AssetId) -> PathBuf {
    let asset = library
        .asset(id)
        .expect("asset readable")
        .expect("asset exists");
    library.file_of(&asset)
}

#[test]
fn opening_a_format_2_library_sorts_files_and_thumbnails_into_buckets() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let flat = FlatLibrary::saved_by_format_2(
        &workspace,
        &["red-dot.png", "dark-circle.svg", "spinner.gif"],
    );

    let library = Library::open(&flat.root).expect("library opens");

    assert_eq!(
        library.format_version().expect("version readable"),
        CURRENT_FORMAT_VERSION
    );
    assert!(flat.top_level_assets().is_empty());
    for id in &flat.ids {
        let file = stored_file(&library, *id);
        assert!(file.starts_with(flat.bucketed_folder(*id)), "{file:?}");
        assert!(file.is_file(), "{file:?}");
        let thumbnail = library.thumbnail_file(*id, THUMBNAIL_PIXELS);
        assert_eq!(
            fs::read_to_string(thumbnail).expect("thumbnail moved"),
            id.to_string()
        );
    }
}

#[test]
fn an_interrupted_sorting_is_finished_at_the_next_opening() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Coupure").expect("library is created");
    let ids = [
        import(&mut library, "red-dot.png"),
        import(&mut library, "dark-circle.svg"),
    ];
    let root = library.root().to_path_buf();
    drop(library);
    let left_behind = root.join(FILES_DIR_NAME).join(ids[0].to_string());
    fs::rename(
        root.join(FILES_DIR_NAME)
            .join(bucket_of(ids[0]))
            .join(ids[0].to_string()),
        &left_behind,
    )
    .expect("folder left out of its bucket");

    let library = Library::open(&root).expect("library opens");

    assert!(!left_behind.exists());
    for id in ids {
        assert!(stored_file(&library, id).is_file());
    }
}

#[test]
fn a_trashed_resource_is_sorted_too_and_can_be_deleted_for_good() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let flat = FlatLibrary::saved_by_format_2(&workspace, &["red-dot.png"]);
    let id = flat.ids[0];
    let mut library = Library::open(&flat.root).expect("library opens");
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![id],
            trashed: true,
        })
        .expect("resource trashed");
    assert!(stored_file(&library, id).is_file());

    library.empty_trash().expect("trash emptied");

    assert!(flat.top_level_assets().is_empty());
    assert!(!flat.bucketed_folder(id).exists());
    assert!(!library.thumbnail_file(id, THUMBNAIL_PIXELS).exists());
}

#[test]
fn a_format_3_library_gains_smart_collections() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Avant").expect("library is created");
    let red = import(&mut library, "red-dot.png");
    let root = library.root().to_path_buf();
    drop(library);
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .pragma_update(None, "user_version", FORMAT_BEFORE_SMART_COLLECTIONS)
        .expect("version updated");
    forget_smart_collections(&connection);
    forget_collection_looks(&connection);
    forget_asset_colors(&connection);
    drop(connection);

    let mut library = Library::open(&root).expect("library opens");
    let id = library
        .create_smart_collection("Everything", AssetView::All, &AssetFilter::default())
        .expect("smart collection saved");

    assert_eq!(
        library.format_version().expect("version readable"),
        CURRENT_FORMAT_VERSION
    );
    let shown: Vec<AssetId> = library
        .visible_assets_in(AssetView::Smart(id))
        .expect("assets listed")
        .iter()
        .map(|asset| asset.id)
        .collect();
    assert_eq!(shown, vec![red]);
}

#[test]
fn a_format_4_library_keeps_its_collections_and_gains_their_looks() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Avant").expect("library is created");
    let logos = library
        .create_collection("Logos", None)
        .expect("collection is created");
    let root = library.root().to_path_buf();
    drop(library);
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .pragma_update(None, "user_version", FORMAT_BEFORE_COLLECTION_LOOKS)
        .expect("version updated");
    forget_collection_looks(&connection);
    forget_asset_colors(&connection);
    forget_smart_collection_colors(&connection);
    drop(connection);

    let mut library = Library::open(&root).expect("library opens");
    let before = library
        .collection(logos)
        .expect("collection is read")
        .expect("collection exists");
    let look = CollectionLook {
        icon: Some("emote-love".to_owned()),
        color: Some("pink".to_owned()),
    };
    library
        .apply_collection_command(&CollectionCommand::Restyle {
            id: logos,
            look: look.clone(),
        })
        .expect("collection is restyled");
    let after = library
        .collection(logos)
        .expect("collection is read")
        .expect("collection exists");

    assert_eq!(
        library.format_version().expect("version readable"),
        CURRENT_FORMAT_VERSION
    );
    assert_eq!(before.name, "Logos");
    assert_eq!(before.look, CollectionLook::default());
    assert_eq!(after.look, look);
}

#[test]
fn a_format_5_library_analyses_the_colors_of_its_resources_again() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Avant").expect("library is created");
    let red = import(&mut library, "red-dot.png");
    let id = library
        .create_smart_collection("Everything", AssetView::All, &AssetFilter::default())
        .expect("smart collection saved");
    let root = library.root().to_path_buf();
    drop(library);
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .pragma_update(None, "user_version", FORMAT_BEFORE_COLORS)
        .expect("version updated");
    forget_asset_colors(&connection);
    forget_smart_collection_colors(&connection);
    drop(connection);

    let library = Library::open(&root).expect("library opens");

    assert_eq!(
        library.format_version().expect("version readable"),
        CURRENT_FORMAT_VERSION
    );
    assert_eq!(library.assets_awaiting_colors(10).expect("listed"), [red]);
    let saved = library
        .smart_collection(id)
        .expect("read")
        .expect("smart collection kept");
    assert!(saved.filter.colors.is_empty());
}

#[test]
fn a_format_6_library_analyses_its_colors_again_to_learn_their_averages() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Avant").expect("library is created");
    let red = import(&mut library, "red-dot.png");
    library
        .record_colors(
            red,
            &[DominantColor {
                family: AssetColor::Red,
                average: Rgb::new(0xe0, 0x1b, 0x24),
            }],
        )
        .expect("colors recorded");
    let root = library.root().to_path_buf();
    drop(library);
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    connection
        .pragma_update(None, "user_version", FORMAT_BEFORE_COLOR_AVERAGES)
        .expect("version updated");
    forget_custom_colors(&connection);
    drop(connection);

    let library = Library::open(&root).expect("library opens");

    assert_eq!(
        library.format_version().expect("version readable"),
        CURRENT_FORMAT_VERSION
    );
    assert_eq!(library.assets_awaiting_colors(10).expect("listed"), [red]);
}
