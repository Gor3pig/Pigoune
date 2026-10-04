use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pigoune_core::{
    AnimationTiming, Asset, AssetFormat, AssetId, CollectionId, DATABASE_FILE_NAME, Dimensions,
    FILES_DIR_NAME, ImportError, ImportOutcome, Library, LibraryError,
};
use rusqlite::Connection;
use tempfile::TempDir;

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

    fn source(&self, name: &str, content: &[u8]) -> PathBuf {
        let path = self.sources.join(name);
        fs::write(&path, content).expect("source written");
        path
    }

    fn sample(&self, fixture_name: &str) -> PathBuf {
        self.source(fixture_name, &fixture_bytes(fixture_name))
    }

    fn import(&mut self, source: &Path) -> Result<ImportOutcome, ImportError> {
        self.library.import_file(source, None)
    }

    fn import_into(
        &mut self,
        source: &Path,
        target: CollectionId,
    ) -> Result<ImportOutcome, ImportError> {
        self.library.import_file(source, Some(target))
    }

    fn collection(&mut self, name: &str) -> CollectionId {
        self.library
            .create_collection(name, None)
            .expect("collection is created")
    }

    fn collections_of(&self, asset: AssetId) -> Vec<CollectionId> {
        self.library
            .collections_of(asset)
            .expect("collections are read")
    }

    fn put_in_trash(&self, asset: AssetId) {
        self.database()
            .execute(
                "UPDATE assets SET trashed_at_unix_ms = 1 WHERE id = ?1",
                [asset.to_string()],
            )
            .expect("asset trashed");
    }

    fn database(&self) -> Connection {
        Connection::open(self.library.root().join(DATABASE_FILE_NAME)).expect("database opens")
    }

    fn import_new(&mut self, source: &Path) -> Asset {
        match self.import(source).expect("import succeeds") {
            ImportOutcome::Imported(id) => self.asset(id),
            outcome => panic!("the file was unexpectedly a duplicate: {outcome:?}"),
        }
    }

    fn asset(&self, id: AssetId) -> Asset {
        self.library
            .asset(id)
            .expect("asset is read")
            .expect("asset exists")
    }

    fn files_dir(&self) -> PathBuf {
        self.library.root().join(FILES_DIR_NAME)
    }

    fn stored_folders(&self) -> Vec<String> {
        asset_folders(&self.files_dir())
    }

    fn asset_count(&self) -> u32 {
        self.database()
            .query_row("SELECT count(*) FROM assets", [], |row| row.get(0))
            .expect("assets counted")
    }
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    fs::read(path).expect("fixture exists")
}

fn dimensions(width: u32, height: u32) -> Dimensions {
    Dimensions::new(width, height).expect("valid dimensions")
}

fn icon_with_sides(sides: &[u8]) -> Vec<u8> {
    let count = u16::try_from(sides.len()).expect("few entries");
    let mut bytes = vec![0, 0, 1, 0];
    bytes.extend(count.to_le_bytes());
    for &side in sides {
        bytes.extend([side, side]);
        bytes.extend([0; 14]);
    }
    bytes
}

fn now_unix_ms() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after 1970");
    i64::try_from(elapsed.as_millis()).expect("realistic date")
}

#[test]
fn an_imported_file_is_copied_and_described() {
    let mut fixture = Fixture::new();
    let source = fixture.sample("dark-circle.svg");
    let original = fs::read(&source).expect("source readable");
    let before = now_unix_ms();

    let asset = fixture.import_new(&source);

    assert_eq!(asset.display_name, "dark-circle");
    assert_eq!(asset.original_file_name, "dark-circle.svg");
    assert_eq!(asset.format, AssetFormat::Svg);
    assert_eq!(asset.dimensions, Some(dimensions(98, 96)));
    assert_eq!(asset.byte_size, original.len() as u64);
    assert_eq!(
        asset.content_hash,
        blake3::hash(&original).to_hex().as_str()
    );
    assert!(!asset.is_animated);
    assert!(asset.embedded_sizes.is_empty());
    assert!((before..=now_unix_ms()).contains(&asset.added_at_unix_ms));
    assert_eq!(
        asset.stored_path,
        Path::new(FILES_DIR_NAME)
            .join(bucket_of(asset.id))
            .join(asset.id.to_string())
            .join("dark-circle.svg")
    );

    let copy = fixture.library.root().join(&asset.stored_path);
    assert_eq!(fs::read(copy).expect("copy readable"), original);
    assert_eq!(fs::read(&source).expect("source still readable"), original);
}

#[test]
fn every_raster_format_reports_its_dimensions() {
    let cases = [
        ("red-dot.png", AssetFormat::Png, dimensions(3, 2)),
        ("blue-photo.jpg", AssetFormat::Jpeg, dimensions(4, 3)),
        ("green-square.webp", AssetFormat::Webp, dimensions(5, 4)),
        ("still.gif", AssetFormat::Gif, dimensions(2, 2)),
        ("orange-banner.avif", AssetFormat::Avif, dimensions(6, 5)),
        ("purple-strip.jxl", AssetFormat::Jxl, dimensions(7, 3)),
        ("teal-column.tiff", AssetFormat::Tiff, dimensions(2, 6)),
        ("navy-tile.bmp", AssetFormat::Bmp, dimensions(8, 8)),
    ];
    let mut fixture = Fixture::new();

    for (name, format, size) in cases {
        let asset = fixture.import_new(&fixture.sample(name));
        assert_eq!(asset.format, format, "{name}");
        assert_eq!(asset.dimensions, Some(size), "{name}");
        assert!(!asset.is_animated, "{name}");
    }
}

#[test]
fn an_animated_gif_is_recognized() {
    let mut fixture = Fixture::new();

    let asset = fixture.import_new(&fixture.sample("spinner.gif"));

    assert_eq!(asset.format, AssetFormat::Gif);
    assert!(asset.is_animated);
}

#[test]
fn an_animated_gif_reports_its_frames_and_duration() {
    let mut fixture = Fixture::new();

    let asset = fixture.import_new(&fixture.sample("spinner.gif"));

    assert_eq!(
        fixture.library.animation_timing(&asset),
        Some(AnimationTiming {
            frames: 3,
            duration: Duration::from_millis(300),
        })
    );
}

#[test]
fn animated_png_and_webp_report_their_frames_and_duration() {
    let mut fixture = Fixture::new();

    for (name, format) in [
        ("blinking.png", AssetFormat::Png),
        ("blinking.webp", AssetFormat::Webp),
    ] {
        let asset = fixture.import_new(&fixture.sample(name));

        assert_eq!(asset.format, format, "{name}");
        assert_eq!(asset.dimensions, Some(dimensions(4, 4)), "{name}");
        assert!(asset.is_animated, "{name}");
        assert_eq!(
            fixture.library.animation_timing(&asset),
            Some(AnimationTiming {
                frames: 3,
                duration: Duration::from_millis(600),
            }),
            "{name}"
        );
    }
}

#[test]
fn a_still_image_has_no_animation_timing() {
    let mut fixture = Fixture::new();

    let asset = fixture.import_new(&fixture.sample("still.gif"));

    assert_eq!(fixture.library.animation_timing(&asset), None);
}

#[test]
fn an_icon_lists_its_sizes_and_shows_the_largest() {
    let mut fixture = Fixture::new();
    let source = fixture.source("app.ico", &icon_with_sides(&[32, 0, 16, 48]));

    let asset = fixture.import_new(&source);

    assert_eq!(asset.format, AssetFormat::Ico);
    assert_eq!(
        asset.embedded_sizes,
        [
            dimensions(16, 16),
            dimensions(32, 32),
            dimensions(48, 48),
            dimensions(256, 256)
        ]
    );
    assert_eq!(asset.dimensions, Some(dimensions(256, 256)));
}

#[test]
fn the_format_comes_from_the_content_not_the_extension() {
    let mut fixture = Fixture::new();
    let source = fixture.source("misnamed.jpg", &fixture_bytes("red-dot.png"));

    let asset = fixture.import_new(&source);

    assert_eq!(asset.format, AssetFormat::Png);
    assert_eq!(asset.original_file_name, "misnamed.jpg");
}

#[test]
fn an_unsupported_file_is_refused_without_leaving_anything() {
    let mut fixture = Fixture::new();
    let cases = [
        fixture.source("notes.png", b"just some text"),
        fixture.source("page.svg", b"<html><body>hello</body></html>"),
        fixture.source("empty.png", b""),
        fixture.sources.clone(),
    ];

    for source in cases {
        let result = fixture.import(&source);
        assert!(
            matches!(result, Err(ImportError::UnsupportedFormat(ref path)) if *path == source),
            "{}: {result:?}",
            source.display()
        );
    }
    assert!(fixture.stored_folders().is_empty());
    assert_eq!(fixture.asset_count(), 0);
}

#[test]
fn an_unreadable_file_is_refused_without_leaving_anything() {
    let mut fixture = Fixture::new();
    let png_signature_only = &fixture_bytes("red-dot.png")[..8];
    let cases = [
        fixture.source("truncated.png", png_signature_only),
        fixture.source(
            "broken.svg",
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"><g></svg>",
        ),
        fixture.sources.join("missing.png"),
    ];

    for source in cases {
        let result = fixture.import(&source);
        assert!(
            matches!(result, Err(ImportError::Unreadable(ref path)) if *path == source),
            "{}: {result:?}",
            source.display()
        );
    }
    assert!(fixture.stored_folders().is_empty());
    assert_eq!(fixture.asset_count(), 0);
}

#[test]
fn a_duplicate_is_reported_and_not_copied_again() {
    let mut fixture = Fixture::new();
    let first = fixture.import_new(&fixture.sample("red-dot.png"));
    let same_content = fixture.source("renamed copy.png", &fixture_bytes("red-dot.png"));

    let outcome = fixture.import(&same_content).expect("import succeeds");

    assert_eq!(outcome, ImportOutcome::AlreadyPresent(first.id));
    assert_eq!(fixture.stored_folders(), [first.id.to_string()]);
    assert_eq!(fixture.asset_count(), 1);
}

#[test]
fn a_failed_copy_leaves_the_library_unchanged() {
    let mut fixture = Fixture::new();
    let source = fixture.sample("red-dot.png");
    let files_dir = fixture.files_dir();
    fs::set_permissions(&files_dir, fs::Permissions::from_mode(0o555)).expect("locked");

    let result = fixture.import(&source);

    fs::set_permissions(&files_dir, fs::Permissions::from_mode(0o755)).expect("unlocked");
    assert!(
        matches!(
            result,
            Err(ImportError::Library(LibraryError::PermissionDenied))
        ),
        "{result:?}"
    );
    assert!(fixture.stored_folders().is_empty());
    assert_eq!(fixture.asset_count(), 0);
}

#[test]
fn imported_assets_are_still_there_after_reopening() {
    let mut fixture = Fixture::new();
    let asset = fixture.import_new(&fixture.sample("spinner.gif"));
    let root = fixture.library.root().to_path_buf();

    drop(fixture.library);
    let reopened = Library::open(&root).expect("library reopens");

    assert_eq!(reopened.asset(asset.id).expect("asset read"), Some(asset));
}

#[test]
fn leftovers_of_an_interrupted_import_are_removed_on_opening() {
    let fixture = Fixture::new();
    let root = fixture.library.root().to_path_buf();
    let leftover = fixture
        .files_dir()
        .join(".0199a8f2-0000-7000-8000-000000000000.partial");
    fs::create_dir(&leftover).expect("leftover created");
    fs::write(leftover.join("half.png"), b"\x89PNG").expect("half file written");

    drop(fixture.library);
    let _reopened = Library::open(&root).expect("library reopens");

    assert!(!leftover.exists());
}

#[test]
fn a_new_file_imported_into_a_collection_is_placed_in_it() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech");

    let outcome = fixture
        .import_into(&fixture.sample("red-dot.png"), tech)
        .expect("import succeeds");

    let ImportOutcome::Imported(id) = outcome else {
        panic!("unexpected outcome {outcome:?}");
    };
    assert_eq!(fixture.collections_of(id), [tech]);
}

#[test]
fn a_file_imported_without_a_collection_stays_unclassified() {
    let mut fixture = Fixture::new();
    fixture.collection("Tech");

    let asset = fixture.import_new(&fixture.sample("red-dot.png"));

    assert!(fixture.collections_of(asset.id).is_empty());
}

#[test]
fn a_duplicate_imported_into_another_collection_is_added_to_it() {
    let mut fixture = Fixture::new();
    let brands = fixture.collection("Brands");
    let tech = fixture.collection("Tech");
    let source = fixture.sample("red-dot.png");
    let Ok(ImportOutcome::Imported(id)) = fixture.import_into(&source, brands) else {
        panic!("first import fails");
    };

    let outcome = fixture.import_into(&source, tech).expect("import succeeds");

    assert_eq!(outcome, ImportOutcome::AddedToCollection(id));
    let mut expected = vec![brands, tech];
    expected.sort();
    assert_eq!(fixture.collections_of(id), expected);
    assert_eq!(fixture.stored_folders(), [id.to_string()]);
}

#[test]
fn a_duplicate_already_in_the_collection_changes_nothing() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech");
    let source = fixture.sample("red-dot.png");
    let Ok(ImportOutcome::Imported(id)) = fixture.import_into(&source, tech) else {
        panic!("first import fails");
    };

    let outcome = fixture.import_into(&source, tech).expect("import succeeds");

    assert_eq!(outcome, ImportOutcome::AlreadyPresent(id));
    assert_eq!(fixture.collections_of(id), [tech]);
}

#[test]
fn a_duplicate_imported_without_a_collection_keeps_its_collections() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech");
    let source = fixture.sample("red-dot.png");
    let Ok(ImportOutcome::Imported(id)) = fixture.import_into(&source, tech) else {
        panic!("first import fails");
    };

    let outcome = fixture.import(&source).expect("import succeeds");

    assert_eq!(outcome, ImportOutcome::AlreadyPresent(id));
    assert_eq!(fixture.collections_of(id), [tech]);
}

#[test]
fn a_duplicate_in_the_trash_is_restored_with_its_collections() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech");
    let source = fixture.sample("red-dot.png");
    let Ok(ImportOutcome::Imported(id)) = fixture.import_into(&source, tech) else {
        panic!("first import fails");
    };
    fixture.put_in_trash(id);

    let outcome = fixture.import(&source).expect("import succeeds");

    assert_eq!(outcome, ImportOutcome::RestoredFromTrash(id));
    assert_eq!(fixture.asset(id).trashed_at_unix_ms, None);
    assert_eq!(fixture.collections_of(id), [tech]);
    assert_eq!(fixture.stored_folders(), [id.to_string()]);
}

#[test]
fn a_duplicate_in_the_trash_imported_into_a_collection_is_restored_and_added() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech");
    let source = fixture.sample("red-dot.png");
    let asset = fixture.import_new(&source);
    fixture.put_in_trash(asset.id);

    let outcome = fixture.import_into(&source, tech).expect("import succeeds");

    assert_eq!(outcome, ImportOutcome::RestoredFromTrash(asset.id));
    assert_eq!(fixture.asset(asset.id).trashed_at_unix_ms, None);
    assert_eq!(fixture.collections_of(asset.id), [tech]);
}

#[test]
fn importing_into_a_missing_collection_is_refused_without_leaving_anything() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech");
    fixture
        .database()
        .execute("DELETE FROM collections", [])
        .expect("collection removed");

    let result = fixture.import_into(&fixture.sample("red-dot.png"), tech);

    assert!(
        matches!(result, Err(ImportError::CollectionNotFound(id)) if id == tech),
        "{result:?}"
    );
    assert!(fixture.stored_folders().is_empty());
    assert_eq!(fixture.asset_count(), 0);
}

#[test]
fn importing_into_a_trashed_collection_is_refused() {
    let mut fixture = Fixture::new();
    let tech = fixture.collection("Tech");
    fixture
        .database()
        .execute("UPDATE collections SET trashed_at_unix_ms = 1", [])
        .expect("collection trashed");

    let result = fixture.import_into(&fixture.sample("red-dot.png"), tech);

    assert!(
        matches!(result, Err(ImportError::CollectionNotFound(id)) if id == tech),
        "{result:?}"
    );
    assert_eq!(fixture.asset_count(), 0);
}

fn asset_folders(files_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(files_dir)
        .expect("files folder listed")
        .flat_map(|entry| {
            let entry = entry.expect("entry read");
            if is_bucket(&entry) {
                folder_names(&entry.path())
            } else {
                vec![entry.file_name().to_string_lossy().into_owned()]
            }
        })
        .collect();
    names.sort();
    names
}

fn is_bucket(entry: &fs::DirEntry) -> bool {
    entry.file_name().len() == 2 && entry.path().is_dir()
}

fn folder_names(folder: &Path) -> Vec<String> {
    fs::read_dir(folder)
        .expect("bucket listed")
        .map(|entry| {
            entry
                .expect("entry read")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

fn bucket_of(id: AssetId) -> String {
    let text = id.to_string();
    text[text.len() - 2..].to_owned()
}
