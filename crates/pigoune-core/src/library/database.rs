mod error;
mod migration;

pub use error::DatabaseError;

use super::{DATABASE_SCHEMA_VERSION, LibraryId, OriginalFilename, path::valid_object_path};
use crate::{
    AssetId, ContainerCodec, ContainerMetadata, ContainerRepresentation, ImageFormat,
    ImageMetadata, ObjectHash, ObjectRecord,
};
use migration::{
    check_sqlite_version, configure_writer, migrate, read_library_id, schema_version,
    verify_library_id,
};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
};

type AssetRow = (Vec<u8>, Vec<u8>, Vec<u8>, String, i64);
type ObjectRow = (
    Vec<u8>,
    i64,
    String,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredObject {
    pub object: ObjectRecord,
    pub metadata: Option<ImageMetadata>,
    pub container: Option<ContainerMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRecord {
    pub id: AssetId,
    pub object_hash: ObjectHash,
    pub original_filename: OriginalFilename,
    pub display_name: String,
    pub imported_at_utc_us: i64,
}

pub struct LibraryDatabase {
    connection: Connection,
}

impl LibraryDatabase {
    pub(crate) fn close(self) -> Result<(), DatabaseError> {
        self.connection.close().map_err(|(_, error)| error.into())
    }

    pub fn create(root: &Path, library_id: LibraryId) -> Result<Self, DatabaseError> {
        let path = root.join("library.db");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(DatabaseError::Io)?;
        let mut connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        check_sqlite_version()?;
        configure_writer(&connection)?;
        migrate(&mut connection, library_id)?;
        Ok(Self { connection })
    }

    pub fn open(root: &Path, expected_id: LibraryId) -> Result<Self, DatabaseError> {
        let path = root.join("library.db");
        let mut connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        check_sqlite_version()?;
        let version = schema_version(&connection)?;
        if version > DATABASE_SCHEMA_VERSION {
            return Err(DatabaseError::SchemaTooNew(version));
        }
        if version == 0 {
            return Err(DatabaseError::UninitializedDatabase);
        }
        verify_library_id(&connection, expected_id)?;
        configure_writer(&connection)?;
        migrate(&mut connection, expected_id)?;
        Ok(Self { connection })
    }

    pub fn schema_version(&self) -> Result<i32, DatabaseError> {
        schema_version(&self.connection)
    }

    pub fn journal_mode(&self) -> Result<String, DatabaseError> {
        self.connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn foreign_keys_enabled(&self) -> Result<bool, DatabaseError> {
        let enabled: i32 = self
            .connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
        Ok(enabled == 1)
    }

    pub fn library_id(&self) -> Result<LibraryId, DatabaseError> {
        read_library_id(&self.connection)
    }

    pub fn get_object(&self, hash: ObjectHash) -> Result<Option<StoredObject>, DatabaseError> {
        let transaction = self.connection.unchecked_transaction()?;
        let row: Option<ObjectRow> = transaction
            .query_row(
                "SELECT hash, size_bytes, relative_path, format, width, height, animated \
                 FROM objects WHERE hash = ?1",
                [hash.digest_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()?;
        let stored = row
            .map(|row| {
                let mut stored = stored_object_from_row(row)?;
                stored.container = read_container(&transaction, hash, stored.metadata)?;
                Ok::<StoredObject, DatabaseError>(stored)
            })
            .transpose()?;
        transaction.commit()?;
        Ok(stored)
    }

    /// Records a published object and its validated intrinsic metadata with an asset.
    pub fn import_published_asset(
        &mut self,
        object: &ObjectRecord,
        metadata: ImageMetadata,
        container: Option<&ContainerMetadata>,
        asset: &AssetRecord,
    ) -> Result<(), DatabaseError> {
        validate_object(object)?;
        validate_asset(asset)?;
        if object.hash != asset.object_hash {
            return Err(DatabaseError::AssetObjectMismatch);
        }
        validate_container(metadata.format(), container, true)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        reconcile_object(&transaction, object, metadata, container)?;
        insert_asset(&transaction, asset)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_asset(&self, id: AssetId) -> Result<Option<AssetRecord>, DatabaseError> {
        let row: Option<AssetRow> = self
            .connection
            .query_row(
                "SELECT id, object_hash, original_filename, display_name, imported_at_utc_us \
                 FROM assets WHERE id = ?1",
                [id.to_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(id, hash, filename, display_name, imported_at_utc_us)| {
            let id = AssetId::from_bytes(array16(id, "asset ID")?);
            let object_hash = ObjectHash::from_digest_bytes(array32(hash, "object hash")?);
            let original_filename = OriginalFilename::from_bytes(filename)
                .map_err(DatabaseError::InvalidOriginalFilename)?;
            let asset = AssetRecord {
                id,
                object_hash,
                original_filename,
                display_name,
                imported_at_utc_us,
            };
            validate_asset(&asset)?;
            Ok(asset)
        })
        .transpose()
    }

    /// Returns logical assets referencing this hash, ordered by their UUID bytes.
    /// An object without assets is not a logical duplicate.
    pub fn asset_ids_for_object(&self, hash: ObjectHash) -> Result<Vec<AssetId>, DatabaseError> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM assets WHERE object_hash = ?1 ORDER BY id")?;
        let rows = statement.query_map([hash.digest_bytes().as_slice()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?;
        rows.map(|row| {
            let bytes = array16(row?, "asset ID")?;
            Ok(AssetId::from_bytes(bytes))
        })
        .collect()
    }
}

fn validate_object(object: &ObjectRecord) -> Result<(), DatabaseError> {
    if !valid_object_path(object.hash, &object.relative_path) {
        return Err(DatabaseError::InvalidObjectPath(
            object.relative_path.clone(),
        ));
    }
    i64::try_from(object.size).map_err(|_| DatabaseError::SizeOutOfRange(object.size))?;
    Ok(())
}

fn object_from_row(hash: Vec<u8>, size: i64, path: String) -> Result<ObjectRecord, DatabaseError> {
    let hash = ObjectHash::from_digest_bytes(array32(hash, "object hash")?);
    let size = u64::try_from(size).map_err(|_| DatabaseError::InvalidStoredValue("object size"))?;
    let object = ObjectRecord {
        hash,
        size,
        relative_path: PathBuf::from(path),
    };
    validate_object(&object)?;
    Ok(object)
}

fn stored_object_from_row(row: ObjectRow) -> Result<StoredObject, DatabaseError> {
    let (hash, size, path, format, width, height, animated) = row;
    let object = object_from_row(hash, size, path)?;
    let metadata = match (format, width, height, animated) {
        (None, None, None, None) => None,
        (Some(format), Some(width), Some(height), Some(animated)) => {
            let format = format
                .parse::<ImageFormat>()
                .map_err(|_| DatabaseError::InvalidStoredValue("image format"))?;
            let width = u32::try_from(width)
                .map_err(|_| DatabaseError::InvalidStoredValue("image width"))?;
            let height = u32::try_from(height)
                .map_err(|_| DatabaseError::InvalidStoredValue("image height"))?;
            let animated = match animated {
                0 => false,
                1 => true,
                _ => return Err(DatabaseError::InvalidStoredValue("image animation")),
            };
            Some(
                ImageMetadata::new(format, width, height, animated)
                    .map_err(|_| DatabaseError::InvalidStoredValue("image dimensions"))?,
            )
        }
        _ => return Err(DatabaseError::InvalidStoredValue("partial image metadata")),
    };
    Ok(StoredObject {
        object,
        metadata,
        container: None,
    })
}

fn validate_container(
    format: ImageFormat,
    container: Option<&ContainerMetadata>,
    normal_import: bool,
) -> Result<(), DatabaseError> {
    match (format, container) {
        (ImageFormat::Ico | ImageFormat::Icns, None) if normal_import => Err(
            DatabaseError::InvalidContainerMetadata("missing inventory for icon container"),
        ),
        (ImageFormat::Ico | ImageFormat::Icns, None) => Ok(()),
        (ImageFormat::Ico, Some(inventory)) => {
            if inventory.representations().iter().any(|item| {
                !matches!(item.codec(), ContainerCodec::Png | ContainerCodec::Dib)
                    || item.scale().is_some()
            }) {
                return Err(DatabaseError::InvalidContainerMetadata(
                    "ICO codec or scale",
                ));
            }
            Ok(())
        }
        (ImageFormat::Icns, Some(inventory)) => {
            if inventory.representations().iter().any(|item| {
                !matches!(
                    item.codec(),
                    ContainerCodec::Png
                        | ContainerCodec::Jpeg2000
                        | ContainerCodec::IcnsRgb
                        | ContainerCodec::IcnsArgb
                ) || item.scale().is_none()
            }) {
                return Err(DatabaseError::InvalidContainerMetadata(
                    "ICNS codec or scale",
                ));
            }
            Ok(())
        }
        (_, Some(_)) => Err(DatabaseError::InvalidContainerMetadata(
            "inventory on simple image",
        )),
        (_, None) => Ok(()),
    }
}

fn read_container(
    connection: &Connection,
    hash: ObjectHash,
    metadata: Option<ImageMetadata>,
) -> Result<Option<ContainerMetadata>, DatabaseError> {
    let mut statement = connection.prepare(
        "SELECT ordinal, width, height, scale, bit_depth, codec, encoded_size, is_primary \
         FROM object_representations WHERE object_hash = ?1 ORDER BY ordinal",
    )?;
    let mut rows = statement.query([hash.digest_bytes().as_slice()])?;
    let mut representations = Vec::new();
    let mut primary = None;
    while let Some(row) = rows.next()? {
        let ordinal = u16::try_from(row.get::<_, i64>(0)?)
            .map_err(|_| DatabaseError::InvalidStoredValue("representation ordinal"))?;
        let width = u32::try_from(row.get::<_, i64>(1)?)
            .map_err(|_| DatabaseError::InvalidStoredValue("representation width"))?;
        let height = u32::try_from(row.get::<_, i64>(2)?)
            .map_err(|_| DatabaseError::InvalidStoredValue("representation height"))?;
        let scale = row
            .get::<_, Option<i64>>(3)?
            .map(|value| {
                u8::try_from(value)
                    .map_err(|_| DatabaseError::InvalidStoredValue("representation scale"))
            })
            .transpose()?;
        let bit_depth = row
            .get::<_, Option<i64>>(4)?
            .map(|value| {
                u16::try_from(value)
                    .map_err(|_| DatabaseError::InvalidStoredValue("representation bit depth"))
            })
            .transpose()?;
        let codec = row
            .get::<_, String>(5)?
            .parse::<ContainerCodec>()
            .map_err(|_| DatabaseError::InvalidStoredValue("container codec"))?;
        let encoded_size = u64::try_from(row.get::<_, i64>(6)?)
            .map_err(|_| DatabaseError::InvalidStoredValue("representation encoded size"))?;
        let is_primary: i64 = row.get(7)?;
        let item = ContainerRepresentation::new(
            ordinal,
            width,
            height,
            bit_depth,
            codec,
            encoded_size,
            scale,
        )
        .map_err(|_| DatabaseError::InvalidStoredValue("container representation"))?;
        if representations
            .last()
            .is_some_and(|previous: &ContainerRepresentation| previous.ordinal() >= ordinal)
        {
            return Err(DatabaseError::InvalidStoredValue(
                "representation ordinal order",
            ));
        }
        match is_primary {
            0 => {}
            1 if primary.is_none() => primary = Some(ordinal),
            1 => {
                return Err(DatabaseError::InvalidStoredValue(
                    "multiple primary representations",
                ));
            }
            _ => return Err(DatabaseError::InvalidStoredValue("primary flag")),
        }
        representations.push(item);
    }
    if representations.is_empty() {
        return Ok(None);
    }
    let primary = primary.ok_or(DatabaseError::InvalidStoredValue(
        "missing primary representation",
    ))?;
    let container = ContainerMetadata::new(representations, primary)
        .map_err(|_| DatabaseError::InvalidStoredValue("container inventory"))?;
    let format = metadata
        .ok_or(DatabaseError::InvalidStoredValue(
            "inventory without image metadata",
        ))?
        .format();
    validate_container(format, Some(&container), false)
        .map_err(|_| DatabaseError::InvalidStoredValue("container format mismatch"))?;
    Ok(Some(container))
}

fn insert_container(
    transaction: &Transaction<'_>,
    hash: ObjectHash,
    container: &ContainerMetadata,
) -> Result<(), DatabaseError> {
    for item in container.representations() {
        let encoded_size = i64::try_from(item.encoded_size()).map_err(|_| {
            DatabaseError::InvalidContainerMetadata("encoded size exceeds SQLite INTEGER")
        })?;
        transaction.execute(
            "INSERT INTO object_representations (object_hash, ordinal, width, height, scale, bit_depth, codec, encoded_size, is_primary) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![hash.digest_bytes().as_slice(), i64::from(item.ordinal()), i64::from(item.width()),
                i64::from(item.height()), item.scale().map(i64::from), item.bit_depth().map(i64::from),
                item.codec().as_str(), encoded_size, i64::from(item.ordinal() == container.primary_ordinal())],
        )?;
    }
    Ok(())
}

fn reconcile_object(
    transaction: &Transaction<'_>,
    object: &ObjectRecord,
    metadata: ImageMetadata,
    container: Option<&ContainerMetadata>,
) -> Result<(), DatabaseError> {
    let row: Option<ObjectRow> = transaction
        .query_row(
            "SELECT hash, size_bytes, relative_path, format, width, height, animated \
             FROM objects WHERE hash = ?1",
            [object.hash.digest_bytes().as_slice()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()?;
    if let Some(row) = row {
        let mut existing = stored_object_from_row(row)?;
        existing.container = read_container(transaction, object.hash, existing.metadata)?;
        if &existing.object != object {
            return Err(DatabaseError::ObjectConflict(object.hash));
        }
        match existing.metadata {
            Some(found) if found != metadata => {
                return Err(DatabaseError::ImageMetadataConflict(object.hash));
            }
            Some(_) => {}
            None => {
                transaction.execute(
                    "UPDATE objects SET format = ?2, width = ?3, height = ?4, animated = ?5 \
                     WHERE hash = ?1",
                    params![
                        object.hash.digest_bytes().as_slice(),
                        metadata.format().as_str(),
                        i64::from(metadata.width()),
                        i64::from(metadata.height()),
                        i64::from(metadata.animated())
                    ],
                )?;
            }
        }
        match (existing.container.as_ref(), container) {
            (Some(found), Some(incoming)) if found != incoming => {
                return Err(DatabaseError::ContainerMetadataConflict(object.hash));
            }
            (None, Some(incoming)) => insert_container(transaction, object.hash, incoming)?,
            _ => {}
        }
    } else {
        let size =
            i64::try_from(object.size).map_err(|_| DatabaseError::SizeOutOfRange(object.size))?;
        let path = object
            .relative_path
            .to_str()
            .ok_or_else(|| DatabaseError::InvalidObjectPath(object.relative_path.clone()))?;
        transaction.execute(
            "INSERT INTO objects (hash, size_bytes, relative_path, format, width, height, animated) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![object.hash.digest_bytes().as_slice(), size, path,
                metadata.format().as_str(), i64::from(metadata.width()),
                i64::from(metadata.height()), i64::from(metadata.animated())],
        )?;
        if let Some(inventory) = container {
            insert_container(transaction, object.hash, inventory)?;
        }
    }
    Ok(())
}

fn validate_asset(asset: &AssetRecord) -> Result<(), DatabaseError> {
    if asset.display_name.is_empty() {
        return Err(DatabaseError::EmptyDisplayName);
    }
    if asset.imported_at_utc_us < 0 {
        return Err(DatabaseError::NegativeImportDate);
    }
    Ok(())
}

fn insert_asset(transaction: &Transaction<'_>, asset: &AssetRecord) -> Result<(), DatabaseError> {
    let result = transaction.execute(
        "INSERT INTO assets \
         (id, object_hash, original_filename, display_name, imported_at_utc_us) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            asset.id.to_bytes().as_slice(),
            asset.object_hash.digest_bytes().as_slice(),
            asset.original_filename.as_bytes(),
            asset.display_name,
            asset.imported_at_utc_us,
        ],
    );
    match result {
        Err(rusqlite::Error::SqliteFailure(error, _))
            if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY =>
        {
            Err(DatabaseError::MissingObject(asset.object_hash))
        }
        other => {
            other?;
            Ok(())
        }
    }
}

fn array16(bytes: Vec<u8>, name: &'static str) -> Result<[u8; 16], DatabaseError> {
    bytes
        .try_into()
        .map_err(|_| DatabaseError::InvalidStoredValue(name))
}

fn array32(bytes: Vec<u8>, name: &'static str) -> Result<[u8; 32], DatabaseError> {
    bytes
        .try_into()
        .map_err(|_| DatabaseError::InvalidStoredValue(name))
}

#[cfg(test)]
mod tests {
    use super::LibraryDatabase;
    use crate::LibraryId;
    use tempfile::tempdir;

    #[test]
    fn writer_connection_applies_full_sync_and_busy_timeout() {
        let root = tempdir().unwrap();
        let database = LibraryDatabase::create(root.path(), LibraryId::new()).unwrap();
        let synchronous: i32 = database
            .connection
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .unwrap();
        let busy_timeout_ms: i32 = database
            .connection
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        assert_eq!(synchronous, 2);
        assert_eq!(busy_timeout_ms, 5_000);
    }

    #[test]
    fn migrated_writer_keeps_full_sync() {
        let root = tempdir().unwrap();
        let id = LibraryId::new();
        let connection = rusqlite::Connection::open(root.path().join("library.db")).unwrap();
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .unwrap();
        connection
            .execute(
                "INSERT INTO library_metadata VALUES (1, ?1)",
                [id.to_bytes().as_slice()],
            )
            .unwrap();
        connection
            .execute_batch(include_str!("../migrations/0002_image_metadata.sql"))
            .unwrap();
        connection.pragma_update(None, "user_version", 2).unwrap();
        drop(connection);

        let database = LibraryDatabase::open(root.path(), id).unwrap();
        let synchronous: i32 = database
            .connection
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .unwrap();
        assert_eq!(synchronous, 2);
        assert_eq!(database.schema_version().unwrap(), 3);
    }
}
