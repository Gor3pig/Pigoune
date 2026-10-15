CREATE TABLE IF NOT EXISTS asset_annotations (
    asset_id TEXT PRIMARY KEY NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    document TEXT NOT NULL,
    has_masks INTEGER NOT NULL DEFAULT 0 CHECK (has_masks IN (0, 1)),
    original_byte_size INTEGER NOT NULL,
    updated_at_unix_ms INTEGER NOT NULL
) STRICT;
