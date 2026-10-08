INSERT OR IGNORE INTO asset_tags (asset_id, tag_id)
SELECT asset_tags.asset_id, twin.id
FROM asset_tags
JOIN tags AS slashed ON slashed.id = asset_tags.tag_id
JOIN tags AS twin ON twin.normalized_name = replace(slashed.normalized_name, '/', '-')
WHERE instr(slashed.name, '/') > 0 AND twin.id <> slashed.id;

DELETE FROM asset_tags WHERE tag_id IN (
    SELECT slashed.id FROM tags AS slashed
    JOIN tags AS twin ON twin.normalized_name = replace(slashed.normalized_name, '/', '-')
    WHERE instr(slashed.name, '/') > 0 AND twin.id <> slashed.id
);

DELETE FROM tags WHERE instr(name, '/') > 0 AND EXISTS (
    SELECT 1 FROM tags AS twin
    WHERE twin.normalized_name = replace(tags.normalized_name, '/', '-') AND twin.id <> tags.id
);

UPDATE tags SET name = replace(name, '/', '-'), normalized_name = replace(normalized_name, '/', '-')
WHERE instr(name, '/') > 0;

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
