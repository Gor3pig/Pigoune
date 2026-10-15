CREATE TABLE IF NOT EXISTS ignored_duplicates (
    first_asset TEXT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    second_asset TEXT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    ignored_at_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (first_asset, second_asset),
    CHECK (first_asset < second_asset)
) STRICT;

CREATE INDEX IF NOT EXISTS ignored_duplicates_by_second ON ignored_duplicates (second_asset);
