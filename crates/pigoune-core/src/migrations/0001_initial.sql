CREATE TABLE library_metadata (
    singleton INTEGER NOT NULL PRIMARY KEY
        CHECK (singleton = 1),
    library_id BLOB NOT NULL UNIQUE
        CHECK (length(library_id) = 16)
) STRICT, WITHOUT ROWID;

CREATE TABLE objects (
    hash BLOB PRIMARY KEY NOT NULL
        CHECK (length(hash) = 32),
    size_bytes INTEGER NOT NULL
        CHECK (size_bytes >= 0),
    relative_path TEXT NOT NULL UNIQUE
        CHECK (length(relative_path) > 0)
) STRICT, WITHOUT ROWID;

CREATE TABLE assets (
    id BLOB PRIMARY KEY NOT NULL
        CHECK (length(id) = 16),

    object_hash BLOB NOT NULL
        REFERENCES objects(hash)
        ON UPDATE RESTRICT
        ON DELETE RESTRICT,

    original_filename BLOB NOT NULL
        CHECK (length(original_filename) > 0),

    display_name TEXT NOT NULL
        CHECK (length(display_name) > 0),

    imported_at_utc_us INTEGER NOT NULL
        CHECK (imported_at_utc_us >= 0)
) STRICT, WITHOUT ROWID;

CREATE INDEX assets_object_hash_idx
    ON assets(object_hash);
