CREATE TABLE smart_collections (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL UNIQUE,
    scope TEXT NOT NULL,
    search_text TEXT NOT NULL DEFAULT '',
    formats TEXT NOT NULL DEFAULT '',
    favorites_only INTEGER NOT NULL DEFAULT 0 CHECK (favorites_only IN (0, 1)),
    position INTEGER NOT NULL DEFAULT 0,
    created_at_unix_ms INTEGER NOT NULL
) STRICT;
