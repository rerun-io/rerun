//! Rerun Viewer context
//!
//! This crate contains data structures that are shared with most modules of the viewer.

// Increased recursion is needed for VideoAssetCache.
// We could also add Send bounds like this to help limit recursion:
// impl Cache for VideoAssetCache
// where
//     Video: Send + Sync,
//     VideoLoadError: Send + Sync,
// but that slows compilation down by a ton
#![recursion_limit = "256"]

mod active_store_context;
mod annotations;
mod app_context;
mod app_options;
mod audio_output;
mod blueprint_helpers;
mod blueprint_id;
mod cache;
mod collapsed_id;
mod command;
mod command_sender;
mod component_fallbacks;
mod component_ui_registry;
mod contents;
mod drag_and_drop;
mod file_dialog;
mod focus_target;
mod heuristics;
mod image_info;
mod item;
mod item_collection;
mod item_counter;
mod link_button;
mod maybe_mut_ref;
mod query_context;
mod query_range;
mod recording_or_table;
mod relative_time_range;
mod route;
mod selection_state;
mod storage_context;
pub mod store_hub;
mod store_view_context;
mod syntax_highlighting;
mod tables;
mod tensor;
mod time_control;
mod time_drag_value;
mod typed_entity_collections;
mod undo;
mod utils;
mod view;
mod viewer_context;
pub mod viewer_url;

// TODO(andreas): Move to its own crate?
pub mod gpu_bridge;
mod visitor_flow_control;

pub use re_ui::UiLayout;

/// Reflection shared by the viewer and its visualizers; clones observe the same runtime edits.
pub type SharedReflection =
    std::sync::Arc<parking_lot::RwLock<re_types_core::reflection::Reflection>>;

/// Cursor shown when the pointer can move the time cursor.
pub const MOVE_TIME_CURSOR_ICON: egui::CursorIcon = egui::CursorIcon::ResizeColumn;

pub use self::active_store_context::ActiveStoreContext;
pub use self::annotations::{
    AnnotationContextQuery, AnnotationContextStoreSubscriber, AnnotationContextTarget,
    AnnotationContextTargetKind, AnnotationMap, Annotations, ResolvedAnnotationInfo,
};
pub use self::app_context::{AppContext, AuthContext};
pub use self::app_options::{AppOptions, ExperimentalAppOptions, VideoOptions};
pub use self::audio_output::AudioOutput;
pub use self::blueprint_helpers::{
    AppBlueprintCtx, BlueprintContext, blueprint_timeline, blueprint_timepoint_for_writes,
    entity_path_for_view_property,
};
pub use self::blueprint_id::{
    BlueprintId, BlueprintIdRegistry, ContainerId, GLOBAL_VIEW_ID, ViewId,
};
pub use self::cache::{
    AppCache, AppCaches, Cache, CacheEntryAccess, EncodedDepthImageStatsCache, ImageDecodeCache,
    ImageHistogramCache, ImageStatsCache, Rgb8Histogram, SharablePlayableVideoStream, StoreCache,
    TensorStatsAccessor, TensorStatsCache, TransformDatabaseStoreCache, VideoAssetCache,
    VideoStoreSource, VideoStreamCache, VideoStreamProcessingError, filter_blob_removed_events,
};
pub use self::collapsed_id::{CollapseItem, CollapseScope, CollapsedId};
pub use self::command::{
    BoundCommand, CommandEnvironment, CommandKind, CommandScope, RecordingCommand,
    RecordingCommandKind, RecordingCommandSender, RedapServerCommand, RedapServerCommandKind,
    RedapServerCommandSender, SetPlaybackSpeed, TableCommand, TableCommandKind, TableCommandSender,
    consume_timeline_shortcut, listen_for_kb_shortcuts, redo_shortcuts, refresh_shortcuts,
    undo_shortcuts,
};
pub use self::command_sender::{
    CommandReceiver, CommandSender, DownloadAction, EditRedapServerModalCommand, SystemCommand,
    SystemCommandSender, command_channel,
};
pub use self::component_fallbacks::{
    ComponentFallbackError, FallbackProviderRegistry, typed_fallback_for,
};
pub use self::component_ui_registry::{
    ComponentUiRegistry, ComponentUiTypes, EditTarget, FallbackComponentUiCallback,
    TryShowEditUiResult,
};
pub use self::contents::{Contents, ContentsName, blueprint_id_to_tile_id};
pub use self::drag_and_drop::{DragAndDropFeedback, DragAndDropManager, DragAndDropPayload};
pub use self::file_dialog::sanitize_file_name;
pub use self::focus_target::FocusTarget;
pub use self::heuristics::suggest_view_for_each_entity;
pub use self::image_info::{
    ColormapWithRange, ImageInfo, StoredBlobCacheKey, resolution_of_image_at,
};
pub use self::item::{
    DataResultInteractionAddress, Item, RedapEntryKind, resolve_mono_instance_path,
    resolve_mono_instance_path_item,
};
pub use self::item_collection::{ItemCollection, ItemContext};
pub use self::item_counter::ItemCounter;
pub use self::link_button::{
    LinkKind, ResolvedEntry, UrlNameLookup, make_url_decorator, segment_button_atoms, url_atoms,
};
pub use self::maybe_mut_ref::MaybeMutRef;
pub use self::query_context::{
    DataQueryResult, DataResultHandle, DataResultNode, DataResultTree, QueryContext,
};
pub use self::query_range::QueryRange;
pub use self::recording_or_table::RecordingOrLocalTable;
pub use self::relative_time_range::{
    RelativeTimeRange, relative_time_range_boundary_label_text, relative_time_range_label_text,
};
pub use self::route::{EntryKind, Route};
pub use self::selection_state::{
    ApplicationSelectionState, HoverHighlight, InteractionHighlight, SelectionChange,
    SelectionHighlight,
};
pub use self::storage_context::StorageContext;
pub use self::store_hub::{EntityDbUsages, StoreHub};
pub use self::store_view_context::StoreViewContext;
pub use self::syntax_highlighting::{InstanceInBrackets, StoreSyntaxHighlighting};
pub use self::tables::{TableStore, TableStores};
pub use self::tensor::{ImageStats, TensorStats};
pub use self::time_control::{
    MoveDirection, MoveSpeed, TIME_PANEL_PATH, TimeControl, TimeControlCommand,
    TimeControlResponse, TimeControlUpdateParams, TimeRangeHighlight, TimeRangeHighlightKind,
    TimeView, time_panel_blueprint_entity_path,
};
pub use self::time_drag_value::TimeDragValue;
pub use self::typed_entity_collections::{
    BufferAndFormatMatch, ComponentMatch, IndicatedEntities, PerVisualizerInstruction,
    PerVisualizerType, PerVisualizerTypeInViewClass, SingleRequiredComponentMatch,
    VisualizableEntities, VisualizableReason,
};
pub use self::undo::{BlueprintUndoState, is_interacting, on_external_blueprint_edit};
pub use self::utils::{
    auto_color_egui, auto_color_for_entity_path, level_to_rich_text, video_stream_time_from_query,
    video_timestamp_component_to_video_time,
};
pub use self::view::{
    ActivePreview, BlueprintPropertiesUi, BufferAndFormatConstraint, DataResult,
    IdentifiedViewSystem, MAX_VIEWS_SPAWNED, NoopVisualizer, OptionalViewEntityHighlight,
    PerSystemEntities, PreviewState, RecommendedMappings, RecommendedView, RecommendedVisualizers,
    SelectionSection, SelectionSectionUi, SingleRequiredComponentConstraint, SystemExecutionOutput,
    ViewClass, ViewClassExt, ViewClassLayoutPriority, ViewClassPlaceholder, ViewClassRegistry,
    ViewClassRegistryError, ViewClassUiOutput, ViewContext, ViewContextCollection,
    ViewContextSystem, ViewContextSystemOncePerFrameResult, ViewEntityHighlight, ViewHighlights,
    ViewOutlineMasks, ViewQuery, ViewSelectionUi, ViewSpawnHeuristics, ViewState, ViewStateExt,
    ViewStates, ViewSystemExecutionError, ViewSystemIdentifier, ViewSystemRegistrator,
    ViewSystemState, ViewerDiagnostic, ViewerReportSeverity, VisualizabilityConstraints,
    VisualizerCollection, VisualizerComponentMappings, VisualizerComponentSource,
    VisualizerExecutionOutput, VisualizerInstruction, VisualizerInstructionReport,
    VisualizerInstructionsPerType, VisualizerQueryInfo, VisualizerReportContext, VisualizerSystem,
    VisualizerTypeReport, VisualizerViewReport, VisualizersSectionOutput, VisualizersSectionUi,
    default_view_entity_drop,
};
pub use self::viewer_context::ViewerContext;
pub use self::visitor_flow_control::VisitorControlFlow; // Historical reasons
pub use re_uri::TableReference;

pub mod external {
    #[cfg(not(target_arch = "wasm32"))]
    pub use tokio;
    pub use {
        nohash_hasher, re_chunk_store, re_entity_db, re_log_types, re_query, re_string_interner,
        re_tf, re_ui,
    };
}

// Re-export
pub use re_byte_size::SizeBytes;
pub use re_chunk_store::MissingChunkReporter;

// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NeedsRepaint {
    Yes,
    No,
}

impl NeedsRepaint {
    pub fn or(self, other: Self) -> Self {
        if self == Self::Yes || other == Self::Yes {
            Self::Yes
        } else {
            Self::No
        }
    }
}

// ---

/// Determines the icon to use for a given container kind.
#[inline]
pub fn icon_for_container_kind(kind: &egui_tiles::ContainerKind) -> &'static re_ui::Icon {
    match kind {
        egui_tiles::ContainerKind::Tabs => &re_ui::icons::CONTAINER_TABS,
        egui_tiles::ContainerKind::Horizontal => &re_ui::icons::CONTAINER_HORIZONTAL,
        egui_tiles::ContainerKind::Vertical => &re_ui::icons::CONTAINER_VERTICAL,
        egui_tiles::ContainerKind::Grid => &re_ui::icons::CONTAINER_GRID,
    }
}

/// Info given to egui when taking a screenshot.
///
/// Specified what we are screenshotting.
#[derive(Clone, Debug)]
pub struct ScreenshotInfo {
    /// What portion of the UI to take a screenshot of (in ui points).
    pub ui_rect: Option<egui::Rect>,
    pub pixels_per_point: f32,

    /// Name of the screenshot (e.g. view name), excluding file extension.
    pub name: String,

    /// Where to put the screenshot.
    pub target: ScreenshotTarget,

    /// Whether to show a user-facing notification (info toast) when the screenshot is done.
    pub notify: bool,
}

/// Where to put the screenshot.
#[derive(Clone, Debug)]
pub enum ScreenshotTarget {
    /// The screenshot will be copied to the clipboard.
    CopyToClipboard,

    /// The screenshot will be saved to disk (prompting the user for a location).
    SaveToPathFromFileDialog,

    /// The screenshot will be saved to the specified file path.
    SaveToPath {
        /// Where to save the image. The extension picks the format.
        path: camino::Utf8PathBuf,

        /// Longest side of the saved image, in pixels.
        /// A larger screenshot is scaled down to fit; `None` applies no limit.
        max_size: Option<u32>,

        /// Called once the file is written, or the screenshot failed.
        on_done:
            Option<re_log_channel::UiCallback<Result<(), re_log_channel::SaveScreenshotError>>>,
    },

    /// The screenshot will be encoded as a PNG, and handed to `on_done`.
    EncodePng {
        /// Longest side of the image, in pixels.
        /// A larger screenshot is scaled down to fit; `None` applies no limit.
        max_size: Option<u32>,

        /// Called with the PNG, or with why the screenshot failed.
        on_done: re_log_channel::UiCallback<Result<Vec<u8>, re_log_channel::SaveScreenshotError>>,
    },
}

impl ScreenshotTarget {
    /// Tell the caller waiting for this screenshot, if any, that it failed.
    pub fn report_failure(self, err: re_log_channel::SaveScreenshotError) {
        match self {
            Self::SaveToPath { on_done, .. } => {
                if let Some(on_done) = on_done {
                    on_done.call(Err(err));
                }
            }
            Self::EncodePng { on_done, .. } => on_done.call(Err(err)),
            Self::CopyToClipboard | Self::SaveToPathFromFileDialog => {}
        }
    }
}

// ----------------------------------------------------------------------------------------

/// Used to publish info aboutr each view.
///
/// We use this for view screenshotting.
///
/// Accessed with [`egui::Memory::caches`].
pub type ViewRectPublisher = egui::cache::FramePublisher<ViewId, PublishedViewInfo>;

/// Information about a view that is published each frame by [`ViewRectPublisher`].
#[derive(Clone, Debug)]
pub struct PublishedViewInfo {
    /// Human-readable name of the view.
    pub name: String,

    /// Where on screen (in ui coords).
    ///
    /// NOTE: this can include a highlighted border of the view.
    pub rect: egui::Rect,
}
