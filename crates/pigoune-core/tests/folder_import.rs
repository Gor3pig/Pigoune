use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use pigoune_core::{
    CollectionId, DATABASE_FILE_NAME, FILES_DIR_NAME, ImportControl, ImportEnding, ImportError,
    ImportProgress, ImportSummary, LARGE_FILE_BYTES, Library, LibraryError,
};
use rusqlite::Connection;
use tempfile::TempDir;

const COLLECTION_PATHS: &str = "WITH RECURSIVE paths(id, path) AS (
        SELECT id, name FROM collections WHERE parent_id IS NULL
        UNION ALL
        SELECT child.id, parent.path || '/' || child.name
        FROM collections child JOIN paths parent ON child.parent_id = parent.id
    )";

struct Fixture {
    _workspace: TempDir,
    library: Library,
    sources: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        let sources = workspace.path().join("sources");
        fs::create_dir(&sources).expect("sources folder created");
        Self {
            _workspace: workspace,
            library,
            sources,
        }
    }

    fn write(&self, relative_path: &str, content: &[u8]) -> PathBuf {
        let path = self.sources.join(relative_path);
        fs::create_dir_all(path.parent().expect("has a parent")).expect("folders created");
        fs::write(&path, content).expect("file written");
        path
    }

    fn sample(&self, relative_path: &str, fixture_name: &str) -> PathBuf {
        self.write(relative_path, &fixture_bytes(fixture_name))
    }

    fn folder(&self, relative_path: &str) -> PathBuf {
        let path = self.sources.join(relative_path);
        fs::create_dir_all(&path).expect("folder created");
        path
    }

    fn import(&mut self, paths: &[PathBuf], target: Option<CollectionId>) -> ImportSummary {
        self.library
            .import_paths(paths, target, |_| true, |_| ImportControl::Continue)
            .expect("import runs")
    }

    fn collection_paths(&self) -> Vec<String> {
        self.strings(&format!(
            "{COLLECTION_PATHS} SELECT path FROM paths ORDER BY path"
        ))
    }

    fn placements(&self) -> Vec<String> {
        self.strings(&format!(
            "{COLLECTION_PATHS}
             SELECT coalesce(paths.path || '/', '') || assets.original_file_name
             FROM assets
             LEFT JOIN asset_collections ON asset_collections.asset_id = assets.id
             LEFT JOIN paths ON paths.id = asset_collections.collection_id
             ORDER BY 1"
        ))
    }

    fn strings(&self, query: &str) -> Vec<String> {
        let connection =
            Connection::open(self.library.root().join(DATABASE_FILE_NAME)).expect("database opens");
        let mut statement = connection.prepare(query).expect("query prepares");
        statement
            .query_map([], |row| row.get(0))
            .expect("query runs")
            .collect::<Result<_, _>>()
            .expect("rows read")
    }
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    fs::read(path).expect("fixture exists")
}

fn assert_completed(summary: &ImportSummary) {
    assert!(
        matches!(summary.ending, ImportEnding::Completed),
        "{summary:?}"
    );
}

#[test]
fn a_folder_tree_becomes_nested_collections() {
    let mut fixture = Fixture::new();
    fixture.sample("Marques/github.svg", "github-mark.svg");
    fixture.sample("Marques/Tech/red.png", "red-dot.png");
    fixture.sample("Marques/Tech/Anciens/spinner.gif", "spinner.gif");
    fixture.write("Marques/Docs/notes.txt", b"not an image");
    fixture.folder("Marques/Vide");

    let summary = fixture.import(&[fixture.sources.join("Marques")], None);

    assert_completed(&summary);
    assert_eq!(summary.imported.len(), 3);
    assert_eq!(summary.unsupported, 1);
    assert!(summary.unreadable.is_empty());
    assert_eq!(
        fixture.collection_paths(),
        ["Marques", "Marques/Tech", "Marques/Tech/Anciens"]
    );
    assert_eq!(
        fixture.placements(),
        [
            "Marques/Tech/Anciens/spinner.gif",
            "Marques/Tech/red.png",
            "Marques/github.svg"
        ]
    );
}

#[test]
fn a_folder_imported_into_a_collection_goes_inside_it() {
    let mut fixture = Fixture::new();
    let clients = fixture
        .library
        .create_collection("Clients", None)
        .expect("collection is created");
    fixture.sample("Marques/red.png", "red-dot.png");

    let summary = fixture.import(&[fixture.sources.join("Marques")], Some(clients));

    assert_completed(&summary);
    assert_eq!(fixture.collection_paths(), ["Clients", "Clients/Marques"]);
    assert_eq!(fixture.placements(), ["Clients/Marques/red.png"]);
}

#[test]
fn reimporting_a_folder_reuses_its_collections() {
    let mut fixture = Fixture::new();
    fixture.sample("Marques/Tech/red.png", "red-dot.png");
    let folder = fixture.sources.join("Marques");
    fixture.import(std::slice::from_ref(&folder), None);
    fixture.sample("Marques/Tech/spinner.gif", "spinner.gif");

    let summary = fixture.import(&[folder], None);

    assert_completed(&summary);
    assert_eq!(summary.imported.len(), 1);
    assert_eq!(summary.already_present, 1);
    assert_eq!(summary.already_known.len(), 1);
    assert_eq!(fixture.collection_paths(), ["Marques", "Marques/Tech"]);
    assert_eq!(
        fixture.placements(),
        ["Marques/Tech/red.png", "Marques/Tech/spinner.gif"]
    );
}

#[test]
fn an_existing_collection_is_reused_whatever_the_letter_case() {
    let mut fixture = Fixture::new();
    fixture
        .library
        .create_collection("MARQUES", None)
        .expect("collection is created");
    fixture.sample("marques/red.png", "red-dot.png");

    fixture.import(&[fixture.sources.join("marques")], None);

    assert_eq!(fixture.collection_paths(), ["MARQUES"]);
    assert_eq!(fixture.placements(), ["MARQUES/red.png"]);
}

#[test]
fn a_duplicate_found_in_a_folder_is_added_to_its_collection() {
    let mut fixture = Fixture::new();
    let loose = fixture.sample("loose.png", "red-dot.png");
    fixture.import(&[loose], None);
    fixture.sample("Marques/red.png", "red-dot.png");

    let summary = fixture.import(&[fixture.sources.join("Marques")], None);

    assert_eq!(summary.added_to_collection, 1);
    assert!(summary.imported.is_empty());
    assert_eq!(fixture.placements(), ["Marques/loose.png"]);
}

#[test]
fn hidden_entries_are_skipped_without_being_counted() {
    let mut fixture = Fixture::new();
    fixture.sample("Marques/red.png", "red-dot.png");
    fixture.sample("Marques/.thumbnail.png", "still.gif");
    fixture.sample("Marques/.git/logo.svg", "github-mark.svg");

    let summary = fixture.import(&[fixture.sources.join("Marques")], None);

    assert_eq!(summary.imported.len(), 1);
    assert_eq!(summary.unsupported, 0);
    assert_eq!(fixture.collection_paths(), ["Marques"]);
    assert_eq!(fixture.placements(), ["Marques/red.png"]);
}

#[test]
fn symbolic_links_inside_a_folder_are_not_followed() {
    let mut fixture = Fixture::new();
    let outside_file = fixture.sample("Ailleurs/logo.svg", "github-mark.svg");
    let outside_folder = fixture.folder("Ailleurs");
    fixture.sample("Marques/red.png", "red-dot.png");
    symlink(&outside_file, fixture.sources.join("Marques/link.svg")).expect("file link");
    symlink(&outside_folder, fixture.sources.join("Marques/Lien")).expect("folder link");
    symlink(
        fixture.sources.join("Marques"),
        fixture.sources.join("Marques/Boucle"),
    )
    .expect("loop link");

    let summary = fixture.import(&[fixture.sources.join("Marques")], None);

    assert_completed(&summary);
    assert_eq!(summary.imported.len(), 1);
    assert_eq!(fixture.placements(), ["Marques/red.png"]);
}

#[test]
fn unreadable_files_are_listed_and_the_rest_is_imported() {
    let mut fixture = Fixture::new();
    let truncated = fixture.write("Marques/truncated.png", &fixture_bytes("red-dot.png")[..8]);
    fixture.sample("Marques/spinner.gif", "spinner.gif");
    let missing = fixture.sources.join("missing");

    let summary = fixture.import(&[fixture.sources.join("Marques"), missing.clone()], None);

    assert_completed(&summary);
    assert_eq!(summary.imported.len(), 1);
    let mut unreadable = summary.unreadable.clone();
    unreadable.sort();
    let mut expected = vec![truncated, missing];
    expected.sort();
    assert_eq!(unreadable, expected);
}

#[test]
fn a_folder_without_any_image_creates_no_collection() {
    let mut fixture = Fixture::new();
    fixture.write("Documents/notes.txt", b"text");
    fixture.write("Documents/Broken/bad.png", b"\x89PNG\r\n\x1a\n");
    fixture.folder("Documents/Empty");

    let summary = fixture.import(&[fixture.sources.join("Documents")], None);

    assert_completed(&summary);
    assert_eq!(summary.unsupported, 1);
    assert_eq!(summary.unreadable.len(), 1);
    assert!(fixture.collection_paths().is_empty());
}

#[test]
fn files_and_folders_can_be_imported_together() {
    let mut fixture = Fixture::new();
    let loose = fixture.sample("loose.svg", "github-mark.svg");
    fixture.sample("Marques/red.png", "red-dot.png");

    let summary = fixture.import(&[loose, fixture.sources.join("Marques")], None);

    assert_eq!(summary.imported.len(), 2);
    assert_eq!(fixture.placements(), ["Marques/red.png", "loose.svg"]);
}

#[test]
fn progress_is_reported_before_each_file() {
    let mut fixture = Fixture::new();
    fixture.sample("Marques/a.png", "red-dot.png");
    fixture.sample("Marques/b.gif", "spinner.gif");
    fixture.sample("Marques/c.svg", "github-mark.svg");
    let mut reports = Vec::new();

    fixture
        .library
        .import_paths(
            &[fixture.sources.join("Marques")],
            None,
            |_| true,
            |progress| {
                reports.push(progress);
                ImportControl::Continue
            },
        )
        .expect("import runs");

    assert_eq!(
        reports,
        [0, 1, 2].map(|done| ImportProgress { done, total: 3 })
    );
}

#[test]
fn a_cancelled_import_keeps_what_was_already_imported() {
    let mut fixture = Fixture::new();
    fixture.sample("Marques/a.png", "red-dot.png");
    fixture.sample("Marques/b.gif", "spinner.gif");
    fixture.sample("Marques/c.svg", "github-mark.svg");

    let summary = fixture
        .library
        .import_paths(
            &[fixture.sources.join("Marques")],
            None,
            |_| true,
            |progress| {
                if progress.done == 1 {
                    ImportControl::Cancel
                } else {
                    ImportControl::Continue
                }
            },
        )
        .expect("import runs");

    assert!(
        matches!(summary.ending, ImportEnding::Cancelled),
        "{summary:?}"
    );
    assert_eq!(summary.imported.len(), 1);
    assert_eq!(fixture.placements(), ["Marques/a.png"]);
}

#[test]
fn a_serious_problem_stops_the_import_and_leaves_nothing_behind() {
    let mut fixture = Fixture::new();
    fixture.sample("Marques/a.png", "red-dot.png");
    fixture.sample("Marques/b.gif", "spinner.gif");
    let files_dir = fixture.library.root().join(FILES_DIR_NAME);
    fs::set_permissions(&files_dir, fs::Permissions::from_mode(0o555)).expect("locked");
    let mut reports = 0;

    let result = fixture.library.import_paths(
        &[fixture.sources.join("Marques")],
        None,
        |_| true,
        |_| {
            reports += 1;
            ImportControl::Continue
        },
    );

    fs::set_permissions(&files_dir, fs::Permissions::from_mode(0o755)).expect("unlocked");
    let summary = result.expect("import runs");
    assert!(
        matches!(
            summary.ending,
            ImportEnding::Interrupted(ImportError::Library(LibraryError::PermissionDenied))
        ),
        "{summary:?}"
    );
    assert_eq!(reports, 1);
    assert!(summary.imported.is_empty());
    assert!(fixture.collection_paths().is_empty());
    assert!(fixture.placements().is_empty());
}

#[test]
fn importing_into_a_missing_collection_is_refused_before_starting() {
    let mut fixture = Fixture::new();
    let clients = fixture
        .library
        .create_collection("Clients", None)
        .expect("collection is created");
    Connection::open(fixture.library.root().join(DATABASE_FILE_NAME))
        .expect("database opens")
        .execute("DELETE FROM collections", [])
        .expect("collection removed");
    fixture.sample("Marques/a.png", "red-dot.png");

    let result = fixture.library.import_paths(
        &[fixture.sources.join("Marques")],
        Some(clients),
        |_| true,
        |_| ImportControl::Continue,
    );

    assert!(
        matches!(result, Err(ImportError::CollectionNotFound(id)) if id == clients),
        "{result:?}"
    );
    assert!(fixture.placements().is_empty());
}

#[test]
fn only_newly_copied_files_over_50_mb_are_reported_as_large() {
    let mut fixture = Fixture::new();
    let padded_png = |name: &str, length: u64| {
        let path = fixture.sample(name, "red-dot.png");
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("sample opens");
        file.set_len(length).expect("sample padded");
        path
    };
    let at_limit = padded_png("Lourds/at-limit.png", LARGE_FILE_BYTES);
    let over_limit = padded_png("Lourds/over-limit.png", LARGE_FILE_BYTES + 1);
    fixture.sample("Lourds/small.gif", "spinner.gif");

    let first = fixture.import(&[fixture.sources.join("Lourds")], None);
    let again = fixture.import(&[at_limit, over_limit], None);

    assert_completed(&first);
    assert_eq!(first.imported.len(), 3);
    assert_eq!(first.large_imported, 1);
    assert_eq!(again.already_present, 2);
    assert_eq!(again.large_imported, 0);
}

#[test]
fn a_new_image_judged_damaged_is_listed_as_unreadable() {
    let mut fixture = Fixture::new();
    let damaged = fixture.sample("Marques/damaged.png", "red-dot.png");
    fixture.sample("Marques/fine.gif", "spinner.gif");

    let summary = fixture
        .library
        .import_paths(
            &[fixture.sources.join("Marques")],
            None,
            |path| !path.ends_with("damaged.png"),
            |_| ImportControl::Continue,
        )
        .expect("import runs");

    assert_completed(&summary);
    assert_eq!(summary.imported.len(), 1);
    assert_eq!(summary.unreadable, [damaged]);
    assert_eq!(fixture.placements(), ["Marques/fine.gif"]);
}

#[test]
fn only_new_supported_images_are_checked() {
    let mut fixture = Fixture::new();
    let known = fixture.sample("known.png", "red-dot.png");
    fixture.import(&[known], None);
    fixture.sample("Marques/again.png", "red-dot.png");
    fixture.sample("Marques/new.gif", "spinner.gif");
    fixture.write("Marques/notes.txt", b"text");
    let mut checked = Vec::new();

    fixture
        .library
        .import_paths(
            &[fixture.sources.join("Marques")],
            None,
            |path| {
                checked.push(path.file_name().expect("file name").to_owned());
                true
            },
            |_| ImportControl::Continue,
        )
        .expect("import runs");

    assert_eq!(checked, ["new.gif"]);
}

#[test]
fn every_file_of_a_large_folder_is_kept_after_reopening() {
    let mut fixture = Fixture::new();
    for index in 0..60 {
        fixture.write(&format!("Lot/{index}.svg"), &numbered_svg(index));
    }

    let summary = fixture.import(&[fixture.sources.join("Lot")], None);

    assert_completed(&summary);
    assert_eq!(summary.imported.len(), 60);
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);
    let reopened = Library::open(&root).expect("library reopens");
    assert_eq!(reopened.visible_assets().expect("assets listed").len(), 60);
    assert_eq!(stored_folders(&root).len(), 60);
}

#[test]
fn an_import_stopped_abruptly_leaves_no_file_without_its_record() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
    let root = library.root().to_path_buf();
    let folder = workspace.path().join("Lot");
    fs::create_dir(&folder).expect("folder created");
    for index in 0..3 {
        fs::write(folder.join(format!("{index}.svg")), numbered_svg(index)).expect("written");
    }

    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        library.import_paths(
            std::slice::from_ref(&folder),
            None,
            |_| true,
            |progress| {
                assert!(progress.done < 2, "the import stops abruptly");
                ImportControl::Continue
            },
        )
    }));
    drop(library);

    assert!(stopped.is_err());
    let reopened = Library::open(&root).expect("library reopens");
    let recorded = reopened.visible_assets().expect("assets listed");
    let mut recorded_folders: Vec<String> =
        recorded.iter().map(|asset| asset.id.to_string()).collect();
    recorded_folders.sort();
    assert_eq!(stored_folders(&root), recorded_folders);
}

fn numbered_svg(index: usize) -> Vec<u8> {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><rect width=\"{index}\" height=\"8\"/></svg>"
    )
    .into_bytes()
}

fn stored_folders(root: &Path) -> Vec<String> {
    let mut folders: Vec<String> = fs::read_dir(root.join(FILES_DIR_NAME))
        .expect("files folder listed")
        .map(|entry| {
            entry
                .expect("entry read")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    folders.sort();
    folders
}
