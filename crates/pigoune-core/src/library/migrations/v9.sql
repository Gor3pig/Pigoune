CREATE TABLE tags_v9 (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    parent_id TEXT REFERENCES tags_v9 (id) ON DELETE CASCADE
) STRICT;

INSERT INTO tags_v9 (id, name, normalized_name) SELECT id, name, normalized_name FROM tags;

CREATE TABLE asset_tags_v9 (
    asset_id TEXT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags_v9 (id) ON DELETE CASCADE,
    PRIMARY KEY (asset_id, tag_id)
) STRICT, WITHOUT ROWID;

INSERT INTO asset_tags_v9 (asset_id, tag_id) SELECT asset_id, tag_id FROM asset_tags;

DROP TABLE asset_tags;
DROP TABLE tags;

ALTER TABLE tags_v9 RENAME TO tags;
ALTER TABLE asset_tags_v9 RENAME TO asset_tags;

CREATE UNIQUE INDEX tags_by_parent_and_name ON tags (ifnull(parent_id, ''), normalized_name);
CREATE INDEX tags_by_parent ON tags (parent_id);
CREATE INDEX asset_tags_by_tag ON asset_tags (tag_id);
