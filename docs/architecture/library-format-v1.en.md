# Library format v1

The application version (Cargo), library format version (`LIBRARY_FORMAT_VERSION = 1`), and SQLite schema version (`DATABASE_SCHEMA_VERSION = 2`) evolve independently. `library.json` stores the format version; `PRAGMA user_version` stores the schema version. A SQL migration does not automatically change the overall format.

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

SQL migration `0001_initial.sql` creates `library_metadata`, `objects`, `assets`, and the `assets_object_hash_idx` index. Migration `0002_image_metadata.sql` rebuilds `objects` and `assets` in one transaction to preserve their foreign key and enforce complete metadata. The tables use `STRICT`; SQLite 3.37 is the minimum version. Pigoune uses `rusqlite` linked to system SQLite, without bundled SQLite. The GNOME 50 SDK/runtime is the reference environment.

SQLite schema v2 stores validated intrinsic properties on `objects`: `format` (a canonical code independent of MIME and file extension), `width`, `height`, and `animated` (0 or 1). A SQL constraint requires all four fields to be either `NULL` or populated with a nonempty format and strictly positive dimensions. Objects migrated from schema v1 have unknown metadata (`NULL`); Pigoune never infers them from a path or name. Later validation of the same content can fill them atomically. A Pigoune version that does not recognize a stored format code currently reports an explicit read error. Validated intrinsic metadata must also be representable in a future recovery manifest.

Asset UUIDs are 16-byte BLOBs; object SHA-256 hashes are 32-byte BLOBs. `objects` stores size and a relative path under `objects/`. `assets` references an object through a foreign key and stores the original filename as exact POSIX bytes in a BLOB, a UTF-8 display name in TEXT, and `imported_at_utc_us` as a nonnegative signed integer representing a UTC instant in microseconds since the Unix epoch. No external source path is stored.

Each write connection enables foreign keys, a five-second busy timeout, and `synchronous = FULL`, then verifies `journal_mode = WAL`. Embedded SQL migrations are ascending, sequential, and transactional; `user_version` changes in the corresponding transaction. A database with a newer schema version is refused for writing.

New bytes are first copied into `objects/.tmp` and validated from that copy. `ObjectStore` then durably publishes the physical object before the SQLite transaction records or reconciles `objects`, creates `assets`, and commits. If SQLite fails, the transaction creates no partial asset; the physical file may remain orphaned until safe maintenance. The foreign key prevents an asset from referencing an object absent from the database. Full byte verification is a separate integrity operation rather than part of ordinary lookups.
