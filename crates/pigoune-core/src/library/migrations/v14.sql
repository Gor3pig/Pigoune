ALTER TABLE collections ADD COLUMN skip_duplicates INTEGER NOT NULL DEFAULT 0 CHECK (skip_duplicates IN (0, 1));
