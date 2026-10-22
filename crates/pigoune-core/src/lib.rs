pub mod annotations;
pub mod library;
mod media;
mod raster;

#[cfg(feature = "migration-stops")]
pub use library::MigrationStop;
pub use library::{
    ALMOST_IDENTICAL_DISTANCE, AdoptError, AnnotatedError, AnnotationRender, Annotations, Asset,
    AssetCommand, AssetError, AssetFilter, AssetId, AssetView, CACHE_DIR_NAME,
    CURRENT_FORMAT_VERSION, CacheFile, Change, ChangeStamp, ClipboardInstall, Collection,
    CollectionCommand, CollectionError, CollectionId, CollectionLook, CollectionPath,
    CollectionRemoval, ColorShare, ConfirmedRemoval, CopyFailure, CopyReport, DATABASE_FILE_NAME,
    DUPLICATE_DISTANCE, DragOutCheck, DuplicateCommand, DuplicateError, DuplicateGroup,
    DuplicateKind, DuplicatePlan, DuplicateProgress, ExportFailure, ExportOutline, FILES_DIR_NAME,
    FlatFiles, FormatShare, GroupMember, HISTORY_LIMIT, HealthIssue, HealthPlan, HealthProgress,
    HealthReport, HealthTarget, IDENTICAL_DETAIL_GAP, IgnoreAction, IgnoreId, IgnoreKind,
    IgnoreOutcome, IgnoredEntry, ImportControl, ImportEnding, ImportError, ImportOutcome,
    ImportProgress, ImportSummary, LARGE_FILE_BYTES, LIBRARY_EXTENSION, LONGEST_TAG_NAME, Library,
    LibraryError, LibraryExportPlan, LibraryExportReport, LibraryOverview, LibraryRecords,
    MAX_QUERY_WORDS, MOST_CHOSEN_AT_ONCE, MOST_IN_A_GROUP, OriginalFile, PROBABLE_DETAIL_GAP,
    RemovalError, RemoveRecordError, ReplaceError, SIMILAR_DISTANCE, SimilarAsset, SimilarSearch,
    SmartCollection, SmartCollectionCommand, SmartCollectionError, SmartCollectionId, SpaceProbe,
    StorageUse, TRASH_RETENTION, Tag, TagCommand, TagError, TagId, TextField, UndoError,
    VERY_CLOSE_DISTANCE, ViewCounts, VolumeSpace, cache_entry_label, cache_file, comparable,
    create_cache_folder, library_display_name, oldest_compatible_version, open_plain_file,
    query_groups, query_text, query_word_count, same_tag_name, storage_entry_label,
    thumbnail_cache_bytes_in, write_render_file, write_thumbnail_file,
};
pub use media::{
    AnimationTiming, AssetColor, AssetFormat, AssetShape, Dimensions, DominantColor, Rgb,
    VisualAnalysis, VisualDetail, VisualHash, animation_timing, dominant_colors, icon_from_pngs,
    single_size_icon, visual_analysis, visual_detail, visual_hash,
};
pub use raster::{RgbaImage, fitted_within};
