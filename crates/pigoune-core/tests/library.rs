use pigoune_core::{
    AssetId, AssetRecord, DATABASE_SCHEMA_VERSION, DatabaseError, LIBRARY_FORMAT_VERSION, Library,
    LibraryDatabase, LibraryId, LibraryManifest, ManifestError, ObjectHash, ObjectRecord,
    OriginalFilename,
};
use rusqlite::{Connection, params};
use std::{fs, path::Path, path::PathBuf};
use tempfile::tempdir;

fn object(bytes: &[u8]) -> ObjectRecord {
    let hash = ObjectHash::from_bytes(bytes);
    let hash_text = hash.to_string();
    ObjectRecord {
        hash,
        size: bytes.len() as u64,
        relative_path: PathBuf::from(format!("objects/{}/{}.svg", &hash_text[..2], hash_text)),
    }
}

fn asset(object: &ObjectRecord, filename: Vec<u8>) -> AssetRecord {
    AssetRecord {
        id: AssetId::new(),
        object_hash: object.hash,
        original_filename: OriginalFilename::from_bytes(filename).unwrap(),
        display_name: "Example".into(),
        imported_at_utc_us: 1_700_000_000_000_000,
    }
}

fn raw_db(root: &Path) -> Connection {
    let db = Connection::open(root.join("library.db")).unwrap();
    db.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    db
}

#[test]
fn library_id_is_distinct_and_round_trips() {
    let first = LibraryId::new();
    let second = LibraryId::new();
    assert_ne!(first, second);
    assert_eq!(first.to_string().parse::<LibraryId>().unwrap(), first);
    assert_eq!(LibraryId::from_bytes(first.to_bytes()).unwrap(), first);
    assert!(LibraryId::from_bytes([0; 16]).is_err());
    assert_eq!(first.to_string(), first.to_string().to_ascii_lowercase());
}

#[test]
fn manifest_round_trip_and_explicit_errors() {
    let directory = tempdir().unwrap();
    let id = LibraryId::new();
    LibraryManifest::new(id)
        .write_new(directory.path())
        .unwrap();
    assert_eq!(
        LibraryManifest::read(directory.path()).unwrap().library_id,
        id
    );
    let json = fs::read_to_string(directory.path().join("library.json")).unwrap();
    assert_eq!(json.matches("library_id").count(), 1);
    assert!(json.contains("\"type\": \"pigoune-library\""));
    assert!(json.contains("\"format_version\": 1"));
    assert_eq!(LIBRARY_FORMAT_VERSION, 1);

    let path = directory.path().join("library.json");
    fs::write(
        &path,
        format!(r#"{{"type":"other","library_id":"{id}","format_version":1}}"#),
    )
    .unwrap();
    assert!(matches!(
        LibraryManifest::read(directory.path()),
        Err(ManifestError::InvalidType(_))
    ));
    fs::write(
        &path,
        format!(r#"{{"type":"pigoune-library","library_id":"{id}","format_version":2}}"#),
    )
    .unwrap();
    assert!(matches!(
        LibraryManifest::read(directory.path()),
        Err(ManifestError::UnsupportedFormatVersion(2))
    ));
    fs::write(&path, "not JSON").unwrap();
    assert!(matches!(
        LibraryManifest::read(directory.path()),
        Err(ManifestError::Json(_))
    ));
    fs::write(
        &path,
        r#"{"type":"pigoune-library","library_id":"invalid","format_version":1}"#,
    )
    .unwrap();
    assert!(matches!(
        LibraryManifest::read(directory.path()),
        Err(ManifestError::InvalidLibraryId(_))
    ));
}

#[test]
fn new_database_uses_wal_foreign_keys_and_matching_metadata() {
    let directory = tempdir().unwrap();
    let library = Library::create(directory.path()).unwrap();
    assert_eq!(
        library.database.schema_version().unwrap(),
        DATABASE_SCHEMA_VERSION
    );
    assert_eq!(library.database.journal_mode().unwrap(), "wal");
    assert!(library.database.foreign_keys_enabled().unwrap());
    assert_eq!(library.database.library_id().unwrap(), library.id);
    assert!(directory.path().join("recovery").is_dir());
    assert!(directory.path().join("objects/.tmp").is_dir());
    assert!(directory.path().join("objects/.lock").is_file());
    assert_eq!(Library::open(directory.path()).unwrap().id, library.id);
}

#[test]
fn mismatched_library_id_and_newer_schema_are_rejected() {
    let directory = tempdir().unwrap();
    let library = Library::create(directory.path()).unwrap();
    assert!(matches!(
        LibraryDatabase::open(directory.path(), LibraryId::new()),
        Err(DatabaseError::LibraryIdMismatch { .. })
    ));
    fs::write(
        directory.path().join("library.json"),
        format!(
            "{{\"type\":\"pigoune-library\",\"library_id\":\"{}\",\"format_version\":1}}",
            LibraryId::new()
        ),
    )
    .unwrap();
    assert!(matches!(
        Library::open(directory.path()),
        Err(pigoune_core::LibraryError::Database(
            DatabaseError::LibraryIdMismatch { .. }
        ))
    ));
    let db = raw_db(directory.path());
    db.pragma_update(None, "user_version", 99).unwrap();
    assert!(matches!(
        LibraryDatabase::open(directory.path(), library.id),
        Err(DatabaseError::SchemaTooNew(99))
    ));
}

#[test]
fn empty_replacement_database_is_not_claimed_by_manifest() {
    let directory = tempdir().unwrap();
    let id = LibraryId::new();
    LibraryManifest::new(id)
        .write_new(directory.path())
        .unwrap();
    drop(Connection::open(directory.path().join("library.db")).unwrap());
    assert!(matches!(
        LibraryDatabase::open(directory.path(), id),
        Err(DatabaseError::UninitializedDatabase)
    ));
    let db = raw_db(directory.path());
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |row| row.get::<_, i32>(0))
            .unwrap(),
        0
    );
}

#[test]
fn strict_schema_rejects_invalid_fields() {
    let directory = tempdir().unwrap();
    let _library = Library::create(directory.path()).unwrap();
    let db = raw_db(directory.path());
    let object = object(b"bytes");
    let path = object.relative_path.to_str().unwrap();
    assert!(
        db.execute(
            "INSERT INTO objects VALUES (?1, 5, ?2)",
            params![b"short".as_slice(), path]
        )
        .is_err()
    );
    assert!(
        db.execute(
            "INSERT INTO objects VALUES (?1, -1, ?2)",
            params![object.hash.digest_bytes().as_slice(), path]
        )
        .is_err()
    );
    db.execute(
        "INSERT INTO objects VALUES (?1, 5, ?2)",
        params![object.hash.digest_bytes().as_slice(), path],
    )
    .unwrap();
    let id = AssetId::new();
    let hash = object.hash.digest_bytes();
    let insert = "INSERT INTO assets VALUES (?1, ?2, ?3, ?4, ?5)";
    assert!(
        db.execute(
            insert,
            params![
                b"short".as_slice(),
                hash.as_slice(),
                b"a.svg".as_slice(),
                "A",
                1
            ]
        )
        .is_err()
    );
    assert!(
        db.execute(
            insert,
            params![
                id.to_bytes().as_slice(),
                hash.as_slice(),
                b"".as_slice(),
                "A",
                1
            ]
        )
        .is_err()
    );
    assert!(
        db.execute(
            insert,
            params![
                id.to_bytes().as_slice(),
                hash.as_slice(),
                b"a.svg".as_slice(),
                "",
                1
            ]
        )
        .is_err()
    );
    assert!(
        db.execute(
            insert,
            params![
                id.to_bytes().as_slice(),
                hash.as_slice(),
                b"a.svg".as_slice(),
                "A",
                -1
            ]
        )
        .is_err()
    );
    db.execute(
        insert,
        params![
            id.to_bytes().as_slice(),
            hash.as_slice(),
            b"a.svg".as_slice(),
            "A",
            1
        ],
    )
    .unwrap();
    assert!(
        db.execute("DELETE FROM objects WHERE hash = ?1", [hash.as_slice()])
            .is_err()
    );
}

#[test]
fn exact_posix_names_round_trip() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    for bytes in [b"normal.svg".as_slice(), b"non-utf8-\xff.svg".as_slice()] {
        let name = OriginalFilename::from_os_str(OsStr::from_bytes(bytes)).unwrap();
        assert_eq!(name.as_bytes(), bytes);
        assert_eq!(name.to_os_string().as_os_str().as_bytes(), bytes);
    }
    assert!(OriginalFilename::from_bytes(Vec::new()).is_err());
    assert!(OriginalFilename::from_bytes(b"a/b".to_vec()).is_err());
    assert!(OriginalFilename::from_bytes(b"a\0b".to_vec()).is_err());

    let directory = tempdir().unwrap();
    let mut library = Library::create(directory.path()).unwrap();
    let record = object(b"non-utf8 name");
    let named_asset = asset(&record, b"non-utf8-\xff.svg".to_vec());
    library
        .database
        .import_published_asset(&record, &named_asset)
        .unwrap();
    let restored = library.database.get_asset(named_asset.id).unwrap().unwrap();
    assert_eq!(restored.original_filename.as_bytes(), b"non-utf8-\xff.svg");
}

#[test]
fn foreign_key_deduplication_and_object_reconciliation() {
    let directory = tempdir().unwrap();
    let mut library = Library::create(directory.path()).unwrap();
    let object = object(b"same bytes");
    let missing = asset(&object, b"missing.svg".to_vec());
    assert!(matches!(
        library.database.create_asset(&missing),
        Err(DatabaseError::MissingObject(_))
    ));
    library.database.register_object(&object).unwrap();
    library.database.register_object(&object).unwrap();
    let first = asset(&object, b"first.svg".to_vec());
    let second = asset(&object, b"second.svg".to_vec());
    library.database.create_asset(&first).unwrap();
    library.database.create_asset(&second).unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(library.database.get_asset(first.id).unwrap(), Some(first));
    assert_eq!(library.database.get_asset(second.id).unwrap(), Some(second));

    let mut changed = object.clone();
    changed.size += 1;
    assert!(matches!(
        library.database.register_object(&changed),
        Err(DatabaseError::ObjectConflict(_))
    ));
    changed = object.clone();
    changed.relative_path.set_extension("png");
    assert!(matches!(
        library.database.register_object(&changed),
        Err(DatabaseError::ObjectConflict(_))
    ));
}

#[test]
fn transaction_rolls_back_new_object_when_asset_insert_fails() {
    let directory = tempdir().unwrap();
    let mut library = Library::create(directory.path()).unwrap();
    let first_object = object(b"first");
    let first_asset = asset(&first_object, b"first.svg".to_vec());
    library
        .database
        .import_published_asset(&first_object, &first_asset)
        .unwrap();

    let second_object = object(b"second");
    let mut conflicting_asset = asset(&second_object, b"second.svg".to_vec());
    conflicting_asset.id = first_asset.id;
    assert!(
        library
            .database
            .import_published_asset(&second_object, &conflicting_asset)
            .is_err()
    );
    assert!(
        library
            .database
            .get_object(second_object.hash)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        library.database.get_asset(first_asset.id).unwrap(),
        Some(first_asset)
    );
}

#[test]
fn unsafe_object_paths_are_rejected_on_write_and_read() {
    let directory = tempdir().unwrap();
    let mut library = Library::create(directory.path()).unwrap();
    let mut record = object(b"path test");
    for path in [
        "/objects/x",
        "objects/../escape",
        "outside/file",
        "objects/aa/../file",
    ] {
        record.relative_path = PathBuf::from(path);
        assert!(matches!(
            library.database.register_object(&record),
            Err(DatabaseError::InvalidObjectPath(_))
        ));
    }
    record = object(b"path test");
    library.database.register_object(&record).unwrap();
    let db = raw_db(directory.path());
    db.execute(
        "UPDATE objects SET relative_path = '/outside' WHERE hash = ?1",
        [record.hash.digest_bytes().as_slice()],
    )
    .unwrap();
    assert!(matches!(
        library.database.get_object(record.hash),
        Err(DatabaseError::InvalidObjectPath(_))
    ));
}

#[test]
fn published_object_and_database_survive_source_removal() {
    use std::os::unix::ffi::OsStrExt;

    let directory = tempdir().unwrap();
    let source_dir = tempdir().unwrap();
    let source = source_dir.path().join("source-graphic.svg");
    fs::write(&source, b"<svg/>").unwrap();
    let mut library = Library::create(directory.path()).unwrap();
    struct AcceptStagedBytes;
    impl pigoune_core::StagedValidator for AcceptStagedBytes {
        type Output = ();
        type Error = std::io::Error;

        fn validate(&self, staged: &pigoune_core::StagedObject<'_>) -> Result<(), Self::Error> {
            use std::io::Read;
            let mut bytes = Vec::new();
            staged.open_read()?.read_to_end(&mut bytes)?;
            assert_eq!(bytes, b"<svg/>");
            Ok(())
        }
    }
    let stored = library
        .object_store
        .stage_file(&source)
        .unwrap()
        .validate_with(&AcceptStagedBytes)
        .unwrap()
        .publish()
        .unwrap()
        .stored
        .object;
    let asset = AssetRecord {
        id: AssetId::new(),
        object_hash: stored.hash,
        original_filename: OriginalFilename::from_os_str(source.file_name().unwrap()).unwrap(),
        display_name: "Graphic".into(),
        imported_at_utc_us: 1_700_000_000_000_000,
    };
    library
        .database
        .import_published_asset(&stored, &asset)
        .unwrap();
    fs::remove_file(&source).unwrap();
    assert_eq!(
        fs::read(directory.path().join(&stored.relative_path)).unwrap(),
        b"<svg/>"
    );
    assert_eq!(
        library.database.get_asset(asset.id).unwrap(),
        Some(asset.clone())
    );
    assert_eq!(
        library.database.get_object(stored.hash).unwrap(),
        Some(stored)
    );
    assert_eq!(
        asset
            .original_filename
            .to_os_string()
            .as_os_str()
            .as_bytes(),
        b"source-graphic.svg"
    );
    let db = raw_db(directory.path());
    let persisted: Vec<u8> = db
        .query_row(
            "SELECT original_filename FROM assets WHERE id = ?1",
            [asset.id.to_bytes().as_slice()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(persisted, b"source-graphic.svg");
    assert!(
        !persisted
            .windows(source_dir.path().as_os_str().as_bytes().len())
            .any(|window| window == source_dir.path().as_os_str().as_bytes())
    );
}
