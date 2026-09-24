use pigoune_core::{
    AssetId, AssetRecord, ContainerCodec, ContainerMetadata, ContainerRepresentation,
    DatabaseError, ImageFormat, ImageMetadata, Library, LibraryDatabase, LibraryId, ObjectHash,
    ObjectRecord, OriginalFilename,
};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn make_object(bytes: &[u8]) -> ObjectRecord {
    let hash = ObjectHash::from_bytes(bytes);
    let text = hash.to_string();
    ObjectRecord {
        hash,
        size: bytes.len() as u64,
        relative_path: PathBuf::from(format!("objects/{}/{}", &text[..2], text)),
    }
}

fn asset(object: &ObjectRecord) -> AssetRecord {
    AssetRecord {
        id: AssetId::new(),
        object_hash: object.hash,
        original_filename: OriginalFilename::from_bytes(b"icon.ico".to_vec()).unwrap(),
        display_name: "Icon".into(),
        imported_at_utc_us: 42,
    }
}

fn metadata(format: ImageFormat) -> ImageMetadata {
    ImageMetadata::new(format, 32, 32, false).unwrap()
}

fn inventory(format: ImageFormat) -> ContainerMetadata {
    let (codec, scale) = match format {
        ImageFormat::Ico => (ContainerCodec::Dib, None),
        ImageFormat::Icns => (ContainerCodec::IcnsRgb, Some(1)),
        _ => unreachable!(),
    };
    ContainerMetadata::new(
        vec![
            ContainerRepresentation::new(0, 16, 16, Some(32), codec, 1024, scale).unwrap(),
            ContainerRepresentation::new(3, 32, 32, None, ContainerCodec::Png, 2048, scale)
                .unwrap(),
        ],
        3,
    )
    .unwrap()
}

fn raw(root: &Path) -> Connection {
    let db = Connection::open(root.join("library.db")).unwrap();
    db.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    db
}

fn insert_v2_object(
    db: &Connection,
    object: &ObjectRecord,
    format: ImageFormat,
    asset: &AssetRecord,
) {
    db.execute("INSERT INTO objects (hash, size_bytes, relative_path, format, width, height, animated) VALUES (?1, ?2, ?3, ?4, 32, 32, 0)",
        params![object.hash.digest_bytes().as_slice(), object.size as i64,
            object.relative_path.to_str().unwrap(), format.as_str()]).unwrap();
    db.execute("INSERT INTO assets (id, object_hash, original_filename, display_name, imported_at_utc_us) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![asset.id.to_bytes().as_slice(), object.hash.digest_bytes().as_slice(),
            asset.original_filename.as_bytes(), asset.display_name, asset.imported_at_utc_us]).unwrap();
}

#[test]
fn v2_migration_preserves_objects_assets_and_legacy_inventories() {
    let root = tempdir().unwrap();
    let id = LibraryId::new();
    let png = make_object(b"png");
    let ico = make_object(b"ico");
    let icns = make_object(b"icns");
    let assets = [asset(&png), asset(&ico), asset(&icns)];
    let db = raw(root.path());
    db.execute_batch(include_str!("../src/migrations/0001_initial.sql"))
        .unwrap();
    db.execute(
        "INSERT INTO library_metadata VALUES (1, ?1)",
        [id.to_bytes().as_slice()],
    )
    .unwrap();
    db.execute_batch(include_str!("../src/migrations/0002_image_metadata.sql"))
        .unwrap();
    insert_v2_object(&db, &png, ImageFormat::Png, &assets[0]);
    insert_v2_object(&db, &ico, ImageFormat::Ico, &assets[1]);
    insert_v2_object(&db, &icns, ImageFormat::Icns, &assets[2]);
    db.pragma_update(None, "user_version", 2).unwrap();
    db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL")
        .unwrap();
    drop(db);

    let mut database = LibraryDatabase::open(root.path(), id).unwrap();
    assert_eq!(database.schema_version().unwrap(), 3);
    assert_eq!(database.journal_mode().unwrap(), "wal");
    assert!(database.foreign_keys_enabled().unwrap());
    for (object, asset) in [(&png, &assets[0]), (&ico, &assets[1]), (&icns, &assets[2])] {
        let stored = database.get_object(object.hash).unwrap().unwrap();
        assert_eq!(stored.object, *object);
        assert_eq!(stored.container, None);
        assert_eq!(database.get_asset(asset.id).unwrap(), Some(asset.clone()));
    }
    let db = raw(root.path());
    assert_eq!(
        db.query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
            .optional()
            .unwrap(),
        None
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM object_representations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(db);

    for (object, format, old_asset) in [
        (&ico, ImageFormat::Ico, &assets[1]),
        (&icns, ImageFormat::Icns, &assets[2]),
    ] {
        let next = asset(object);
        let inventory = inventory(format);
        let mut failed = asset(object);
        failed.id = old_asset.id;
        assert!(
            database
                .import_published_asset(object, metadata(format), Some(&inventory), &failed)
                .is_err()
        );
        assert_eq!(
            database.get_object(object.hash).unwrap().unwrap().container,
            None
        );
        database
            .import_published_asset(object, metadata(format), Some(&inventory), &next)
            .unwrap();
        assert_eq!(
            database.get_object(object.hash).unwrap().unwrap().container,
            Some(inventory)
        );
        assert_eq!(
            database.get_asset(old_asset.id).unwrap(),
            Some(old_asset.clone())
        );
        assert_eq!(database.get_asset(next.id).unwrap(), Some(next));
    }
}

#[test]
fn inventory_reconciliation_and_atomic_rollback() {
    let root = tempdir().unwrap();
    let mut library = Library::create(&root.path().join("library")).unwrap();
    let object = make_object(b"same icon");
    let inventory = inventory(ImageFormat::Ico);
    let first = asset(&object);
    library
        .database
        .import_published_asset(
            &object,
            metadata(ImageFormat::Ico),
            Some(&inventory),
            &first,
        )
        .unwrap();
    let second = asset(&object);
    library
        .database
        .import_published_asset(
            &object,
            metadata(ImageFormat::Ico),
            Some(&inventory),
            &second,
        )
        .unwrap();
    let changed = ContainerMetadata::new(inventory.representations().to_vec(), 0).unwrap();
    let rejected = asset(&object);
    assert!(matches!(
        library.database.import_published_asset(
            &object,
            metadata(ImageFormat::Ico),
            Some(&changed),
            &rejected
        ),
        Err(DatabaseError::ContainerMetadataConflict(_))
    ));
    assert!(library.database.get_asset(rejected.id).unwrap().is_none());
    assert_eq!(
        library
            .database
            .get_object(object.hash)
            .unwrap()
            .unwrap()
            .container,
        Some(inventory.clone())
    );

    let new_object = make_object(b"new icon");
    let mut duplicate_id = asset(&new_object);
    duplicate_id.id = first.id;
    assert!(
        library
            .database
            .import_published_asset(
                &new_object,
                metadata(ImageFormat::Ico),
                Some(&inventory),
                &duplicate_id
            )
            .is_err()
    );
    assert!(
        library
            .database
            .get_object(new_object.hash)
            .unwrap()
            .is_none()
    );
    let db = raw(&root.path().join("library"));
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM object_representations WHERE object_hash=?1",
            [new_object.hash.digest_bytes().as_slice()],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn reader_preserves_persisted_primary_without_reselection() {
    let root = tempdir().unwrap();
    let mut library = Library::create(&root.path().join("library")).unwrap();
    let object = make_object(b"explicit primary");
    let base = inventory(ImageFormat::Ico);
    let selected = ContainerMetadata::new(base.representations().to_vec(), 0).unwrap();
    library
        .database
        .import_published_asset(
            &object,
            metadata(ImageFormat::Ico),
            Some(&selected),
            &asset(&object),
        )
        .unwrap();
    assert_eq!(
        library
            .database
            .get_object(object.hash)
            .unwrap()
            .unwrap()
            .container
            .unwrap()
            .primary_ordinal(),
        0
    );
}

#[test]
fn format_container_rules_are_enforced_on_write_and_read() {
    let root = tempdir().unwrap();
    let mut library = Library::create(&root.path().join("library")).unwrap();
    let object = make_object(b"format rules");
    let ico = inventory(ImageFormat::Ico);
    for (format, container) in [
        (ImageFormat::Ico, None),
        (ImageFormat::Icns, None),
        (ImageFormat::Png, Some(&ico)),
        (ImageFormat::Icns, Some(&ico)),
    ] {
        assert!(matches!(
            library.database.import_published_asset(
                &object,
                metadata(format),
                container,
                &asset(&object)
            ),
            Err(DatabaseError::InvalidContainerMetadata(_))
        ));
    }
    library
        .database
        .import_published_asset(
            &object,
            metadata(ImageFormat::Ico),
            Some(&ico),
            &asset(&object),
        )
        .unwrap();
    let db = raw(&root.path().join("library"));
    db.execute(
        "UPDATE objects SET format='png' WHERE hash=?1",
        [object.hash.digest_bytes().as_slice()],
    )
    .unwrap();
    assert!(matches!(
        library.database.get_object(object.hash),
        Err(DatabaseError::InvalidStoredValue(
            "container format mismatch"
        ))
    ));
}

#[test]
fn sql_constraints_and_unknown_codec_behavior() {
    let root = tempdir().unwrap();
    let library = Library::create(&root.path().join("library")).unwrap();
    let object = make_object(b"raw SQL rows");
    let db = raw(&root.path().join("library"));
    db.execute("INSERT INTO objects (hash, size_bytes, relative_path, format, width, height, animated) VALUES (?1, ?2, ?3, 'ico', 32, 32, 0)",
        params![object.hash.digest_bytes().as_slice(), object.size as i64, object.relative_path.to_str().unwrap()]).unwrap();
    let sql = "INSERT INTO object_representations (object_hash, ordinal, width, height, scale, bit_depth, codec, encoded_size, is_primary) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";
    let hash = object.hash.digest_bytes();
    let insert = |hash: &[u8],
                  ordinal: i64,
                  width: i64,
                  height: i64,
                  scale: Option<i64>,
                  depth: Option<i64>,
                  codec: &str,
                  size: i64,
                  primary: i64| {
        db.execute(
            sql,
            params![
                hash, ordinal, width, height, scale, depth, codec, size, primary
            ],
        )
    };
    assert!(insert(&hash, 0, 16, 16, None, None, "png", 100, 1).is_ok());
    for (h, ordinal, width, height, scale, depth, codec, size, primary) in [
        (hash[..31].to_vec(), 1, 16, 16, None, None, "png", 100, 0),
        (hash.to_vec(), -1, 16, 16, None, None, "png", 100, 0),
        (hash.to_vec(), 65536, 16, 16, None, None, "png", 100, 0),
        (hash.to_vec(), 1, 0, 16, None, None, "png", 100, 0),
        (hash.to_vec(), 1, 16, 0, None, None, "png", 100, 0),
        (hash.to_vec(), 1, 16, 16, Some(0), None, "png", 100, 0),
        (hash.to_vec(), 1, 16, 16, None, Some(0), "png", 100, 0),
        (hash.to_vec(), 1, 16, 16, None, None, "", 100, 0),
        (hash.to_vec(), 1, 16, 16, None, None, "png", 0, 0),
        (hash.to_vec(), 1, 16, 16, None, None, "png", 100, -1),
        (hash.to_vec(), 1, 16, 16, None, None, "png", 100, 2),
        (hash.to_vec(), 0, 16, 16, None, None, "png", 100, 0),
        (hash.to_vec(), 1, 16, 16, None, None, "png", 100, 1),
        (vec![1; 32], 1, 16, 16, None, None, "png", 100, 0),
    ] {
        assert!(
            insert(
                &h, ordinal, width, height, scale, depth, codec, size, primary
            )
            .is_err(),
            "accepted invalid row: {ordinal}, {codec}"
        );
    }
    assert!(insert(&hash, 3, 32, 32, None, None, "future-codec", 200, 0).is_ok());
    assert!(matches!(
        library.database.get_object(object.hash),
        Err(DatabaseError::InvalidStoredValue("container codec"))
    ));
    db.execute("DELETE FROM object_representations WHERE ordinal=3", [])
        .unwrap();
    db.execute(
        "UPDATE object_representations SET is_primary=0 WHERE ordinal=0",
        [],
    )
    .unwrap();
    assert!(matches!(
        library.database.get_object(object.hash),
        Err(DatabaseError::InvalidStoredValue(
            "missing primary representation"
        ))
    ));
}
