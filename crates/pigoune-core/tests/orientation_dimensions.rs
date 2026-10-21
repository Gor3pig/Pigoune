use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetFilter, AssetFormat, AssetId, AssetShape, AssetView, ImportOutcome, Library,
};
use rusqlite::Connection;

fn exif(orientation: u16) -> Vec<u8> {
    let mut tiff = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
    tiff.extend_from_slice(&1_u16.to_le_bytes());
    tiff.extend_from_slice(&0x0112_u16.to_le_bytes());
    tiff.extend_from_slice(&3_u16.to_le_bytes());
    tiff.extend_from_slice(&1_u32.to_le_bytes());
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    tiff
}

fn jpeg(width: u16, height: u16, orientation: Option<u16>) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8];
    if let Some(orientation) = orientation {
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend(exif(orientation));
        bytes.extend_from_slice(&[0xFF, 0xE1]);
        bytes.extend_from_slice(
            &u16::try_from(payload.len() + 2)
                .expect("small")
                .to_be_bytes(),
        );
        bytes.extend_from_slice(&payload);
    }
    bytes.extend_from_slice(&[0xFF, 0xC0, 0, 17, 8]);
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    bytes.extend_from_slice(&[0xFF, 0xDA, 0, 2, 0xFF, 0xD9]);
    bytes
}

fn png(width: u32, height: u32, orientation: Option<u16>) -> Vec<u8> {
    let chunk = |kind: &[u8; 4], data: &[u8]| {
        let mut bytes = u32::try_from(data.len())
            .expect("small")
            .to_be_bytes()
            .to_vec();
        bytes.extend_from_slice(kind);
        bytes.extend_from_slice(data);
        bytes.extend_from_slice(&[0; 4]);
        bytes
    };
    let mut header = width.to_be_bytes().to_vec();
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    bytes.extend(chunk(b"IHDR", &header));
    if let Some(orientation) = orientation {
        bytes.extend(chunk(b"eXIf", &exif(orientation)));
    }
    bytes.extend(chunk(b"IDAT", &[0; 4]));
    bytes.extend(chunk(b"IEND", &[]));
    bytes
}

fn tiff(width: u32, height: u32, orientation: Option<u16>) -> Vec<u8> {
    let mut entries: Vec<(u16, u16, u32)> = vec![(0x100, 4, width), (0x101, 4, height)];
    if let Some(orientation) = orientation {
        entries.push((0x112, 3, u32::from(orientation)));
    }
    let mut bytes = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
    bytes.extend_from_slice(&u16::try_from(entries.len()).expect("small").to_le_bytes());
    for (tag, kind, value) in entries {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    bytes
}

fn webp(width: u32, height: u32, orientation: Option<u16>) -> Vec<u8> {
    let canvas = |value: u32| {
        let wide = (value - 1).to_le_bytes();
        [wide[0], wide[1], wide[2]]
    };
    let mut features = vec![if orientation.is_some() { 0x08 } else { 0 }, 0, 0, 0];
    features.extend_from_slice(&canvas(width));
    features.extend_from_slice(&canvas(height));
    let mut chunks = b"VP8X".to_vec();
    chunks.extend_from_slice(&10_u32.to_le_bytes());
    chunks.extend_from_slice(&features);
    if let Some(orientation) = orientation {
        let payload = exif(orientation);
        chunks.extend_from_slice(b"EXIF");
        chunks.extend_from_slice(&u32::try_from(payload.len()).expect("small").to_le_bytes());
        chunks.extend_from_slice(&payload);
        if payload.len() % 2 == 1 {
            chunks.push(0);
        }
    }
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(
        &u32::try_from(chunks.len() + 4)
            .expect("small")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(b"WEBP");
    bytes.extend_from_slice(&chunks);
    bytes
}

fn avif(width: u32, height: u32, angle: Option<u8>) -> Vec<u8> {
    let boxed = |kind: &[u8; 4], payload: &[u8]| {
        let mut bytes = u32::try_from(payload.len() + 8)
            .expect("small")
            .to_be_bytes()
            .to_vec();
        bytes.extend_from_slice(kind);
        bytes.extend_from_slice(payload);
        bytes
    };
    let mut ispe = vec![0, 0, 0, 0];
    ispe.extend_from_slice(&width.to_be_bytes());
    ispe.extend_from_slice(&height.to_be_bytes());
    let mut properties = boxed(b"ispe", &ispe);
    if let Some(angle) = angle {
        properties.extend(boxed(b"irot", &[angle]));
    }
    let mut meta = vec![0, 0, 0, 0];
    meta.extend(boxed(b"hdlr", &[0; 20]));
    meta.extend(boxed(b"iprp", &boxed(b"ipco", &properties)));
    let mut ftyp = b"avif".to_vec();
    ftyp.extend_from_slice(&[0, 0, 0, 0]);
    ftyp.extend_from_slice(b"avifmif1");
    let mut file = boxed(b"ftyp", &ftyp);
    file.extend(boxed(b"meta", &meta));
    file.extend(boxed(b"mdat", &[0; 16]));
    file
}

struct Bench {
    _workspace: tempfile::TempDir,
    sources: PathBuf,
    library: Library,
}

impl Bench {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace");
        let library = Library::create(workspace.path(), "Essai").expect("library");
        let sources = workspace.path().join("sources");
        fs::create_dir_all(&sources).expect("sources");
        Self {
            _workspace: workspace,
            sources,
            library,
        }
    }

    fn import(&mut self, name: &str, bytes: &[u8]) -> AssetId {
        let path = self.sources.join(name);
        fs::write(&path, bytes).expect("written");
        match self.library.import_file(&path, None).expect("import") {
            ImportOutcome::Imported(id) => id,
            other => panic!("{other:?}"),
        }
    }

    fn size_of(&self, id: AssetId) -> (u32, u32) {
        let asset = self.library.asset(id).expect("q").expect("asset");
        let size = asset.dimensions.expect("dimensions");
        (size.width(), size.height())
    }
}

#[test]
fn a_picture_turned_by_its_metadata_is_stored_with_the_sides_it_is_shown_with() {
    let mut bench = Bench::new();
    for (orientation, expected) in [
        (None, (600, 400)),
        (Some(1), (600, 400)),
        (Some(2), (600, 400)),
        (Some(3), (600, 400)),
        (Some(4), (600, 400)),
        (Some(5), (400, 600)),
        (Some(6), (400, 600)),
        (Some(7), (400, 600)),
        (Some(8), (400, 600)),
        (Some(0), (600, 400)),
        (Some(9), (600, 400)),
    ] {
        let id = bench.import(
            &format!("photo-{orientation:?}.jpg"),
            &jpeg(600, 400, orientation),
        );
        assert_eq!(
            bench.size_of(id),
            expected,
            "JPEG with orientation {orientation:?}"
        );
    }
}

#[test]
fn every_format_that_carries_an_orientation_is_read() {
    let mut bench = Bench::new();
    let cases: Vec<(&str, Vec<u8>, Vec<u8>, AssetFormat)> = vec![
        (
            "tiff",
            tiff(300, 200, Some(1)),
            tiff(300, 200, Some(6)),
            AssetFormat::Tiff,
        ),
        (
            "png",
            png(300, 200, Some(1)),
            png(300, 200, Some(8)),
            AssetFormat::Png,
        ),
        (
            "webp",
            webp(300, 200, Some(1)),
            webp(300, 200, Some(6)),
            AssetFormat::Webp,
        ),
        (
            "avif",
            avif(300, 200, Some(0)),
            avif(300, 200, Some(1)),
            AssetFormat::Avif,
        ),
    ];
    for (name, upright, turned, format) in cases {
        let flat = bench.import(&format!("{name}-flat"), &upright);
        let sideways = bench.import(&format!("{name}-turned"), &turned);
        assert_eq!(
            bench.library.asset(flat).expect("q").expect("asset").format,
            format,
            "{name}"
        );
        assert_eq!(bench.size_of(flat), (300, 200), "{name} upright");
        assert_eq!(bench.size_of(sideways), (200, 300), "{name} turned");
    }
}

#[test]
fn the_shape_filter_agrees_with_the_sides_that_are_displayed() {
    let mut bench = Bench::new();
    let turned = bench.import("portrait.jpg", &jpeg(600, 400, Some(6)));
    let landscape = bench.import("landscape.jpg", &jpeg(600, 400, Some(1)));
    let only = |shape| {
        let filter = AssetFilter {
            shapes: vec![shape],
            ..AssetFilter::default()
        };
        bench
            .library
            .find_assets_in(AssetView::All, &filter)
            .expect("found")
            .into_iter()
            .map(|asset| asset.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(only(AssetShape::Portrait), vec![turned]);
    assert_eq!(only(AssetShape::Landscape), vec![landscape]);
    assert!(only(AssetShape::Square).is_empty());
}

fn become_format_12(root: &Path) {
    let connection = Connection::open(root.join("library.db")).expect("database");
    connection
        .execute_batch("DROP TABLE asset_annotations; PRAGMA user_version = 12;")
        .expect("format 12");
}

fn stored_size(root: &Path, id: AssetId) -> (Option<u32>, Option<u32>) {
    let connection = Connection::open(root.join("library.db")).expect("database");
    connection
        .query_row(
            "SELECT width, height FROM assets WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("row")
}

fn forget_the_orientation_in_the_stored_size(root: &Path, id: AssetId, size: (u32, u32)) {
    let connection = Connection::open(root.join("library.db")).expect("database");
    connection
        .execute(
            "UPDATE assets SET width = ?2, height = ?3 WHERE id = ?1",
            rusqlite::params![id, size.0, size.1],
        )
        .expect("old sizes");
}

fn walk(folder: &Path, found: &mut Vec<(PathBuf, Vec<u8>)>) {
    for entry in fs::read_dir(folder).expect("folder").flatten() {
        if entry.path().is_dir() {
            walk(&entry.path(), found);
        } else {
            found.push((entry.path(), fs::read(entry.path()).expect("file")));
        }
    }
}

fn files_of(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    walk(&root.join("files"), &mut found);
    found.sort();
    found
}

#[test]
fn the_migration_corrects_the_sizes_of_turned_pictures_and_touches_nothing_else() {
    let mut bench = Bench::new();
    let turned = bench.import("turned.jpg", &jpeg(600, 400, Some(6)));
    let upright = bench.import("upright.jpg", &jpeg(600, 400, Some(1)));
    let square = bench.import("square.jpg", &jpeg(500, 500, Some(6)));
    let nonsense = bench.import("nonsense.jpg", &jpeg(600, 400, Some(42)));
    let missing = bench.import("missing.jpg", &jpeg(640, 480, Some(6)));
    let truncated = bench.import("truncated.jpg", &jpeg(640, 480, Some(8)));
    let drifted = bench.import("drifted.jpg", &jpeg(610, 410, Some(6)));
    let root = bench.library.root().to_path_buf();
    let missing_file = {
        let asset = bench.library.asset(missing).expect("q").expect("asset");
        bench.library.root().join(&asset.stored_path)
    };
    let truncated_file = {
        let asset = bench.library.asset(truncated).expect("q").expect("asset");
        bench.library.root().join(&asset.stored_path)
    };
    drop(bench.library);
    for (id, size) in [
        (turned, (600, 400)),
        (upright, (600, 400)),
        (square, (500, 500)),
        (nonsense, (600, 400)),
        (missing, (640, 480)),
        (truncated, (640, 480)),
        (drifted, (123, 456)),
    ] {
        forget_the_orientation_in_the_stored_size(&root, id, size);
    }
    fs::remove_file(&missing_file).expect("removed");
    let bytes = fs::read(&truncated_file).expect("read");
    fs::write(&truncated_file, &bytes[..14]).expect("truncated");
    become_format_12(&root);
    let before = files_of(&root);

    let library = Library::open(&root).expect("migrates");
    drop(library);

    assert_eq!(
        stored_size(&root, turned),
        (Some(400), Some(600)),
        "corrected"
    );
    assert_eq!(stored_size(&root, upright), (Some(600), Some(400)));
    assert_eq!(stored_size(&root, square), (Some(500), Some(500)));
    assert_eq!(
        stored_size(&root, nonsense),
        (Some(600), Some(400)),
        "bad metadata ignored"
    );
    assert_eq!(
        stored_size(&root, missing),
        (Some(640), Some(480)),
        "a missing file is skipped"
    );
    assert_eq!(
        stored_size(&root, truncated),
        (Some(640), Some(480)),
        "an unreadable one too"
    );
    assert_eq!(
        stored_size(&root, drifted),
        (Some(123), Some(456)),
        "only a plain swap of the stored sides is ever corrected"
    );
    assert_eq!(files_of(&root), before, "no original is touched");
}

#[test]
fn migrating_twice_never_turns_a_picture_back() {
    let mut bench = Bench::new();
    let turned = bench.import("turned.jpg", &jpeg(600, 400, Some(6)));
    let root = bench.library.root().to_path_buf();
    drop(bench.library);
    forget_the_orientation_in_the_stored_size(&root, turned, (600, 400));
    for _ in 0..3 {
        become_format_12(&root);
        drop(Library::open(&root).expect("migrates"));
        assert_eq!(stored_size(&root, turned), (Some(400), Some(600)));
    }
}

#[test]
fn a_big_library_is_corrected_in_a_reasonable_time_without_changing_other_rows() {
    let mut bench = Bench::new();
    let template = bench.import("template.jpg", &jpeg(600, 400, Some(6)));
    let root = bench.library.root().to_path_buf();
    let asset = bench.library.asset(template).expect("q").expect("asset");
    let source = bench.library.root().join(&asset.stored_path);
    drop(bench.library);
    let connection = Connection::open(root.join("library.db")).expect("database");
    let columns: Vec<String> = connection
        .prepare("SELECT name FROM pragma_table_info('assets')")
        .expect("prepared")
        .query_map([], |row| row.get(0))
        .expect("queried")
        .collect::<Result<_, _>>()
        .expect("columns");
    let copies = 1_500;
    let transaction = connection.unchecked_transaction().expect("transaction");
    for number in 0..copies {
        let id = format!("00000000-0000-7000-8000-{number:012}");
        let relative = format!("files/{:02}/{id}/copy.jpg", number % 100);
        let target = root.join(&relative);
        fs::create_dir_all(target.parent().expect("folder")).expect("folder");
        fs::copy(&source, &target).expect("copied");
        let select: Vec<String> = columns
            .iter()
            .map(|column| match column.as_str() {
                "id" | "content_hash" => "?1".to_owned(),
                "stored_path" => "?2".to_owned(),
                "width" => "600".to_owned(),
                "height" => "400".to_owned(),
                other => other.to_owned(),
            })
            .collect();
        transaction
            .execute(
                &format!(
                    "INSERT INTO assets ({}) SELECT {} FROM assets WHERE id = ?3",
                    columns.join(", "),
                    select.join(", ")
                ),
                rusqlite::params![id, relative, template],
            )
            .expect("copied row");
    }
    transaction.commit().expect("committed");
    drop(connection);
    forget_the_orientation_in_the_stored_size(&root, template, (600, 400));
    become_format_12(&root);

    let started = std::time::Instant::now();
    let library = Library::open(&root).expect("migrates");
    let elapsed = started.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "{copies} files were rechecked in {elapsed:?}"
    );
    let connection = Connection::open(root.join("library.db")).expect("database");
    let wrong: i64 = connection
        .query_row(
            "SELECT count(*) FROM assets WHERE width != 400 OR height != 600",
            [],
            |row| row.get(0),
        )
        .expect("counted");
    assert_eq!(wrong, 0);
    drop(library);
}
