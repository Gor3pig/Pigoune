CREATE TABLE assets (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    original_file_name TEXT NOT NULL,
    stored_path TEXT NOT NULL UNIQUE,
    format TEXT NOT NULL,
    width INTEGER,
    height INTEGER,
    byte_size INTEGER NOT NULL,
    content_hash TEXT NOT NULL UNIQUE,
    is_animated INTEGER NOT NULL DEFAULT 0 CHECK (is_animated IN (0, 1)),
    embedded_sizes TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT '',
    source_url TEXT NOT NULL DEFAULT '',
    license TEXT NOT NULL DEFAULT '',
    author TEXT NOT NULL DEFAULT '',
    is_favorite INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),
    added_at_unix_ms INTEGER NOT NULL,
    trashed_at_unix_ms INTEGER
) STRICT;

CREATE INDEX assets_by_added_at ON assets (added_at_unix_ms);
CREATE INDEX assets_by_trashed_at ON assets (trashed_at_unix_ms);

CREATE TABLE collections (
    id TEXT PRIMARY KEY NOT NULL,
    parent_id TEXT REFERENCES collections (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at_unix_ms INTEGER NOT NULL,
    trashed_at_unix_ms INTEGER
) STRICT;

CREATE INDEX collections_by_parent ON collections (parent_id);

CREATE TABLE asset_collections (
    asset_id TEXT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    collection_id TEXT NOT NULL REFERENCES collections (id) ON DELETE CASCADE,
    PRIMARY KEY (asset_id, collection_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX asset_collections_by_collection ON asset_collections (collection_id);

CREATE TABLE tags (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL UNIQUE
) STRICT;

CREATE TABLE asset_tags (
    asset_id TEXT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    PRIMARY KEY (asset_id, tag_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX asset_tags_by_tag ON asset_tags (tag_id);
