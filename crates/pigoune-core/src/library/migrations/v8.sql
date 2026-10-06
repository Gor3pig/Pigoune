ALTER TABLE smart_collections ADD COLUMN shapes TEXT NOT NULL DEFAULT '';
ALTER TABLE smart_collections ADD COLUMN fits_screen INTEGER NOT NULL DEFAULT 0;
UPDATE smart_collections SET favorites_only = 1 WHERE scope = 'favorites';
UPDATE smart_collections SET scope = 'all';
