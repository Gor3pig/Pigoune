CREATE TABLE objects_v2 (
    hash BLOB PRIMARY KEY NOT NULL CHECK (length(hash) = 32),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    relative_path TEXT NOT NULL UNIQUE CHECK (length(relative_path) > 0),
    format TEXT,
    width INTEGER,
    height INTEGER,
    animated INTEGER,
    CHECK (
        (format IS NULL AND width IS NULL AND height IS NULL AND animated IS NULL)
        OR
        (format IS NOT NULL AND width IS NOT NULL AND height IS NOT NULL
            AND animated IS NOT NULL AND length(format) > 0
            AND width > 0 AND height > 0 AND animated IN (0, 1))
    )
) STRICT, WITHOUT ROWID;

INSERT INTO objects_v2 (hash, size_bytes, relative_path)
SELECT hash, size_bytes, relative_path FROM objects;

CREATE TABLE assets_v2 (
    id BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
    object_hash BLOB NOT NULL REFERENCES objects_v2(hash)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    original_filename BLOB NOT NULL CHECK (length(original_filename) > 0),
    display_name TEXT NOT NULL CHECK (length(display_name) > 0),
    imported_at_utc_us INTEGER NOT NULL CHECK (imported_at_utc_us >= 0)
) STRICT, WITHOUT ROWID;

INSERT INTO assets_v2 (id, object_hash, original_filename, display_name, imported_at_utc_us)
SELECT id, object_hash, original_filename, display_name, imported_at_utc_us FROM assets;

DROP TABLE assets;
DROP TABLE objects;
ALTER TABLE objects_v2 RENAME TO objects;
ALTER TABLE assets_v2 RENAME TO assets;
CREATE INDEX assets_object_hash_idx ON assets(object_hash);
