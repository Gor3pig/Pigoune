use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, BLUR, Geometry, Options, Rect, Shape,
};
use pigoune_core::{
    AssetCommand, AssetId, CURRENT_FORMAT_VERSION, ImportOutcome, Library, MigrationStop,
    TagCommand, TextField,
};
use rusqlite::Connection;
use rusqlite::types::Value;

const CHILD_ROOT: &str = "PIGOUNE_MIGRATION_CHILD_ROOT";
const CHILD_STOP: &str = "PIGOUNE_MIGRATION_CHILD_STOP";
type Dump = BTreeMap<String, (Vec<String>, Vec<Vec<String>>)>;

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/red-dot.png")
}

fn blur(x: f32) -> Shape {
    Shape::new(
        "blur",
        Geometry::Box(Rect {
            x,
            y: 2.0,
            width: 30.0,
            height: 40.0,
        }),
        Options::defaults(BLUR.options),
    )
}

struct Fixture {
    root: PathBuf,
    plain: AssetId,
    others: Vec<AssetId>,
}

fn become_format_12(root: &Path) {
    let connection = Connection::open(root.join("library.db")).expect("database");
    connection
        .execute_batch("DROP TABLE asset_annotations; PRAGMA user_version = 12;")
        .expect("format 12");
}

fn build_format_12(parent: &Path) -> Fixture {
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
    let mut others = Vec::new();
    for number in 1..=3_usize {
        let path = source(&format!("image-{number}.png"), number * 10);
        let ImportOutcome::Imported(id) = library.import_file(&path, None).expect("import") else {
            panic!("new");
        };
        library
            .apply_asset_command(&AssetCommand::SetText {
                asset: id,
                field: TextField::Note,
                value: format!("note {number}"),
            })
            .expect("note");
        others.push(id);
    }
    library
        .apply_tag_command(&TagCommand::Add {
            assets: others.clone(),
            name: "trip".to_owned(),
        })
        .expect("tag");
    let root = library.root().to_path_buf();
    drop(library);
    fs::remove_dir_all(&sources).expect("sources removed");
    become_format_12(&root);
    Fixture {
        root,
        plain,
        others,
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
        let kept: Vec<String> = columns.into_iter().collect();
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
    let columns = connection
        .prepare("SELECT name FROM pragma_table_info('asset_annotations') ORDER BY cid")
        .expect("prepared")
        .query_map([], |row| row.get(0))
        .expect("queried")
        .collect::<Result<Vec<String>, _>>()
        .expect("columns");
    let verdict: String = connection
        .pragma_query_value(None, "integrity_check", |row| row.get(0))
        .expect("integrity");
    (version, columns, verdict)
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

const NEW_COLUMNS: [&str; 4] = ["asset_id", "document", "has_masks", "updated_at_unix_ms"];

fn only_the_12_tables(dump: &Dump) -> Dump {
    dump.iter()
        .filter(|(name, _)| name.as_str() != "asset_annotations")
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

#[test]
fn a_format_12_library_keeps_every_element_file_and_value_when_it_becomes_format_13() {
    let workspace = tempfile::tempdir().expect("workspace");
    let fixture = build_format_12(workspace.path());
    let rows_before = dump(&fixture.root);
    let files_before = files(&fixture.root);
    assert_eq!(schema_state(&fixture.root).0, 12);
    assert!(schema_state(&fixture.root).1.is_empty());
    assert_eq!(rows_before["assets"].1.len(), 4);
    assert_eq!(files_before.len(), 4, "{files_before:?}");

    let mut library = Library::open(&fixture.root).expect("migrates");

    assert_eq!(
        library.format_version().expect("version"),
        CURRENT_FORMAT_VERSION
    );
    assert_eq!(
        files(&fixture.root),
        files_before,
        "every image is untouched"
    );
    let (version, columns, verdict) = schema_state(&fixture.root);
    assert_eq!((version, verdict.as_str()), (13, "ok"));
    assert_eq!(columns, NEW_COLUMNS);
    let after = dump(&fixture.root);
    assert_nothing_lost(
        &only_the_12_tables(&rows_before),
        &only_the_12_tables(&after),
        "migration",
    );
    assert!(after["asset_annotations"].1.is_empty());

    let plain = library.asset(fixture.plain).expect("q").expect("asset");
    assert!(plain.is_favorite);
    assert_eq!(plain.note, "a note that must survive");
    assert_eq!(library.tags_of(fixture.others[0]).expect("tags").len(), 1);
    assert_eq!(library.visible_assets().expect("assets").len(), 4);
    let report = library
        .health_plan()
        .expect("plan")
        .run(|_| std::ops::ControlFlow::Continue(()))
        .expect("done");
    assert_eq!(report.problems(), 0);

    let id = fixture.others[0];
    let bytes_before =
        fs::read(library.file_of(&library.asset(id).expect("q").expect("asset"))).expect("file");
    let document = AnnotationDocument::new(None, vec![AnnotationObject::new(1, blur(3.0))]);
    library
        .save_annotations(id, &document)
        .expect("annotating works after the migration");
    assert_eq!(
        library.annotation_document(id).expect("q").expect("some"),
        document
    );
    assert_eq!(
        fs::read(library.file_of(&library.asset(id).expect("q").expect("asset"))).expect("file"),
        bytes_before,
        "the image is still untouched"
    );
}

#[test]
fn a_migration_is_idempotent_and_a_second_opening_changes_nothing() {
    let workspace = tempfile::tempdir().expect("workspace");
    let fixture = build_format_12(workspace.path());
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
        12 => assert!(columns.is_empty(), "no half migrated table: {columns:?}"),
        13 => assert_eq!(columns, NEW_COLUMNS),
        other => panic!("unexpected version {other}"),
    }
}

#[test]
fn an_abrupt_stop_during_the_migration_leaves_either_the_old_or_the_new_whole_state() {
    for (stop, expect_before_reopening) in [("before-commit", 12), ("after-commit", 13)] {
        let workspace = tempfile::tempdir().expect("workspace");
        let fixture = build_format_12(workspace.path());
        let rows_before = only_the_12_tables(&dump(&fixture.root));
        let files_before = files(&fixture.root);
        let isolated = workspace.path().join(format!("child-{stop}"));
        copy_folder(&fixture.root, &isolated);

        migrate_in_a_child_that_aborts(&isolated, stop);

        expect_only_whole_states(&isolated, expect_before_reopening);
        assert_nothing_lost(&rows_before, &only_the_12_tables(&dump(&isolated)), stop);
        assert_eq!(files(&isolated), files_before, "{stop}: images untouched");

        let library = Library::open(&isolated).expect("the library opens after the stop");
        assert_eq!(
            library.format_version().expect("version"),
            CURRENT_FORMAT_VERSION,
            "{stop}"
        );
        drop(library);
        expect_only_whole_states(&isolated, 13);
        assert_nothing_lost(&rows_before, &only_the_12_tables(&dump(&isolated)), stop);
        assert_eq!(
            files(&isolated),
            files_before,
            "{stop}: images untouched after reopening"
        );
        let library = Library::open(&isolated).expect("opens once more");
        assert!(
            library
                .asset(fixture.plain)
                .expect("q")
                .expect("asset")
                .is_favorite
        );
    }
}
