# Library format v1

The application version (Cargo), library format version (`LIBRARY_FORMAT_VERSION = 1`), and SQLite schema version (`DATABASE_SCHEMA_VERSION = 3`) evolve independently. `library.json` stores the format version; `PRAGMA user_version` stores the schema version. A SQL migration does not automatically change the overall format.

## Directory

```text
<library-root>/
├── library.json
├── library.db
├── objects/
│   ├── .tmp/
│   ├── .lock
│   └── <shards...>
└── recovery/
```

Caches live outside this directory. In WAL mode, `library.db-wal` and `library.db-shm` may appear at its root; a copy or backup must represent a consistent SQLite state.

## Creation

The final destination must not exist. Pigoune builds the complete library in a temporary sibling directory on the same filesystem, then publishes it with an atomic `RENAME_NOREPLACE` rename without replacing a destination that appeared meanwhile. `library.json` is complete and synchronized before publication. An error before publication leaves no partial library at the final path. A crash may leave a `.pigoune-create-*` sibling; another creation never automatically removes these remnants.

## Identity and metadata

`library.json` contains only `type: "pigoune-library"`, `library_id` (a canonical lowercase hyphenated UUID v4), and `format_version: 1`. The `library_metadata` table stores the same `LibraryId` as a 16-byte BLOB in a single `singleton = 1` row. Opening for writing compares these identities and fails if they differ. An invalid manifest is never silently repaired.

SQL migration `0001_initial.sql` creates `library_metadata`, `objects`, `assets`, and the `assets_object_hash_idx` index. Migration `0002_image_metadata.sql` rebuilds `objects` and `assets` in one transaction to preserve their foreign key and enforce complete metadata. Migration `0003_container_representations.sql` adds `object_representations` and its partial unique primary index without rebuilding `objects` or `assets`. The tables use `STRICT`; SQLite 3.37 is the minimum version. Pigoune uses `rusqlite` linked to system SQLite, without bundled SQLite. The GNOME 50 SDK/runtime is the reference environment.

SQLite schema v2 stores validated intrinsic properties on `objects`: `format` (a canonical code independent of MIME and file extension), `width`, `height`, and `animated` (0 or 1). A SQL constraint requires all four fields to be either `NULL` or populated with a nonempty format and strictly positive dimensions. Objects migrated from schema v1 have unknown metadata (`NULL`); Pigoune never infers them from a path or name. Later validation of the same content can fill them atomically. A Pigoune version that does not recognize a stored format code currently reports an explicit read error. Validated intrinsic metadata must also be representable in a future recovery manifest.

SQLite schema v3 stores validated ICO/ICNS inventories on the physical object in `object_representations`: physical ordinal, dimensions, optional scale and bit depth, stable codec code, encoded size, and the selected primary flag. The `(object_hash, ordinal)` key preserves gaps; a partial unique index permits at most one primary per object, while the reader requires exactly one when rows exist. Older ICO/ICNS objects from schema v1/v2 may have no inventory; migration neither reparses them nor invents one. A later validated import enriches them atomically with its asset. An existing inventory must match exactly or import fails. Payload and mask offsets, mask references, FourCC values, unknown ICNS elements, parser details, decoded data, and import warnings are not stored. A future operation requiring a precise payload must reparse and revalidate the immutable object. The future recovery manifest must also represent persisted `ContainerMetadata`.

Asset UUIDs are 16-byte BLOBs; object SHA-256 hashes are 32-byte BLOBs. `objects` stores size and a relative path under `objects/`. `assets` references an object through a foreign key and stores the original filename as exact POSIX bytes in a BLOB, a UTF-8 display name in TEXT, and `imported_at_utc_us` as a nonnegative signed integer representing a UTC instant in microseconds since the Unix epoch. No external source path is stored.

Each write connection enables foreign keys, a five-second busy timeout, and `synchronous = FULL`, then verifies `journal_mode = WAL`. Embedded SQL migrations are ascending, sequential, and transactional; `user_version` changes in the corresponding transaction. A database with a newer schema version is refused for writing.

New bytes are first copied into `objects/.tmp` and validated from that copy. `ObjectStore` then durably publishes the physical object before the SQLite transaction records or reconciles `objects` and `object_representations`, creates `assets`, and commits. If SQLite fails, the transaction creates no partial asset; the physical file may remain orphaned until safe maintenance. The foreign key prevents an asset from referencing an object absent from the database. Full byte verification is a separate integrity operation rather than part of ordinary lookups.
