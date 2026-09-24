CREATE TABLE object_representations (
    object_hash BLOB NOT NULL CHECK (length(object_hash) = 32),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal <= 65535),
    width INTEGER NOT NULL CHECK (width > 0),
    height INTEGER NOT NULL CHECK (height > 0),
    scale INTEGER CHECK (scale IS NULL OR scale > 0),
    bit_depth INTEGER CHECK (bit_depth IS NULL OR bit_depth > 0),
    codec TEXT NOT NULL CHECK (length(codec) > 0),
    encoded_size INTEGER NOT NULL CHECK (encoded_size > 0),
    is_primary INTEGER NOT NULL CHECK (is_primary IN (0, 1)),
    PRIMARY KEY (object_hash, ordinal),
    FOREIGN KEY (object_hash) REFERENCES objects(hash)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;

CREATE UNIQUE INDEX object_representations_primary_idx
    ON object_representations(object_hash) WHERE is_primary = 1;
