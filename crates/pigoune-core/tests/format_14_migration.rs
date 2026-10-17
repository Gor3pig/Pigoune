use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, BLUR, Geometry, Options, RECTANGLE, Rect, SealRecord,
    Shape,
};
use pigoune_core::{
    AnnotatedImage, AssetCommand, AssetId, ImportOutcome, Library, MigrationStop, SealStatus,
    TagCommand, TextField,
};
use rusqlite::Connection;
use rusqlite::types::Value;

const CHILD_ROOT: &str = "PIGOUNE_MIGRATION_CHILD_ROOT";
const CHILD_STOP: &str = "PIGOUNE_MIGRATION_CHILD_STOP";
const SEAL_COLUMNS: [&str; 4] = [
    "seal_generation",
    "seal_base_hash",
    "seal_phase",
    "seal_journal",
];

type Dump = BTreeMap<String, (Vec<String>, Vec<Vec<String>>)>;

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/red-dot.png")
}

fn region(kind: &str, x: f32) -> Shape {
    Shape::new(
        kind,
        Geometry::Box(Rect {
            x,
            y: 2.0,
            width: 30.0,
            height: 40.0,
        }),
        Options::defaults(if kind == "blur" {
            BLUR.options
        } else {
            RECTANGLE.options
        }),
    )
}

struct Fixture {
    root: PathBuf,
    plain: AssetId,
    annotated: Vec<AssetId>,
}

fn become_format_13(root: &Path) {
    let connection = Connection::open(root.join("library.db")).expect("database");
    for column in SEAL_COLUMNS {
        connection
            .execute_batch(&format!(
                "ALTER TABLE asset_annotations DROP COLUMN {column};"
            ))
            .expect("column dropped");
    }
    connection
        .execute_batch("PRAGMA user_version = 13;")
        .expect("format 13");
}

fn build_format_13(parent: &Path) -> Fixture {
    let mut library = Library::create(parent, "Audit").expect("library");
    let sources = parent.join("sources");
    fs::create_dir_all(&sources).expect("sources");
    let source = |name: &str, number: usize| {
        let mut bytes = fs::read(sample()).expect("sample");
        bytes.extend_from_slice(number.to_string().as_bytes());
        let path = sources.join(name);
        fs::write(&path, bytes).expect("written");
        path
    };
    let ImportOutcome::Imported(plain) = library.import_file(&sample(), None).expect("plain")
    else {
        panic!("new");
    };
    library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![plain],
            favorite: true,
        })
        .expect("favorite");
    library
        .apply_asset_command(&AssetCommand::SetText {
            asset: plain,
            field: TextField::Note,
            value: "a note that must survive".to_owned(),
        })
        .expect("note");
    library
        .create_collection("Captures", None)
        .expect("collection");
    let mut annotated = Vec::new();
    for number in 1..=3_u8 {
        let rendered = source(&format!("rendered-{number}.png"), usize::from(number) * 10);
        let original = source(
            &format!("original-{number}.png"),
            usize::from(number) * 10 + 1,
        );
        let mut objects = vec![AnnotationObject::new(1, region("blur", f32::from(number)))];
        if number == 2 {
            objects.push(AnnotationObject::new(2, region("rectangle", 50.0)));
            objects[1].state.locked = true;
            objects[1].state.name = Some("Frame".to_owned());
        }
        let document = AnnotationDocument::new(None, objects);
        let id = library
            .import_annotated(AnnotatedImage {
                rendered: &rendered,
                original: &original,
                document: &document,
            })
            .expect("annotated");
        library
            .apply_asset_command(&AssetCommand::SetText {
                asset: id,
                field: TextField::Note,
                value: format!("annotated {number}"),
            })
            .expect("note");
        annotated.push(id);
    }
    library
        .apply_tag_command(&TagCommand::Add {
            assets: annotated.clone(),
            name: "capture".to_owned(),
        })
        .expect("tag");
    let root = library.root().to_path_buf();
    drop(library);
    fs::remove_dir_all(&sources).expect("sources removed");
    become_format_13(&root);
    Fixture {
        root,
        plain,
        annotated,
    }
}

fn dump(root: &Path) -> Dump {
    let connection = Connection::open(root.join("library.db")).expect("database");
    let names: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .expect("prepared")
        .query_map([], |row| row.get(0))
        .expect("queried")
        .collect::<Result<_, _>>()
        .expect("names");
    let mut dump = Dump::new();
    for name in names {
        let columns: Vec<String> = connection
            .prepare(&format!("SELECT name FROM pragma_table_info('{name}')"))
            .expect("prepared")
            .query_map([], |row| row.get(0))
            .expect("queried")
            .collect::<Result<_, _>>()
            .expect("columns");
        let kept: Vec<String> = columns
            .into_iter()
            .filter(|column| !column.starts_with("seal_"))
            .collect();
        let rows = connection
            .prepare(&format!(
                "SELECT {} FROM {name} ORDER BY 1",
                kept.join(", ")
            ))
            .expect("prepared")
            .query_map([], |row| {
                (0..kept.len())
                    .map(|index| row.get::<_, Value>(index).map(|value| format!("{value:?}")))
                    .collect::<Result<Vec<_>, _>>()
            })
            .expect("queried")
            .collect::<Result<_, _>>()
            .expect("rows");
        dump.insert(name, (kept, rows));
    }
    dump
}

fn files(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, folder: &Path, found: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(folder).expect("folder").flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("inside")
                .to_string_lossy()
                .into_owned();
            if relative.starts_with("cache") || relative.starts_with("library.db") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, found);
            } else {
                let hash = blake3::hash(&fs::read(&path).expect("file"))
                    .to_hex()
                    .to_string();
                found.insert(relative, hash);
            }
        }
    }
    let mut found = BTreeMap::new();
    walk(root, root, &mut found);
    found
}

fn schema_state(root: &Path) -> (u32, Vec<String>, String) {
    let connection = Connection::open(root.join("library.db")).expect("database");
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("version");
    let present = connection
        .prepare("SELECT name FROM pragma_table_info('asset_annotations') WHERE name LIKE 'seal_%' ORDER BY name")
        .expect("prepared")
        .query_map([], |row| row.get(0))
        .expect("queried")
        .collect::<Result<Vec<String>, _>>()
        .expect("columns");
    let verdict: String = connection
        .pragma_query_value(None, "integrity_check", |row| row.get(0))
        .expect("integrity");
    (version, present, verdict)
}

fn copy_folder(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("destination");
    for entry in fs::read_dir(from).expect("folder").flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copied");
        }
    }
}

fn assert_nothing_lost(before: &Dump, after: &Dump, what: &str) {
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "{what}: tables"
    );
    for (name, (columns, rows)) in before {
        assert_eq!(columns, &after[name].0, "{what}: columns of {name}");
        assert_eq!(rows, &after[name].1, "{what}: rows of {name}");
    }
}

fn assert_the_new_columns_hold_their_defaults(root: &Path) {
    let connection = Connection::open(root.join("library.db")).expect("database");
    let defaults: Vec<(i64, Option<String>, String, Option<String>)> = connection
        .prepare("SELECT seal_generation, seal_base_hash, seal_phase, seal_journal FROM asset_annotations")
        .expect("prepared")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .expect("queried")
        .collect::<Result<_, _>>()
        .expect("defaults");
    assert_eq!(defaults.len(), 3);
    assert!(
        defaults
            .iter()
            .all(|row| *row == (0, None, "none".to_owned(), None)),
        "{defaults:?}"
    );
}

fn assert_still_usable(library: &mut Library, fixture: &Fixture, scratch: &Path) {
    let id = fixture.annotated[0];
    let copies = library.export_copies(&[id]).expect("export works");
    assert_eq!(copies.len(), 1);
    let rendered = scratch.join("edited.png");
    let mut bytes = fs::read(sample()).expect("sample");
    bytes.extend_from_slice(b"edited after the migration");
    fs::write(&rendered, &bytes).expect("written");
    let document = library.annotation_document(id).expect("doc").expect("some");
    library
        .update_annotated(id, &rendered, &document)
        .expect("a save works");
    let baked = scratch.join("baked.png");
    bytes.extend_from_slice(b" and baked");
    fs::write(&baked, &bytes).expect("written");
    let mut sealed = document.clone();
    sealed.seal = Some(SealRecord {
        generation: 1,
        base: blake3::hash(&bytes).to_hex().to_string(),
    });
    sealed.objects[0].state.sealed = true;
    library
        .seal_annotated(id, &rendered, &baked, &sealed)
        .expect("a sealing works after the migration");
    assert_eq!(
        library.seal_status(id).expect("status"),
        SealStatus::Verified { generation: 1 }
    );
}

#[test]
fn a_format_13_library_keeps_every_element_file_and_value_when_it_becomes_format_14() {
    let workspace = tempfile::tempdir().expect("workspace");
    let fixture = build_format_13(workspace.path());
    let rows_before = dump(&fixture.root);
    let files_before = files(&fixture.root);
    let documents_before: Vec<String> = rows_before["asset_annotations"]
        .1
        .iter()
        .map(|row| row[1].clone())
        .collect();
    assert_eq!(schema_state(&fixture.root).0, 13);
    assert!(schema_state(&fixture.root).1.is_empty());
    assert_eq!(rows_before["assets"].1.len(), 4);
    assert_eq!(rows_before["asset_annotations"].1.len(), 3);
    assert_eq!(files_before.len(), 7, "{files_before:?}");

    let mut library = Library::open(&fixture.root).expect("migrates");

    assert_eq!(
        library.format_version().expect("version"),
        pigoune_core::CURRENT_FORMAT_VERSION
    );
    assert_eq!(
        files(&fixture.root),
        files_before,
        "every image is untouched"
    );
    let (version, columns, verdict) = schema_state(&fixture.root);
    assert_eq!(version, 14);
    assert_eq!(
        columns,
        [
            "seal_base_hash",
            "seal_generation",
            "seal_journal",
            "seal_phase"
        ]
    );
    assert_eq!(verdict, "ok");
    assert_nothing_lost(&rows_before, &dump(&fixture.root), "after the migration");
    assert_the_new_columns_hold_their_defaults(&fixture.root);

    let plain = library.asset(fixture.plain).expect("q").expect("asset");
    assert!(plain.is_favorite);
    assert_eq!(plain.note, "a note that must survive");
    let documents_after: Vec<String> = fixture
        .annotated
        .iter()
        .map(|id| library.annotations(*id).expect("q").expect("some").document)
        .collect();
    for document in &documents_after {
        assert!(documents_before.contains(&format!("{:?}", Value::Text(document.clone()))));
        assert!(
            AnnotationDocument::from_json(document)
                .expect("readable")
                .seal
                .is_none()
        );
    }
    let second = library
        .annotation_document(fixture.annotated[1])
        .expect("doc")
        .expect("some");
    assert!(second.objects[1].state.locked, "the states survive");
    assert_eq!(second.objects[1].state.name.as_deref(), Some("Frame"));
    for id in &fixture.annotated {
        assert_eq!(
            library.seal_status(*id).expect("status"),
            SealStatus::Unsealed
        );
        library.ensure_may_leave(*id).expect("may leave");
        assert!(
            library
                .original_path(*id)
                .expect("q")
                .expect("some")
                .is_file()
        );
    }
    assert!(library.seal_attention().expect("attention").is_empty());
    let report = library
        .health_plan()
        .expect("plan")
        .run(|_| std::ops::ControlFlow::Continue(()))
        .expect("done");
    assert_eq!(report.problems(), 0);

    assert_still_usable(&mut library, &fixture, workspace.path());
}

#[test]
fn a_migration_is_idempotent_and_a_second_opening_changes_nothing() {
    let workspace = tempfile::tempdir().expect("workspace");
    let fixture = build_format_13(workspace.path());
    drop(Library::open(&fixture.root).expect("migrates"));
    let rows = dump(&fixture.root);
    let files_after_first = files(&fixture.root);
    let state = schema_state(&fixture.root);
    drop(Library::open(&fixture.root).expect("opens again"));
    assert_nothing_lost(&rows, &dump(&fixture.root), "second opening");
    assert_eq!(files(&fixture.root), files_after_first);
    assert_eq!(schema_state(&fixture.root), state);
}

#[test]
fn the_child_that_migrates_and_aborts() {
    let (Ok(root), Ok(stop)) = (std::env::var(CHILD_ROOT), std::env::var(CHILD_STOP)) else {
        return;
    };
    let stop = match stop.as_str() {
        "after-two-columns" => MigrationStop::AfterTwoSealColumns,
        "before-commit" => MigrationStop::BeforeCommit,
        _ => MigrationStop::AfterCommit,
    };
    let _ = Library::open_halting_during_migration(Path::new(&root), stop);
    std::process::exit(0);
}

fn migrate_in_a_child_that_aborts(root: &Path, stop: &str) {
    let status = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "the_child_that_migrates_and_aborts",
            "--test-threads=1",
        ])
        .env(CHILD_ROOT, root)
        .env(CHILD_STOP, stop)
        .output()
        .expect("the child runs")
        .status;
    assert!(
        !status.success() && status.code().is_none(),
        "the child must be stopped brutally by its own abort, not finish: {status:?}"
    );
}

fn expect_only_whole_states(root: &Path, expect_version: u32) {
    let (version, columns, verdict) = schema_state(root);
    assert_eq!(verdict, "ok");
    assert_eq!(version, expect_version);
    match version {
        13 => assert!(columns.is_empty(), "no half migrated columns: {columns:?}"),
        14 => assert_eq!(columns.len(), 4, "every column or none: {columns:?}"),
        other => panic!("unexpected version {other}"),
    }
}

#[test]
fn an_abrupt_stop_during_the_migration_leaves_either_the_old_or_the_new_whole_state() {
    for (stop, expect_before_reopening) in [
        ("after-two-columns", 13),
        ("before-commit", 13),
        ("after-commit", 14),
    ] {
        let workspace = tempfile::tempdir().expect("workspace");
        let fixture = build_format_13(workspace.path());
        let rows_before = dump(&fixture.root);
        let files_before = files(&fixture.root);
        let isolated = workspace.path().join(format!("child-{stop}"));
        copy_folder(&fixture.root, &isolated);

        migrate_in_a_child_that_aborts(&isolated, stop);

        expect_only_whole_states(&isolated, expect_before_reopening);
        assert_nothing_lost(&rows_before, &dump(&isolated), stop);
        assert_eq!(files(&isolated), files_before, "{stop}: images untouched");

        let library = Library::open(&isolated).expect("the library opens after the stop");
        assert_eq!(
            library.format_version().expect("version"),
            pigoune_core::CURRENT_FORMAT_VERSION,
            "{stop}"
        );
        drop(library);
        expect_only_whole_states(&isolated, 14);
        assert_nothing_lost(&rows_before, &dump(&isolated), stop);
        assert_eq!(
            files(&isolated),
            files_before,
            "{stop}: images untouched after reopening"
        );
        let library = Library::open(&isolated).expect("opens once more");
        for id in &fixture.annotated {
            assert_eq!(
                library.seal_status(*id).expect("status"),
                SealStatus::Unsealed,
                "{stop}"
            );
        }
        assert!(
            library
                .asset(fixture.plain)
                .expect("q")
                .expect("asset")
                .is_favorite
        );
    }
}
