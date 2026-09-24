use pigoune_core::{
    AssetId, AssetRecord, DATABASE_SCHEMA_VERSION, DatabaseError, ImageFormat, ImageMetadata,
    LIBRARY_FORMAT_VERSION, Library, LibraryDatabase, LibraryError, LibraryId, LibraryManifest,
    ManifestError, ObjectHash, ObjectRecord, OriginalFilename, StoredObject,
};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    fs,
    path::Path,
    path::PathBuf,
    sync::{Arc, Barrier},
    thread,
};
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

fn image_metadata() -> ImageMetadata {
    ImageMetadata::new(ImageFormat::Svg, 3, 2, false).unwrap()
}

fn raw_db(root: &Path) -> Connection {
    let db = Connection::open(root.join("library.db")).unwrap();
    db.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    db
}

fn creation_siblings(parent: &Path) -> Vec<PathBuf> {
    fs::read_dir(parent)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".pigoune-create-")
        })
        .collect()
}

#[test]
fn creation_publishes_complete_library_and_reopens() {
    let parent = tempdir().unwrap();
    let destination = parent.path().join("library");
    assert!(!destination.exists());
    let library = Library::create(&destination).unwrap();
    assert_eq!(
        library.id,
        LibraryManifest::read(&destination).unwrap().library_id
    );
    assert_eq!(library.id, library.database.library_id().unwrap());
    assert_eq!(library.database.schema_version().unwrap(), 3);
    for entry in ["library.db", "library.json", "objects/.lock"] {
        assert!(destination.join(entry).is_file(), "missing {entry}");
    }
    for entry in ["objects/.tmp", "recovery"] {
        assert!(destination.join(entry).is_dir(), "missing {entry}");
    }
    assert!(creation_siblings(parent.path()).is_empty());
    let id = library.id;
    drop(library);
    assert_eq!(Library::open(&destination).unwrap().id, id);
}

#[test]
fn existing_empty_directory_is_preserved() {
    let parent = tempdir().unwrap();
    let destination = parent.path().join("library");
    fs::create_dir(&destination).unwrap();
    assert!(matches!(
        Library::create(&destination),
        Err(LibraryError::DestinationExists)
    ));
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    assert!(creation_siblings(parent.path()).is_empty());
}

#[test]
fn existing_nonempty_directory_is_preserved() {
    let parent = tempdir().unwrap();
    let destination = parent.path().join("library");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("personal.txt"), b"keep").unwrap();
    assert!(matches!(
        Library::create(&destination),
        Err(LibraryError::DestinationExists)
    ));
    assert_eq!(fs::read(destination.join("personal.txt")).unwrap(), b"keep");
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
}

#[test]
fn existing_file_is_preserved() {
    let parent = tempdir().unwrap();
    let destination = parent.path().join("library");
    fs::write(&destination, b"keep").unwrap();
    assert!(matches!(
        Library::create(&destination),
        Err(LibraryError::DestinationExists)
    ));
    assert_eq!(fs::read(&destination).unwrap(), b"keep");
}

#[test]
fn existing_symlink_and_target_are_preserved() {
    let parent = tempdir().unwrap();
    let target = parent.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("personal.txt"), b"keep").unwrap();
    let destination = parent.path().join("library");
    std::os::unix::fs::symlink(&target, &destination).unwrap();
    assert!(matches!(
        Library::create(&destination),
        Err(LibraryError::DestinationExists)
    ));
    assert!(
        fs::symlink_metadata(&destination)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(target.join("personal.txt")).unwrap(), b"keep");
}

#[test]
fn invalid_destination_and_missing_parent_are_rejected() {
    let parent = tempdir().unwrap();
    for destination in [
        Path::new("/"),
        Path::new("."),
        Path::new(".."),
        Path::new("library"),
    ] {
        assert!(matches!(
            Library::create(destination),
            Err(LibraryError::InvalidDestination(_))
        ));
    }
    assert!(matches!(
        Library::create(&parent.path().join("missing/library")),
        Err(LibraryError::InvalidParent(_))
    ));
    assert!(!parent.path().join("missing").exists());
}

#[test]
fn concurrent_creation_never_replaces_the_winner() {
    let parent = tempdir().unwrap();
    let destination = parent.path().join("library");
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let destination = destination.clone();
            thread::spawn(move || {
                barrier.wait();
                Library::create(&destination).map(|library| library.id)
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(LibraryError::DestinationExists)))
            .count(),
        1
    );
    let winner = results.into_iter().find_map(Result::ok).unwrap();
    assert_eq!(Library::open(&destination).unwrap().id, winner);
    assert!(creation_siblings(parent.path()).is_empty());
}

#[test]
fn unrelated_creation_workspace_is_untouched() {
    let parent = tempdir().unwrap();
    let old = parent
        .path()
        .join(format!(".pigoune-create-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&old).unwrap();
    fs::write(old.join("personal.txt"), b"keep").unwrap();
    Library::create(&parent.path().join("library")).unwrap();
    assert_eq!(fs::read(old.join("personal.txt")).unwrap(), b"keep");
    assert_eq!(creation_siblings(parent.path()), vec![old]);
}

#[test]
fn returned_store_and_database_write_under_final_destination() {
    struct Accept;
    impl pigoune_core::StagedValidator for Accept {
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

    let parent = tempdir().unwrap();
    let destination = parent.path().join("library");
    let mut library = Library::create(&destination).unwrap();
    let stored = library
        .object_store
        .stage_reader(&b"<svg/>"[..], Some(std::ffi::OsStr::new("image.svg")))
        .unwrap()
        .validate_with(&Accept)
        .unwrap()
        .publish()
        .unwrap()
        .stored
        .object;
    assert_eq!(
        fs::read(destination.join(&stored.relative_path)).unwrap(),
        b"<svg/>"
    );
    let asset = asset(&stored, b"image.svg".to_vec());
    library
        .database
        .import_published_asset(&stored, image_metadata(), None, &asset)
        .unwrap();
    drop(library);
    let reopened = Library::open(&destination).unwrap();
    assert_eq!(reopened.database.get_asset(asset.id).unwrap(), Some(asset));
    assert!(creation_siblings(parent.path()).is_empty());
}

#[test]
fn schema_v1_migrates_existing_object_and_asset_without_guessing_metadata() {
    let directory = tempdir().unwrap();
    let id = LibraryId::new();
    let object = object(b"legacy content");
    let legacy_asset = asset(&object, b"legacy.svg".to_vec());
    let db = raw_db(directory.path());
    db.execute_batch(include_str!("../src/migrations/0001_initial.sql"))
        .unwrap();
    db.execute(
        "INSERT INTO library_metadata VALUES (1, ?1)",
        [id.to_bytes().as_slice()],
    )
    .unwrap();
    db.execute(
        "INSERT INTO objects VALUES (?1, ?2, ?3)",
        params![
            object.hash.digest_bytes().as_slice(),
            object.size as i64,
            object.relative_path.to_str().unwrap()
        ],
    )
    .unwrap();
    db.execute(
        "INSERT INTO assets VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            legacy_asset.id.to_bytes().as_slice(),
            object.hash.digest_bytes().as_slice(),
            legacy_asset.original_filename.as_bytes(),
            legacy_asset.display_name,
            legacy_asset.imported_at_utc_us
        ],
    )
    .unwrap();
    db.pragma_update(None, "user_version", 1).unwrap();
    drop(db);

    let mut database = LibraryDatabase::open(directory.path(), id).unwrap();
    assert_eq!(database.schema_version().unwrap(), 3);
    assert_eq!(database.journal_mode().unwrap(), "wal");
    assert!(database.foreign_keys_enabled().unwrap());
    assert_eq!(
        database.get_object(object.hash).unwrap(),
        Some(StoredObject {
            object: object.clone(),
            metadata: None,
            container: None,
        })
    );
    assert_eq!(
        database.get_asset(legacy_asset.id).unwrap(),
        Some(legacy_asset.clone())
    );
    let db = raw_db(directory.path());
    assert_eq!(
        db.query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
            .optional()
            .unwrap(),
        None
    );
    assert!(
        db.execute(
            "DELETE FROM objects WHERE hash = ?1",
            [object.hash.digest_bytes().as_slice()]
        )
        .is_err()
    );
    drop(db);

    let mut failed = asset(&object, b"failed.svg".to_vec());
    failed.id = legacy_asset.id;
    assert!(
        database
            .import_published_asset(&object, image_metadata(), None, &failed)
            .is_err()
    );
    assert_eq!(
        database.get_object(object.hash).unwrap().unwrap().metadata,
        None
    );

    let another = asset(&object, b"reimport.svg".to_vec());
    database
        .import_published_asset(&object, image_metadata(), None, &another)
        .unwrap();
    assert_eq!(
        database.get_object(object.hash).unwrap().unwrap().metadata,
        Some(image_metadata())
    );
    let mismatch = ImageMetadata::new(ImageFormat::Png, 3, 2, false).unwrap();
    let conflicting = asset(&object, b"conflict.svg".to_vec());
    assert!(matches!(
        database.import_published_asset(&object, mismatch, None, &conflicting),
        Err(DatabaseError::ImageMetadataConflict(_))
    ));
    assert_eq!(
        database.get_object(object.hash).unwrap().unwrap().metadata,
        Some(image_metadata())
    );
    assert!(database.get_asset(conflicting.id).unwrap().is_none());
}

#[test]
fn sql_image_metadata_is_all_or_none() {
    let parent = tempdir().unwrap();
    let directory = parent.path().join("library");
    let _library = Library::create(directory.as_path()).unwrap();
    let db = raw_db(directory.as_path());
    let record = object(b"sql constraints");
    let hash = record.hash.digest_bytes();
    let path = record.relative_path.to_str().unwrap();
    let insert = "INSERT INTO objects (hash, size_bytes, relative_path, format, width, height, animated) \
                  VALUES (?1, 15, ?2, ?3, ?4, ?5, ?6)";
    for (format, width, height, animated) in [
        (Some("png"), None, None, None),
        (None, Some(1), None, None),
        (Some("png"), Some(0), Some(2), Some(0)),
        (Some("png"), Some(3), Some(0), Some(0)),
        (Some("png"), Some(3), Some(2), Some(-1)),
        (Some("png"), Some(3), Some(2), Some(2)),
        (Some(""), Some(3), Some(2), Some(0)),
    ] {
        assert!(
            db.execute(
                insert,
                params![hash.as_slice(), path, format, width, height, animated]
            )
            .is_err(),
            "accepted invalid metadata: {format:?} {width:?} {height:?} {animated:?}"
        );
    }
    db.execute(
        insert,
        params![
            hash.as_slice(),
            path,
            None::<&str>,
            None::<i64>,
            None::<i64>,
            None::<i64>
        ],
    )
    .unwrap();
    db.execute(
        "UPDATE objects SET format = 'png', width = 3, height = 2, animated = 0 WHERE hash = ?1",
        [hash.as_slice()],
    )
    .unwrap();
    assert!(
        db.execute(
            "UPDATE objects SET width = NULL WHERE hash = ?1",
            [hash.as_slice()]
        )
        .is_err()
    );
    assert!(db.execute("UPDATE objects SET format = 'future', width = 3, height = 2, animated = 1 WHERE hash = ?1",
        [hash.as_slice()]).is_ok());
    drop(db);
    let library = Library::open(directory.as_path()).unwrap();
    assert!(matches!(
        library.database.get_object(record.hash),
        Err(DatabaseError::InvalidStoredValue("image format"))
    ));
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
    let parent = tempdir().unwrap();
    let directory = parent.path().join("library");
    let library = Library::create(directory.as_path()).unwrap();
    assert_eq!(
        library.database.schema_version().unwrap(),
        DATABASE_SCHEMA_VERSION
    );
    assert_eq!(library.database.journal_mode().unwrap(), "wal");
    assert!(library.database.foreign_keys_enabled().unwrap());
    assert_eq!(library.database.library_id().unwrap(), library.id);
    assert!(directory.as_path().join("recovery").is_dir());
    assert!(directory.as_path().join("objects/.tmp").is_dir());
    assert!(directory.as_path().join("objects/.lock").is_file());
    assert_eq!(Library::open(directory.as_path()).unwrap().id, library.id);
}

#[test]
fn mismatched_library_id_and_newer_schema_are_rejected() {
    let parent = tempdir().unwrap();
    let directory = parent.path().join("library");
    let library = Library::create(directory.as_path()).unwrap();
    assert!(matches!(
        LibraryDatabase::open(directory.as_path(), LibraryId::new()),
        Err(DatabaseError::LibraryIdMismatch { .. })
    ));
    fs::write(
        directory.as_path().join("library.json"),
        format!(
            "{{\"type\":\"pigoune-library\",\"library_id\":\"{}\",\"format_version\":1}}",
            LibraryId::new()
        ),
    )
    .unwrap();
    assert!(matches!(
        Library::open(directory.as_path()),
        Err(pigoune_core::LibraryError::Database(
            DatabaseError::LibraryIdMismatch { .. }
        ))
    ));
    let db = raw_db(directory.as_path());
    db.pragma_update(None, "user_version", 99).unwrap();
    assert!(matches!(
        LibraryDatabase::open(directory.as_path(), library.id),
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
    let parent = tempdir().unwrap();
    let directory = parent.path().join("library");
    let _library = Library::create(directory.as_path()).unwrap();
    let db = raw_db(directory.as_path());
    let object = object(b"bytes");
    let path = object.relative_path.to_str().unwrap();
    assert!(
        db.execute(
            "INSERT INTO objects (hash, size_bytes, relative_path) VALUES (?1, 5, ?2)",
            params![b"short".as_slice(), path]
        )
        .is_err()
    );
    assert!(
        db.execute(
            "INSERT INTO objects (hash, size_bytes, relative_path) VALUES (?1, -1, ?2)",
            params![object.hash.digest_bytes().as_slice(), path]
        )
        .is_err()
    );
    db.execute(
        "INSERT INTO objects (hash, size_bytes, relative_path) VALUES (?1, 5, ?2)",
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

    let parent = tempdir().unwrap();
    let directory = parent.path().join("library");
    let mut library = Library::create(directory.as_path()).unwrap();
    let record = object(b"non-utf8 name");
    let named_asset = asset(&record, b"non-utf8-\xff.svg".to_vec());
    library
        .database
        .import_published_asset(&record, image_metadata(), None, &named_asset)
        .unwrap();
    let restored = library.database.get_asset(named_asset.id).unwrap().unwrap();
    assert_eq!(restored.original_filename.as_bytes(), b"non-utf8-\xff.svg");
}

#[test]
fn foreign_key_deduplication_and_object_reconciliation() {
    let parent = tempdir().unwrap();
    let directory = parent.path().join("library");
    let mut library = Library::create(directory.as_path()).unwrap();
    let object = object(b"same bytes");
    let missing = asset(&object, b"missing.svg".to_vec());
    let db = raw_db(directory.as_path());
    assert!(db.execute("INSERT INTO assets (id, object_hash, original_filename, display_name, imported_at_utc_us) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![missing.id.to_bytes().as_slice(), object.hash.digest_bytes().as_slice(),
            missing.original_filename.as_bytes(), missing.display_name, missing.imported_at_utc_us]).is_err());
    let first = asset(&object, b"first.svg".to_vec());
    let second = asset(&object, b"second.svg".to_vec());
    library
        .database
        .import_published_asset(&object, image_metadata(), None, &first)
        .unwrap();
    library
        .database
        .import_published_asset(&object, image_metadata(), None, &second)
        .unwrap();
    let mut expected_ids = vec![first.id, second.id];
    expected_ids.sort_by_key(|id| id.to_bytes());
    assert_eq!(
        library.database.asset_ids_for_object(object.hash).unwrap(),
        expected_ids
    );
    assert_ne!(first.id, second.id);
    assert_eq!(library.database.get_asset(first.id).unwrap(), Some(first));
    assert_eq!(library.database.get_asset(second.id).unwrap(), Some(second));

    let mut changed = object.clone();
    changed.size += 1;
    assert!(matches!(
        library.database.import_published_asset(
            &changed,
            image_metadata(),
            None,
            &asset(&changed, b"changed.svg".to_vec())
        ),
        Err(DatabaseError::ObjectConflict(_))
    ));
    changed = object.clone();
    changed.relative_path.set_extension("png");
    assert!(matches!(
        library.database.import_published_asset(
            &changed,
            image_metadata(),
            None,
            &asset(&changed, b"changed.svg".to_vec())
        ),
        Err(DatabaseError::ObjectConflict(_))
    ));
}

#[test]
fn object_row_without_assets_is_not_a_logical_duplicate() {
    let parent = tempdir().unwrap();
    let directory = parent.path().join("library");
    let mut library = Library::create(directory.as_path()).unwrap();
    let record = object(b"orphan row");
    let db = raw_db(directory.as_path());
    db.execute(
        "INSERT INTO objects (hash, size_bytes, relative_path, format, width, height, animated) \
         VALUES (?1, ?2, ?3, 'svg', 3, 2, 0)",
        params![
            record.hash.digest_bytes().as_slice(),
            record.size as i64,
            record.relative_path.to_str().unwrap()
        ],
    )
    .unwrap();
    assert!(
        library
            .database
            .asset_ids_for_object(record.hash)
            .unwrap()
            .is_empty()
    );
    let first = asset(&record, b"first.svg".to_vec());
    library
        .database
        .import_published_asset(&record, image_metadata(), None, &first)
        .unwrap();
    assert_eq!(
        library.database.asset_ids_for_object(record.hash).unwrap(),
        vec![first.id]
    );
}

#[test]
fn transaction_rolls_back_new_object_when_asset_insert_fails() {
    let parent = tempdir().unwrap();
    let directory = parent.path().join("library");
    let mut library = Library::create(directory.as_path()).unwrap();
    let first_object = object(b"first");
    let first_asset = asset(&first_object, b"first.svg".to_vec());
    library
        .database
        .import_published_asset(&first_object, image_metadata(), None, &first_asset)
        .unwrap();

    let second_object = object(b"second");
    let mut conflicting_asset = asset(&second_object, b"second.svg".to_vec());
    conflicting_asset.id = first_asset.id;
    assert!(
        library
            .database
            .import_published_asset(&second_object, image_metadata(), None, &conflicting_asset)
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
    let parent = tempdir().unwrap();
    let directory = parent.path().join("library");
    let mut library = Library::create(directory.as_path()).unwrap();
    let mut record = object(b"path test");
    for path in [
        "/objects/x",
        "objects/../escape",
        "outside/file",
        "objects/aa/../file",
    ] {
        record.relative_path = PathBuf::from(path);
        assert!(matches!(
            library.database.import_published_asset(
                &record,
                image_metadata(),
                None,
                &asset(&record, b"path.svg".to_vec())
            ),
            Err(DatabaseError::InvalidObjectPath(_))
        ));
    }
    record = object(b"path test");
    library
        .database
        .import_published_asset(
            &record,
            image_metadata(),
            None,
            &asset(&record, b"path.svg".to_vec()),
        )
        .unwrap();
    let db = raw_db(directory.as_path());
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

    let parent = tempdir().unwrap();
    let directory = parent.path().join("library");
    let source_dir = tempdir().unwrap();
    let source = source_dir.path().join("source-graphic.svg");
    fs::write(&source, b"<svg/>").unwrap();
    let mut library = Library::create(directory.as_path()).unwrap();
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
        .import_published_asset(&stored, image_metadata(), None, &asset)
        .unwrap();
    fs::remove_file(&source).unwrap();
    assert_eq!(
        fs::read(directory.as_path().join(&stored.relative_path)).unwrap(),
        b"<svg/>"
    );
    assert_eq!(
        library.database.get_asset(asset.id).unwrap(),
        Some(asset.clone())
    );
    assert_eq!(
        library.database.get_object(stored.hash).unwrap(),
        Some(StoredObject {
            object: stored,
            metadata: Some(image_metadata()),
            container: None,
        })
    );
    assert_eq!(
        asset
            .original_filename
            .to_os_string()
            .as_os_str()
            .as_bytes(),
        b"source-graphic.svg"
    );
    let db = raw_db(directory.as_path());
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
