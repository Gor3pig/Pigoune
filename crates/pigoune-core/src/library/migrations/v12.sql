CREATE TABLE IF NOT EXISTS ignored_duplicate_actions (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('group', 'member')),
    subject_asset TEXT REFERENCES assets (id) ON DELETE CASCADE,
    created_at_unix_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS ignored_duplicate_members (
    action_id TEXT NOT NULL REFERENCES ignored_duplicate_actions (id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    kept_apart INTEGER NOT NULL DEFAULT 0 CHECK (kept_apart IN (0, 1)),
    PRIMARY KEY (action_id, asset_id)
) STRICT;

CREATE INDEX IF NOT EXISTS ignored_duplicate_members_by_asset ON ignored_duplicate_members (asset_id);
CREATE INDEX IF NOT EXISTS ignored_duplicate_actions_by_subject ON ignored_duplicate_actions (subject_asset);
