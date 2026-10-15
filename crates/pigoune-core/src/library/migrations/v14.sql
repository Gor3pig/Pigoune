ALTER TABLE ignored_duplicate_members ADD COLUMN kept_apart INTEGER NOT NULL DEFAULT 0 CHECK (kept_apart IN (0, 1));
