mod error;
mod migration;

pub use error::DatabaseError;

use super::{DATABASE_SCHEMA_VERSION, LibraryId, OriginalFilename, path::valid_object_path};
use crate::{AssetId, ObjectHash, ObjectRecord};
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

    pub fn get_object(&self, hash: ObjectHash) -> Result<Option<ObjectRecord>, DatabaseError> {
        let row: Option<(Vec<u8>, i64, String)> = self
            .connection
            .query_row(
                "SELECT hash, size_bytes, relative_path FROM objects WHERE hash = ?1",
                [hash.digest_bytes().as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        row.map(|(hash_bytes, size, path)| object_from_row(hash_bytes, size, path))
            .transpose()
    }

    pub fn register_object(&mut self, object: &ObjectRecord) -> Result<(), DatabaseError> {
        validate_object(object)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        reconcile_object(&transaction, object)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn create_asset(&mut self, asset: &AssetRecord) -> Result<(), DatabaseError> {
        validate_asset(asset)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_asset(&transaction, asset)?;
        transaction.commit()?;
        Ok(())
    }

    /// Record metadata only after `ObjectStore` has published the object.
    pub fn import_published_asset(
        &mut self,
        object: &ObjectRecord,
        asset: &AssetRecord,
    ) -> Result<(), DatabaseError> {
        validate_object(object)?;
        validate_asset(asset)?;
        if object.hash != asset.object_hash {
            return Err(DatabaseError::AssetObjectMismatch);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        reconcile_object(&transaction, object)?;
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

fn reconcile_object(
    transaction: &Transaction<'_>,
    object: &ObjectRecord,
) -> Result<(), DatabaseError> {
    let row: Option<(i64, String)> = transaction
        .query_row(
            "SELECT size_bytes, relative_path FROM objects WHERE hash = ?1",
            [object.hash.digest_bytes().as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((size, path)) = row {
        let existing = object_from_row(object.hash.digest_bytes().to_vec(), size, path)?;
        if &existing != object {
            return Err(DatabaseError::ObjectConflict(object.hash));
        }
    } else {
        let size =
            i64::try_from(object.size).map_err(|_| DatabaseError::SizeOutOfRange(object.size))?;
        let path = object
            .relative_path
            .to_str()
            .ok_or_else(|| DatabaseError::InvalidObjectPath(object.relative_path.clone()))?;
        transaction.execute(
            "INSERT INTO objects (hash, size_bytes, relative_path) VALUES (?1, ?2, ?3)",
            params![object.hash.digest_bytes().as_slice(), size, path],
        )?;
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
}
