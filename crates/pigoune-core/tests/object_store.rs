use pigoune_core::{ObjectHash, ObjectStore, StoreError};
use std::{fs, path::Path};
use tempfile::tempdir;

fn object_count(root: &Path) -> usize {
    fs::read_dir(root.join("objects"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().unwrap().is_dir())
        .filter(|entry| entry.file_name() != ".tmp")
        .map(|shard| fs::read_dir(shard.path()).unwrap().count())
        .sum()
}

#[test]
fn preserves_bytes_and_source_and_uses_relative_sharded_path() {
    let library = tempdir().unwrap();
    let source_dir = tempdir().unwrap();
    let source = source_dir.path().join("graphic.SVG");
    let bytes = [0, 255, 10, 0, 42, 128];
    fs::write(&source, bytes).unwrap();
    let store = ObjectStore::new(library.path()).unwrap();

    let result = store.store_file(&source).unwrap();
    let hash = ObjectHash::from_bytes(&bytes);
    let hash_text = hash.to_string();
    assert_eq!(result.object.hash, hash);
    assert_eq!(result.object.size, bytes.len() as u64);
    assert_eq!(
        result.object.relative_path,
        Path::new("objects")
            .join(&hash_text[..2])
            .join(format!("{hash_text}.svg"))
    );
    assert!(!result.object.relative_path.is_absolute());
    assert_eq!(
        fs::read(library.path().join(&result.object.relative_path)).unwrap(),
        bytes
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_ne!(
            fs::metadata(&source).unwrap().ino(),
            fs::metadata(library.path().join(&result.object.relative_path))
                .unwrap()
                .ino()
        );
    }
    fs::remove_file(&source).unwrap();
    assert_eq!(
        fs::read(library.path().join(&result.object.relative_path)).unwrap(),
        bytes
    );
    assert_eq!(store.find(hash).unwrap(), Some(result.object));
    assert!(store.contains(hash).unwrap());
}

#[test]
fn deduplicates_across_extensions_and_keeps_first_name() {
    let library = tempdir().unwrap();
    let source_dir = tempdir().unwrap();
    let first_source = source_dir.path().join("first.PNG");
    let second_source = source_dir.path().join("second.svg");
    fs::write(&first_source, b"same bytes").unwrap();
    fs::write(&second_source, b"same bytes").unwrap();
    let store = ObjectStore::new(library.path()).unwrap();

    let first = store.store_file(&first_source).unwrap();
    let second = store.store_file(&second_source).unwrap();
    let third = store.store_file(&first_source).unwrap();
    assert!(!first.reused);
    assert!(second.reused && third.reused);
    assert_eq!(first.object, second.object);
    assert_eq!(first.object, third.object);
    assert_eq!(first.object.relative_path.extension().unwrap(), "png");
    assert_eq!(object_count(library.path()), 1);
    assert_eq!(
        fs::read_dir(library.path().join("objects/.tmp"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn different_bytes_create_different_objects() {
    let library = tempdir().unwrap();
    let source_dir = tempdir().unwrap();
    let a = source_dir.path().join("a.svg");
    let b = source_dir.path().join("b.svg");
    fs::write(&a, b"one").unwrap();
    fs::write(&b, b"two").unwrap();
    let store = ObjectStore::new(library.path()).unwrap();
    let first = store.store_file(a).unwrap();
    let second = store.store_file(b).unwrap();
    assert_ne!(first.object.hash, second.object.hash);
    assert_eq!(object_count(library.path()), 2);
}

#[test]
fn unsafe_extension_is_omitted() {
    let library = tempdir().unwrap();
    let source_dir = tempdir().unwrap();
    let source = source_dir.path().join("asset.bad-ext");
    fs::write(&source, b"safe").unwrap();
    let store = ObjectStore::new(library.path()).unwrap();
    let result = store.store_file(source).unwrap();
    assert_eq!(
        result
            .object
            .relative_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
        result.object.hash.to_string()
    );
    assert_eq!(result.object.relative_path.components().count(), 3);
    assert!(
        library
            .path()
            .join(&result.object.relative_path)
            .starts_with(library.path().join("objects"))
    );
}

#[test]
fn streams_a_multi_megabyte_file() {
    let library = tempdir().unwrap();
    let source_dir = tempdir().unwrap();
    let source = source_dir.path().join("large.bin");
    let bytes: Vec<u8> = (0..2_000_000).map(|n| (n % 251) as u8).collect();
    fs::write(&source, &bytes).unwrap();
    let store = ObjectStore::new(library.path()).unwrap();
    let result = store.store_file(source).unwrap();
    assert_eq!(result.object.hash, ObjectHash::from_bytes(&bytes));
    assert_eq!(
        fs::read(library.path().join(result.object.relative_path)).unwrap(),
        bytes
    );
}

#[test]
fn abandoned_temporary_file_is_not_an_object() {
    let library = tempdir().unwrap();
    let store = ObjectStore::new(library.path()).unwrap();
    fs::write(library.path().join("objects/.tmp/abandoned"), b"incomplete").unwrap();
    assert!(
        !store
            .contains(ObjectHash::from_bytes(b"incomplete"))
            .unwrap()
    );
    assert_eq!(object_count(library.path()), 0);
}

#[cfg(unix)]
#[test]
fn failed_read_removes_its_temporary_file() {
    let library = tempdir().unwrap();
    let source_dir = tempdir().unwrap();
    let store = ObjectStore::new(library.path()).unwrap();
    assert!(matches!(
        store.store_file(source_dir.path()),
        Err(StoreError::Io(_))
    ));
    assert_eq!(
        fs::read_dir(library.path().join("objects/.tmp"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(object_count(library.path()), 0);
}

#[test]
fn rejects_corrupt_existing_object() {
    let library = tempdir().unwrap();
    let source_dir = tempdir().unwrap();
    let source = source_dir.path().join("asset.svg");
    fs::write(&source, b"original").unwrap();
    let store = ObjectStore::new(library.path()).unwrap();
    let object = store.store_file(&source).unwrap().object;
    let stored_path = library.path().join(&object.relative_path);
    fs::remove_file(&stored_path).unwrap();
    fs::write(stored_path, b"changed").unwrap();
    assert!(matches!(
        store.store_file(source),
        Err(StoreError::InconsistentStore(_))
    ));
}

#[test]
fn concurrent_same_bytes_have_one_physical_object() {
    let library = tempdir().unwrap();
    let source_dir = tempdir().unwrap();
    let a = source_dir.path().join("a.png");
    let b = source_dir.path().join("b.svg");
    fs::write(&a, b"shared").unwrap();
    fs::write(&b, b"shared").unwrap();
    let root_a = library.path().to_path_buf();
    let root_b = root_a.clone();
    let first =
        std::thread::spawn(move || ObjectStore::new(root_a).unwrap().store_file(a).unwrap());
    let second =
        std::thread::spawn(move || ObjectStore::new(root_b).unwrap().store_file(b).unwrap());
    let first = first.join().unwrap();
    let second = second.join().unwrap();
    assert_eq!(first.object, second.object);
    assert_ne!(first.reused, second.reused);
    assert_eq!(object_count(library.path()), 1);
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_shard() {
    use std::os::unix::fs::symlink;
    let library = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let source_dir = tempdir().unwrap();
    let source = source_dir.path().join("asset.svg");
    fs::write(&source, b"link test").unwrap();
    let hash = ObjectHash::from_bytes(b"link test").to_string();
    let store = ObjectStore::new(library.path()).unwrap();
    symlink(
        outside.path(),
        library.path().join("objects").join(&hash[..2]),
    )
    .unwrap();
    assert!(matches!(
        store.store_file(source),
        Err(StoreError::InvalidInternalPath(_))
    ));
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_lock_file() {
    use std::os::unix::fs::symlink;
    let library = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::create_dir(library.path().join("objects")).unwrap();
    let outside_file = outside.path().join("untouched");
    fs::write(&outside_file, b"original").unwrap();
    symlink(&outside_file, library.path().join("objects/.lock")).unwrap();
    assert!(matches!(
        ObjectStore::new(library.path()),
        Err(StoreError::InvalidInternalPath(_))
    ));
    assert_eq!(fs::read(outside_file).unwrap(), b"original");
}
