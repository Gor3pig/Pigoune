use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetId, CACHE_DIR_NAME, CURRENT_FORMAT_VERSION, DATABASE_FILE_NAME,
    FILES_DIR_NAME, ImportOutcome, Library,
};
use rusqlite::Connection;
use tempfile::TempDir;

const FORMAT_BEFORE_ANIMATED_PNG_AND_WEBP: u32 = 1;
const FORMAT_BEFORE_BUCKETS: u32 = 2;
const THUMBNAIL_PIXELS: u32 = 256;

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
    (root, ids)
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
        Connection::open(flat.root.join(DATABASE_FILE_NAME))
            .expect("database opens")
            .pragma_update(None, "user_version", FORMAT_BEFORE_BUCKETS)
            .expect("version updated");
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
