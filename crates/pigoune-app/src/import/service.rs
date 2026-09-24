use std::{
    error::Error,
    fmt,
    path::Path,
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

use pigoune_core::{
    AssetId, AssetRecord, DatabaseError, ImageMetadata, Library, ObjectHash, OriginalFilename,
    OriginalFilenameError, StoreError, StoreResult,
};

use super::{GlycinValidator, ImportValidationError, ImportWarning};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DuplicatePolicy {
    Detect,
    ImportAnyway,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ImportOutcome {
    Imported {
        asset: AssetRecord,
        object: StoreResult,
        metadata: ImageMetadata,
        warnings: Vec<ImportWarning>,
    },
    /// Validation warnings are returned even though no asset was created.
    Duplicate {
        hash: ObjectHash,
        existing_asset_ids: Vec<AssetId>,
        warnings: Vec<ImportWarning>,
    },
}

#[derive(Debug)]
pub enum ImportError {
    SourceWithoutFilename,
    InvalidOriginalFilename(OriginalFilenameError),
    EmptyDisplayName,
    Staging(StoreError),
    Validation(ImportValidationError),
    Database(DatabaseError),
    Publication(StoreError),
    ClockBeforeEpoch(SystemTimeError),
    ClockOutOfRange,
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceWithoutFilename => f.write_str("source has no usable filename"),
            Self::InvalidOriginalFilename(error) => write!(f, "invalid original filename: {error}"),
            Self::EmptyDisplayName => f.write_str("asset display name is empty"),
            Self::Staging(error) => write!(f, "cannot stage source: {error}"),
            Self::Validation(error) => write!(f, "image validation failed: {error}"),
            Self::Database(error) => write!(f, "database import failed: {error}"),
            Self::Publication(error) => write!(f, "object publication failed: {error}"),
            Self::ClockBeforeEpoch(error) => {
                write!(f, "import instant is before Unix epoch: {error}")
            }
            Self::ClockOutOfRange => f.write_str("import instant exceeds SQLite timestamp range"),
        }
    }
}

impl Error for ImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidOriginalFilename(error) => Some(error),
            Self::Staging(error) | Self::Publication(error) => Some(error),
            Self::Validation(error) => Some(error),
            Self::Database(error) => Some(error),
            Self::ClockBeforeEpoch(error) => Some(error),
            Self::SourceWithoutFilename | Self::EmptyDisplayName | Self::ClockOutOfRange => None,
        }
    }
}

/// Synchronous import entry point. Run it on a worker, never on the GTK main thread.
/// The library database is currently used as a serialized writer.
pub struct ImportService;

impl ImportService {
    pub fn import_file(
        library: &mut Library,
        source: &Path,
        display_name: &str,
        policy: DuplicatePolicy,
    ) -> Result<ImportOutcome, ImportError> {
        Self::import_file_with_clock(library, source, display_name, policy, SystemTime::now)
    }

    fn import_file_with_clock(
        library: &mut Library,
        source: &Path,
        display_name: &str,
        policy: DuplicatePolicy,
        now: impl FnOnce() -> SystemTime,
    ) -> Result<ImportOutcome, ImportError> {
        if display_name.is_empty() {
            return Err(ImportError::EmptyDisplayName);
        }
        let filename = source
            .file_name()
            .ok_or(ImportError::SourceWithoutFilename)?;
        let original_filename = OriginalFilename::from_os_str(filename)
            .map_err(ImportError::InvalidOriginalFilename)?;

        let staged = library
            .object_store
            .stage_file(source)
            .map_err(ImportError::Staging)?;
        let hash = staged.hash();
        let validated = staged
            .validate_with(&GlycinValidator)
            .map_err(ImportError::Validation)?;
        if policy == DuplicatePolicy::Detect {
            let existing_asset_ids = library
                .database
                .asset_ids_for_object(hash)
                .map_err(ImportError::Database)?;
            if !existing_asset_ids.is_empty() {
                return Ok(ImportOutcome::Duplicate {
                    hash,
                    existing_asset_ids,
                    warnings: validated.validation().warnings.clone(),
                });
            }
        }

        let published = validated.publish().map_err(ImportError::Publication)?;
        let imported_at_utc_us = timestamp_us(now())?;
        let asset = AssetRecord {
            id: AssetId::new(),
            object_hash: published.stored.object.hash,
            original_filename,
            display_name: display_name.to_owned(),
            imported_at_utc_us,
        };
        // A failed transaction rolls back its rows. The published object may remain
        // orphaned; removing it here could race with another asset using the same bytes.
        library
            .database
            .import_published_asset(
                &published.stored.object,
                published.validation.metadata,
                &asset,
            )
            .map_err(ImportError::Database)?;
        Ok(ImportOutcome::Imported {
            asset,
            object: published.stored,
            metadata: published.validation.metadata,
            warnings: published.validation.warnings,
        })
    }
}

fn timestamp_us(instant: SystemTime) -> Result<i64, ImportError> {
    let duration = instant
        .duration_since(UNIX_EPOCH)
        .map_err(ImportError::ClockBeforeEpoch)?;
    i64::try_from(duration.as_micros()).map_err(|_| ImportError::ClockOutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pigoune_core::ImageFormat;
    use std::{fs, os::unix::ffi::OsStrExt, path::PathBuf, time::Duration};
    use tempfile::tempdir;

    const PNG: &[u8] = include_bytes!("../../tests/fixtures/sample.png");
    const SVG_EXTERNAL: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="3" height="2"><rect width="3" height="2" fill="#315b8f"/><image href="missing.png" width="1" height="1"/></svg>"##;

    fn source(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn import(library: &mut Library, source: &Path, policy: DuplicatePolicy) -> ImportOutcome {
        ImportService::import_file(library, source, "Example", policy).unwrap()
    }

    fn imported(
        outcome: ImportOutcome,
    ) -> (AssetRecord, StoreResult, ImageMetadata, Vec<ImportWarning>) {
        match outcome {
            ImportOutcome::Imported {
                asset,
                object,
                metadata,
                warnings,
            } => (asset, object, metadata, warnings),
            other => panic!("expected Imported, got {other:?}"),
        }
    }

    fn tmp_count(root: &Path) -> usize {
        fs::read_dir(root.join("objects/.tmp")).unwrap().count()
    }

    #[test]
    fn normal_png_import_preserves_bytes_and_survives_source_removal() {
        let root = tempdir().unwrap();
        let source = source(root.path(), "sample.png", PNG);
        let mut library = Library::create(&root.path().join("library")).unwrap();
        let (asset, object, metadata, warnings) =
            imported(import(&mut library, &source, DuplicatePolicy::Detect));
        assert_eq!(metadata.format(), ImageFormat::Png);
        assert_eq!((metadata.width(), metadata.height()), (3, 2));
        assert!(warnings.is_empty());
        assert!(!object.reused);
        assert_eq!(object.object.hash, ObjectHash::from_bytes(PNG));
        assert_eq!(
            library.database.get_asset(asset.id).unwrap(),
            Some(asset.clone())
        );
        assert_eq!(
            library
                .database
                .get_object(object.object.hash)
                .unwrap()
                .unwrap()
                .metadata,
            Some(metadata)
        );
        fs::remove_file(&source).unwrap();
        assert_eq!(
            fs::read(
                root.path()
                    .join("library")
                    .join(&object.object.relative_path)
            )
            .unwrap(),
            PNG
        );
        assert_eq!(library.database.get_asset(asset.id).unwrap(), Some(asset));
        assert_eq!(tmp_count(&root.path().join("library")), 0);
    }

    #[test]
    fn misleading_extension_and_non_utf8_names_round_trip() {
        let root = tempdir().unwrap();
        let mut library = Library::create(&root.path().join("library")).unwrap();
        let source = source(root.path(), "picture.jpg", PNG);
        let (asset, _, metadata, _) =
            imported(import(&mut library, &source, DuplicatePolicy::Detect));
        assert_eq!(asset.original_filename.as_bytes(), b"picture.jpg");
        assert_eq!(metadata.format(), ImageFormat::Png);

        let name = std::ffi::OsStr::from_bytes(b"non-utf8-\xff.png");
        let source = root.path().join(name);
        fs::write(&source, PNG).unwrap();
        let (asset, _, _, _) =
            imported(import(&mut library, &source, DuplicatePolicy::ImportAnyway));
        assert_eq!(asset.original_filename.as_bytes(), name.as_bytes());
        assert_eq!(
            library
                .database
                .get_asset(asset.id)
                .unwrap()
                .unwrap()
                .original_filename
                .as_bytes(),
            name.as_bytes()
        );
    }

    #[test]
    fn external_svg_warning_and_exact_bytes_are_returned() {
        let root = tempdir().unwrap();
        let mut library = Library::create(&root.path().join("library")).unwrap();
        let source = source(root.path(), "external.svg", SVG_EXTERNAL);
        let (asset, object, metadata, warnings) =
            imported(import(&mut library, &source, DuplicatePolicy::Detect));
        assert_eq!(metadata.format(), ImageFormat::Svg);
        assert_eq!(warnings, vec![ImportWarning::SvgExternalReferences]);
        assert_eq!(
            fs::read(
                root.path()
                    .join("library")
                    .join(object.object.relative_path)
            )
            .unwrap(),
            SVG_EXTERNAL
        );
        assert_eq!(
            import(&mut library, &source, DuplicatePolicy::Detect),
            ImportOutcome::Duplicate {
                hash: ObjectHash::from_bytes(SVG_EXTERNAL),
                existing_asset_ids: vec![asset.id],
                warnings: vec![ImportWarning::SvgExternalReferences],
            }
        );
    }

    #[test]
    fn invalid_content_and_empty_name_leave_no_staging_or_database_rows() {
        let root = tempdir().unwrap();
        let mut library = Library::create(&root.path().join("library")).unwrap();
        let fake = source(root.path(), "fake.png", b"not an image");
        assert!(matches!(
            ImportService::import_file(&mut library, &fake, "Example", DuplicatePolicy::Detect),
            Err(ImportError::Validation(_))
        ));
        assert_eq!(tmp_count(&root.path().join("library")), 0);
        assert!(
            !library
                .object_store
                .contains(ObjectHash::from_bytes(b"not an image"))
                .unwrap()
        );
        assert!(
            library
                .database
                .asset_ids_for_object(ObjectHash::from_bytes(b"not an image"))
                .unwrap()
                .is_empty()
        );

        let valid = source(root.path(), "valid.png", PNG);
        assert!(matches!(
            ImportService::import_file(&mut library, &valid, "", DuplicatePolicy::Detect),
            Err(ImportError::EmptyDisplayName)
        ));
        assert_eq!(tmp_count(&root.path().join("library")), 0);
        assert!(
            !library
                .object_store
                .contains(ObjectHash::from_bytes(PNG))
                .unwrap()
        );
        assert!(
            library
                .database
                .asset_ids_for_object(ObjectHash::from_bytes(PNG))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn detect_stops_before_publication_and_import_anyway_reuses_physical_bytes() {
        let root = tempdir().unwrap();
        let mut library = Library::create(&root.path().join("library")).unwrap();
        let source = source(root.path(), "first.png", PNG);
        let (first, first_object, _, _) =
            imported(import(&mut library, &source, DuplicatePolicy::Detect));
        let duplicate = import(&mut library, &source, DuplicatePolicy::Detect);
        assert_eq!(
            duplicate,
            ImportOutcome::Duplicate {
                hash: first.object_hash,
                existing_asset_ids: vec![first.id],
                warnings: vec![]
            }
        );
        assert_eq!(tmp_count(&root.path().join("library")), 0);
        assert_eq!(
            library
                .database
                .asset_ids_for_object(first.object_hash)
                .unwrap(),
            vec![first.id]
        );
        assert_eq!(
            library.object_store.find(first.object_hash).unwrap(),
            Some(first_object.object.clone())
        );

        let (second, second_object, _, _) =
            imported(import(&mut library, &source, DuplicatePolicy::ImportAnyway));
        assert_ne!(first.id, second.id);
        assert_eq!(first.object_hash, second.object_hash);
        assert!(second_object.reused);
        assert_eq!(second_object.object, first_object.object);
        assert_eq!(
            library
                .database
                .asset_ids_for_object(first.object_hash)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn orphan_physical_object_is_not_a_logical_duplicate() {
        let root = tempdir().unwrap();
        let mut library = Library::create(&root.path().join("library")).unwrap();
        let source = source(root.path(), "orphan.png", PNG);
        let orphan = library
            .object_store
            .stage_file(&source)
            .unwrap()
            .validate_with(&GlycinValidator)
            .unwrap()
            .publish()
            .unwrap();
        assert!(
            library
                .database
                .asset_ids_for_object(orphan.stored.object.hash)
                .unwrap()
                .is_empty()
        );
        let (asset, stored, _, _) =
            imported(import(&mut library, &source, DuplicatePolicy::Detect));
        assert!(stored.reused);
        assert_eq!(stored.object, orphan.stored.object);
        assert_eq!(
            library
                .database
                .asset_ids_for_object(asset.object_hash)
                .unwrap(),
            vec![asset.id]
        );
    }

    #[test]
    fn source_without_filename_and_invalid_name_fail_before_staging() {
        let root = tempdir().unwrap();
        let mut library = Library::create(&root.path().join("library")).unwrap();
        assert!(matches!(
            ImportService::import_file(
                &mut library,
                Path::new("/"),
                "Example",
                DuplicatePolicy::Detect
            ),
            Err(ImportError::SourceWithoutFilename)
        ));
        let invalid = Path::new(std::ffi::OsStr::from_bytes(b"bad\0name"));
        assert!(matches!(
            ImportService::import_file(&mut library, invalid, "Example", DuplicatePolicy::Detect),
            Err(ImportError::InvalidOriginalFilename(_))
        ));
        assert_eq!(tmp_count(&root.path().join("library")), 0);
    }

    #[test]
    fn injected_clock_sets_exact_microseconds_and_rejects_invalid_instants() {
        let root = tempdir().unwrap();
        let mut library = Library::create(&root.path().join("library")).unwrap();
        let source = source(root.path(), "clock.png", PNG);
        let instant =
            UNIX_EPOCH + Duration::from_secs(1_700_000_000) + Duration::from_micros(123_456);
        let (asset, _, _, _) = imported(
            ImportService::import_file_with_clock(
                &mut library,
                &source,
                "Clock",
                DuplicatePolicy::Detect,
                || instant,
            )
            .unwrap(),
        );
        assert_eq!(asset.imported_at_utc_us, 1_700_000_000_123_456);
        assert_eq!(
            library
                .database
                .get_asset(asset.id)
                .unwrap()
                .unwrap()
                .imported_at_utc_us,
            asset.imported_at_utc_us
        );
        assert!(matches!(
            timestamp_us(UNIX_EPOCH - Duration::from_micros(1)),
            Err(ImportError::ClockBeforeEpoch(_))
        ));
        assert!(matches!(
            timestamp_us(UNIX_EPOCH + Duration::from_secs(i64::MAX as u64)),
            Err(ImportError::ClockOutOfRange)
        ));
        assert!(matches!(
            ImportService::import_file_with_clock(
                &mut library,
                &source,
                "Before epoch",
                DuplicatePolicy::ImportAnyway,
                || UNIX_EPOCH - Duration::from_micros(1),
            ),
            Err(ImportError::ClockBeforeEpoch(_))
        ));
        assert!(matches!(
            ImportService::import_file_with_clock(
                &mut library,
                &source,
                "Overflow",
                DuplicatePolicy::ImportAnyway,
                || UNIX_EPOCH + Duration::from_secs(i64::MAX as u64),
            ),
            Err(ImportError::ClockOutOfRange)
        ));
        assert_eq!(
            library
                .database
                .asset_ids_for_object(asset.object_hash)
                .unwrap(),
            vec![asset.id]
        );
    }
}
