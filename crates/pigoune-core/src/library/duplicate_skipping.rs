use std::collections::HashSet;

use super::{AssetId, Library, LibraryError};

const ANY_SKIPPED: &str = "SELECT EXISTS (SELECT 1 FROM collections WHERE skip_duplicates = 1 AND trashed_at_unix_ms IS NULL)";

const ASSETS_LEFT_OUT: &str = "
    WITH RECURSIVE skipped(id) AS (
        SELECT id FROM collections WHERE skip_duplicates = 1 AND trashed_at_unix_ms IS NULL
        UNION
        SELECT child.id FROM collections child
            JOIN skipped ON child.parent_id = skipped.id
        WHERE child.trashed_at_unix_ms IS NULL
    )
    SELECT held.asset_id FROM asset_collections held
        JOIN collections ON collections.id = held.collection_id
            AND collections.trashed_at_unix_ms IS NULL
    GROUP BY held.asset_id
    HAVING min(held.collection_id IN (SELECT id FROM skipped)) = 1";

impl Library {
    pub(super) fn assets_in_skipped_collections(&self) -> Result<HashSet<AssetId>, LibraryError> {
        let any: bool = self
            .connection
            .query_row(ANY_SKIPPED, [], |row| row.get(0))?;
        if !any {
            return Ok(HashSet::new());
        }
        let mut statement = self.connection.prepare(ASSETS_LEFT_OUT)?;
        let left_out = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(left_out)
    }
}
