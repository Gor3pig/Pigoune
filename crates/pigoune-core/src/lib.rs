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
    AnimationTiming, Archive, ArchiveEntry, ArchiveError, AssetColor, AssetFormat, AssetShape,
    BaseType, Dimensions, DominantColor, GimpRender, LayerInfo, LayerKind, LayerRole, PageFile,
    Precision, PsdError, Rgb, SampleFormat, SampleGrid, Samples, VisualAnalysis, VisualDetail,
    VisualHash, XcfDocument, XcfError, XcfReader, animation_timing, compose_image,
    container_format_of, container_preview, dominant_colors, first_stored_text, gimp_dimensions,
    gimp_file_is_intact, gimp_problem, icon_from_pngs, is_gimp_file, is_layered_file,
    is_photoshop_file, layered_format_of, layered_preview_png, page_file, photoshop_file_is_intact,
    photoshop_problem, read_merged_image, read_merged_image_file, read_merged_image_from,
    render_gimp_file, single_size_icon, visual_analysis, visual_detail, visual_hash,
};
pub use raster::{RgbaImage, fitted_within};
