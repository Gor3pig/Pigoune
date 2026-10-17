CREATE TABLE IF NOT EXISTS asset_annotations (
    asset_id TEXT PRIMARY KEY NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    document TEXT NOT NULL,
    updated_at_unix_ms INTEGER NOT NULL
) STRICT;
