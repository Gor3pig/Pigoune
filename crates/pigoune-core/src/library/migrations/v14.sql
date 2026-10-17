ALTER TABLE asset_annotations ADD COLUMN seal_generation INTEGER NOT NULL DEFAULT 0
    CHECK (seal_generation >= 0);
ALTER TABLE asset_annotations ADD COLUMN seal_base_hash TEXT;
ALTER TABLE asset_annotations ADD COLUMN seal_phase TEXT NOT NULL DEFAULT 'none'
    CHECK (seal_phase IN ('none', 'preparing', 'finalizing'));
ALTER TABLE asset_annotations ADD COLUMN seal_journal TEXT;
