use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use super::collection::{distinct_count, ids_as_json};
use super::view::{self, AssetView, SUBTREE};
use super::{AssetId, Library, LibraryError, layout};
use crate::media::{
    self, AnimationTiming, AssetFormat, Dimensions, DominantColor, VisualAnalysis, VisualDetail,
    dominant_from_text,
};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{OptionalExtension, Row, params};

const EMBEDDED_SIZES_SEPARATOR: char = ',';
const ASSET_COLUMNS: &str =
    "id, display_name, original_file_name, stored_path, format, width, height,
    byte_size, content_hash, is_animated, embedded_sizes, added_at_unix_ms, trashed_at_unix_ms,
    is_favorite, note, source_url, license, author, colors";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub id: AssetId,
    pub display_name: String,
    pub original_file_name: String,
    pub stored_path: PathBuf,
    pub format: AssetFormat,
    pub dimensions: Option<Dimensions>,
    pub byte_size: u64,
    pub content_hash: String,
    pub is_animated: bool,
    pub embedded_sizes: Vec<Dimensions>,
    pub added_at_unix_ms: i64,
    pub trashed_at_unix_ms: Option<i64>,
    pub is_favorite: bool,
    pub note: String,
    pub source_url: String,
    pub license: String,
    pub author: String,
    pub colors: Vec<DominantColor>,
}

impl Library {
    pub fn asset(&self, id: AssetId) -> Result<Option<Asset>, LibraryError> {
        Ok(self
            .connection
            .query_row(
                &format!("SELECT {ASSET_COLUMNS} FROM assets WHERE id = ?1"),
                [id],
                asset_from_row,
            )
            .optional()?)
    }

    pub fn all_are_favorite(&self, assets: &[AssetId]) -> Result<bool, LibraryError> {
        let favorites: i64 = self.connection.query_row(
            "SELECT count(*) FROM assets
             WHERE is_favorite = 1 AND id IN (SELECT value FROM json_each(?1))",
            [ids_as_json(assets)],
            |row| row.get(0),
        )?;
        Ok(usize::try_from(favorites).is_ok_and(|favorites| favorites == distinct_count(assets)))
    }

    pub fn visible_assets(&self) -> Result<Vec<Asset>, LibraryError> {
        self.visible_assets_in(AssetView::All)
    }

    pub fn visible_assets_in(&self, view: AssetView) -> Result<Vec<Asset>, LibraryError> {
        match view {
            AssetView::Smart(id) => self.smart_collection_assets(id),
            AssetView::All | AssetView::Trash => Ok(self.cached_assets(view)?.as_ref().clone()),
            _ => self.load_assets(view),
        }
    }

    pub(super) fn shared_assets(&self, view: AssetView) -> Result<Arc<Vec<Asset>>, LibraryError> {
        match view {
            AssetView::All | AssetView::Trash => self.cached_assets(view),
            _ => Ok(Arc::new(self.visible_assets_in(view)?)),
        }
    }

    fn cached_assets(&self, view: AssetView) -> Result<Arc<Vec<Asset>>, LibraryError> {
        if !self.connection.is_autocommit() {
            return Ok(Arc::new(self.load_assets(view)?));
        }
        let stamp = self.connection.total_changes();
        let known = {
            let cache = self.query_cache.lock(stamp);
            if matches!(view, AssetView::Trash) {
                cache.trash.clone()
            } else {
                cache.all.clone()
            }
        };
        if let Some(known) = known {
            return Ok(known);
        }
        let loaded = Arc::new(self.load_assets(view)?);
        let mut cache = self.query_cache.lock(stamp);
        if matches!(view, AssetView::Trash) {
            cache.trash = Some(loaded.clone());
        } else {
            cache.all = Some(loaded.clone());
        }
        Ok(loaded)
    }

    fn load_assets(&self, view: AssetView) -> Result<Vec<Asset>, LibraryError> {
        let filter = view::filter(view);
        let clause = filter.clause();
        let mut statement = self.connection.prepare(&format!(
            "{SUBTREE} SELECT {ASSET_COLUMNS} FROM assets
             WHERE {clause}
             ORDER BY added_at_unix_ms DESC, id DESC"
        ))?;
        let assets = statement
            .query_map([filter.parameter], asset_from_row)?
            .collect::<Result<_, _>>()?;
        Ok(assets)
    }

    pub fn view_contains(&self, view: AssetView, asset: AssetId) -> Result<bool, LibraryError> {
        if let AssetView::Smart(id) = view {
            return Ok(self
                .smart_collection_assets(id)?
                .iter()
                .any(|candidate| candidate.id == asset));
        }
        let filter = view::filter(view);
        let clause = filter.clause();
        Ok(self
            .connection
            .query_row(
                &format!("{SUBTREE} SELECT 1 FROM assets WHERE id = ?2 AND {clause}"),
                params![filter.parameter, asset],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    pub fn assets_outside_view(
        &self,
        view: AssetView,
        assets: &[AssetId],
    ) -> Result<Vec<AssetId>, LibraryError> {
        if let AssetView::Smart(id) = view {
            let inside: HashSet<AssetId> = self
                .smart_collection_assets(id)?
                .into_iter()
                .map(|asset| asset.id)
                .collect();
            return Ok(assets
                .iter()
                .copied()
                .filter(|asset| !inside.contains(asset))
                .collect());
        }
        let mut outside = Vec::new();
        for asset in assets {
            if !self.view_contains(view, *asset)? {
                outside.push(*asset);
            }
        }
        Ok(outside)
    }

    #[must_use]
    pub fn file_of(&self, asset: &Asset) -> PathBuf {
        self.root.join(&asset.stored_path)
    }

    #[must_use]
    pub fn animation_timing(&self, asset: &Asset) -> Option<AnimationTiming> {
        if asset.is_animated {
            media::animation_timing(&self.file_of(asset), asset.format)
        } else {
            None
        }
    }

    pub fn thumbnail_file(&self, id: AssetId, pixels: u32) -> PathBuf {
        layout::thumbnail_path(&self.root, id, pixels)
    }

    pub fn assets_awaiting_colors(&self, limit: usize) -> Result<Vec<AssetId>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT id FROM assets WHERE colors IS NULL
             ORDER BY added_at_unix_ms DESC, id DESC LIMIT ?1",
        )?;
        let ids = statement
            .query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    pub fn record_colors_batch(
        &mut self,
        batch: &[(AssetId, Vec<DominantColor>)],
    ) -> Result<(), LibraryError> {
        let transaction = self.connection.transaction()?;
        {
            let mut statement =
                transaction.prepare_cached("UPDATE assets SET colors = ?2 WHERE id = ?1")?;
            for (id, colors) in batch {
                statement.execute(params![id, media::dominant_text(colors)])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn assets_awaiting_visual_hash(&self, limit: usize) -> Result<Vec<AssetId>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT id FROM assets WHERE visual_hash IS NULL
             ORDER BY added_at_unix_ms DESC, id DESC LIMIT ?1",
        )?;
        let ids = statement
            .query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    pub fn count_awaiting_visual_hash(&self) -> Result<u64, LibraryError> {
        Ok(self.connection.query_row(
            "SELECT COUNT(*) FROM assets
             WHERE visual_hash IS NULL AND trashed_at_unix_ms IS NULL",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn analysis_backlog(&self) -> Result<(u64, u64), LibraryError> {
        Ok(self.connection.query_row(
            "SELECT COUNT(CASE WHEN colors IS NULL OR visual_hash IS NULL THEN 1 END), COUNT(*)
             FROM assets WHERE trashed_at_unix_ms IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    }

    fn keeping_queries_cached(
        &mut self,
        write: impl FnOnce(&mut Self) -> Result<(), LibraryError>,
    ) -> Result<(), LibraryError> {
        let outside_transaction = self.connection.is_autocommit();
        let before = self.connection.total_changes();
        let written = write(self);
        if outside_transaction && written.is_ok() {
            self.query_cache
                .carry(before, self.connection.total_changes());
        }
        written
    }

    pub fn record_visual_details_batch(
        &mut self,
        batch: &[(AssetId, Option<VisualDetail>)],
    ) -> Result<(), LibraryError> {
        self.keeping_queries_cached(|library| library.store_visual_details(batch))
    }

    fn store_visual_details(
        &mut self,
        batch: &[(AssetId, Option<VisualDetail>)],
    ) -> Result<(), LibraryError> {
        let transaction = self.connection.transaction()?;
        {
            let mut statement = transaction.prepare_cached(
                "INSERT OR REPLACE INTO details.asset_details (asset_id, detail)
                 SELECT ?1, ?2 WHERE EXISTS (SELECT 1 FROM main.assets WHERE id = ?1)",
            )?;
            for (id, detail) in batch {
                let bytes = detail
                    .as_ref()
                    .map_or_else(Vec::new, |detail| detail.as_bytes().to_vec());
                statement.execute(params![id, bytes])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn record_visual_analysis_batch(
        &mut self,
        batch: &[(AssetId, Option<VisualAnalysis>)],
    ) -> Result<(), LibraryError> {
        self.keeping_queries_cached(|library| library.store_visual_analysis(batch))
    }

    fn store_visual_analysis(
        &mut self,
        batch: &[(AssetId, Option<VisualAnalysis>)],
    ) -> Result<(), LibraryError> {
        let mut analyses: Vec<(&AssetId, Vec<u8>, Vec<u8>)> = Vec::with_capacity(batch.len());
        for (id, analysis) in batch {
            let (hash, detail): (Vec<u8>, Vec<u8>) = analysis.as_ref().map_or_else(
                || (Vec::new(), Vec::new()),
                |analysis| {
                    (
                        analysis.hash.to_bytes().to_vec(),
                        analysis.detail.as_bytes().to_vec(),
                    )
                },
            );
            analyses.push((id, hash, detail));
        }
        let transaction = self.connection.transaction()?;
        {
            let mut hashes =
                transaction.prepare_cached("UPDATE assets SET visual_hash = ?2 WHERE id = ?1")?;
            for (id, hash, _) in &analyses {
                hashes.execute(params![id, hash])?;
            }
        }
        transaction.commit()?;
        let transaction = self.connection.transaction()?;
        {
            let mut details = transaction.prepare_cached(
                "INSERT OR REPLACE INTO details.asset_details (asset_id, detail)
                 SELECT ?1, ?2 WHERE EXISTS (SELECT 1 FROM main.assets WHERE id = ?1)",
            )?;
            for (id, _, detail) in &analyses {
                details.execute(params![id, detail])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn record_colors(
        &mut self,
        id: AssetId,
        colors: &[DominantColor],
    ) -> Result<(), LibraryError> {
        self.record_colors_batch(&[(id, colors.to_vec())])
    }
}

fn asset_from_row(row: &Row) -> rusqlite::Result<Asset> {
    let width: Option<u32> = row.get("width")?;
    let height: Option<u32> = row.get("height")?;
    Ok(Asset {
        id: row.get("id")?,
        display_name: row.get("display_name")?,
        original_file_name: row.get("original_file_name")?,
        stored_path: PathBuf::from(row.get::<_, String>("stored_path")?),
        format: row.get("format")?,
        dimensions: width
            .zip(height)
            .and_then(|(width, height)| Dimensions::new(width, height)),
        byte_size: row.get("byte_size")?,
        content_hash: row.get("content_hash")?,
        is_animated: row.get("is_animated")?,
        embedded_sizes: row.get::<_, EmbeddedSizes>("embedded_sizes")?.0,
        added_at_unix_ms: row.get("added_at_unix_ms")?,
        trashed_at_unix_ms: row.get("trashed_at_unix_ms")?,
        is_favorite: row.get("is_favorite")?,
        note: row.get("note")?,
        source_url: row.get("source_url")?,
        license: row.get("license")?,
        author: row.get("author")?,
        colors: row
            .get::<_, Option<String>>("colors")?
            .map(|text| dominant_from_text(&text))
            .unwrap_or_default(),
    })
}

pub(super) struct EmbeddedSizes(pub Vec<Dimensions>);

impl ToSql for EmbeddedSizes {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let text = self
            .0
            .iter()
            .map(Dimensions::to_string)
            .collect::<Vec<_>>()
            .join(&EMBEDDED_SIZES_SEPARATOR.to_string());
        Ok(ToSqlOutput::from(text))
    }
}

impl FromSql for EmbeddedSizes {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        if text.is_empty() {
            return Ok(Self(Vec::new()));
        }
        text.split(EMBEDDED_SIZES_SEPARATOR)
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map(Self)
            .map_err(|_| FromSqlError::InvalidType)
    }
}

impl ToSql for AssetFormat {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.code()))
    }
}

impl FromSql for AssetFormat {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Self::from_code(value.as_str()?).ok_or(FromSqlError::InvalidType)
    }
}

#[cfg(test)]
mod cache_tests {
    use std::path::Path;
    use std::sync::Arc;

    use rusqlite::Connection;

    use super::{AssetView, DominantColor, Library, LibraryError};
    use crate::library::{AssetCommand, AssetId, ImportOutcome, TagCommand};
    use crate::media::{AssetColor, Rgb};

    struct Fixture {
        _workspace: tempfile::TempDir,
        library: Library,
        assets: Vec<AssetId>,
    }

    fn fixture() -> Fixture {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
        let mut assets = Vec::new();
        for name in ["red-dot.png", "dark-circle.svg", "blue-photo.jpg"] {
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name);
            match library.import_file(&source, None).expect("import succeeds") {
                ImportOutcome::Imported(id) => assets.push(id),
                outcome => panic!("unexpected outcome {outcome:?}"),
            }
        }
        library
            .apply_tag_command(&TagCommand::Add {
                assets: assets.clone(),
                name: "Brands/Logo".to_owned(),
            })
            .expect("assets are tagged");
        Fixture {
            _workspace: workspace,
            library,
            assets,
        }
    }

    fn warm(library: &Library) -> (Arc<Vec<super::Asset>>, Arc<Vec<super::Asset>>) {
        let all = library.shared_assets(AssetView::All).expect("listing");
        let trash = library.shared_assets(AssetView::Trash).expect("trash");
        library.tag_names_by_asset().expect("tag names");
        (all, trash)
    }

    fn still_cached(library: &Library, all: &Arc<Vec<super::Asset>>) -> bool {
        Arc::ptr_eq(
            &library.shared_assets(AssetView::All).expect("listing"),
            all,
        )
    }

    #[test]
    fn the_hash_the_details_and_both_in_a_row_keep_a_valid_cache() {
        let mut fixture = fixture();
        let id = fixture.assets[0];
        let (all, _) = warm(&fixture.library);
        let tags = fixture.library.tag_names_by_asset().expect("tag names");

        fixture
            .library
            .record_visual_analysis_batch(&[(id, None)])
            .expect("the analysis is recorded");
        assert!(still_cached(&fixture.library, &all));
        fixture
            .library
            .record_visual_details_batch(&[(id, None)])
            .expect("the details are recorded");
        assert!(still_cached(&fixture.library, &all));
        fixture
            .library
            .record_visual_analysis_batch(&[(id, None)])
            .expect("the analysis is recorded again");
        assert!(still_cached(&fixture.library, &all));
        assert!(Arc::ptr_eq(
            &fixture.library.tag_names_by_asset().expect("tag names"),
            &tags
        ));
    }

    #[test]
    fn a_cache_already_stale_before_the_write_is_not_revived() {
        let mut fixture = fixture();
        let id = fixture.assets[0];
        let (all, _) = warm(&fixture.library);
        fixture
            .library
            .apply_asset_command(&AssetCommand::SetFavorite {
                assets: vec![id],
                favorite: true,
            })
            .expect("a favorite is set");

        fixture
            .library
            .record_visual_analysis_batch(&[(id, None)])
            .expect("the analysis is recorded");

        assert!(!still_cached(&fixture.library, &all));
        let shown = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert!(
            shown
                .iter()
                .any(|asset| asset.id == id && asset.is_favorite)
        );
    }

    #[test]
    fn a_colour_write_after_the_analysis_still_invalidates_the_cache() {
        let mut fixture = fixture();
        let id = fixture.assets[0];
        let (all, _) = warm(&fixture.library);
        fixture
            .library
            .record_visual_analysis_batch(&[(id, None)])
            .expect("the analysis is recorded");
        assert!(still_cached(&fixture.library, &all));
        let colours = vec![DominantColor {
            family: AssetColor::Red,
            average: Rgb::new(200, 20, 20),
        }];

        fixture
            .library
            .record_colors_batch(&[(id, colours.clone())])
            .expect("colours are recorded");

        assert!(!still_cached(&fixture.library, &all));
        let shown = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert!(
            shown
                .iter()
                .any(|asset| asset.id == id && asset.colors == colours)
        );
    }

    #[test]
    fn a_write_that_fails_halfway_keeps_the_usual_invalidation_and_the_error() {
        let mut fixture = fixture();
        let id = fixture.assets[0];
        let (all, _) = warm(&fixture.library);
        let details = fixture
            .library
            .root
            .join(crate::library::CACHE_DIR_NAME)
            .join("visual-details.db");
        let outside = Connection::open(&details).expect("the details file opens");
        let definition: String = outside
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'asset_details'",
                [],
                |row| row.get(0),
            )
            .expect("the details table exists");
        outside
            .execute_batch("DROP TABLE asset_details")
            .expect("the table is dropped");

        let result = fixture.library.record_visual_analysis_batch(&[(id, None)]);
        outside
            .execute_batch(&definition)
            .expect("the table is restored");

        assert!(matches!(result, Err(LibraryError::Database(_))));
        assert!(!still_cached(&fixture.library, &all));
    }

    #[test]
    fn nothing_is_carried_when_the_connection_was_inside_a_transaction() {
        let mut fixture = fixture();
        let (all, _) = warm(&fixture.library);
        fixture
            .library
            .connection
            .execute_batch("BEGIN")
            .expect("a transaction opens");

        let written = fixture.library.keeping_queries_cached(|library| {
            library
                .connection
                .execute("UPDATE assets SET note = 'changed'", [])?;
            Ok(())
        });
        fixture
            .library
            .connection
            .execute_batch("COMMIT")
            .expect("the transaction commits");

        assert!(written.is_ok());
        assert!(!still_cached(&fixture.library, &all));
    }

    #[test]
    fn a_library_without_a_warm_cache_is_unaffected() {
        let mut fixture = fixture();
        let id = fixture.assets[0];

        fixture
            .library
            .record_visual_details_batch(&[(id, None)])
            .expect("the details are recorded");

        let all = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert_eq!(all.len(), 3);
    }
}
