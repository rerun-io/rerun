//! Rerun Viewer context
//!
//! This crate contains data structures that are shared with most modules of the viewer.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod annotations;
mod app_options;
mod async_runtime_handle;
mod blueprint_helpers;
mod blueprint_id;
mod cache;
mod collapsed_id;
mod command_sender;
mod component_fallbacks;
mod component_ui_registry;
mod contents;
mod display_mode;
mod drag_and_drop;
mod file_dialog;
mod global_context;
mod heuristics;
mod image_info;
mod item;
mod item_collection;
mod maybe_mut_ref;
pub mod open_url;
mod query_context;
mod query_range;
mod recording_or_table;
mod selection_state;
mod storage_context;
mod store_context;
pub mod store_hub;
mod tables;
mod tensor;
mod time_control;
mod typed_entity_collections;
mod undo;
mod utils;
mod view;
mod viewer_context;

// TODO(andreas): Move to its own crate?
pub mod gpu_bridge;
mod visitor_flow_control;

pub use re_ui::UiLayout;

pub use self::annotations::{
    AnnotationContextStoreSubscriber, AnnotationMap, Annotations, ResolvedAnnotationInfo,
    ResolvedAnnotationInfos,
};
pub use self::app_options::{AppOptions, ExperimentalAppOptions, VideoOptions};
pub use self::async_runtime_handle::{AsyncRuntimeError, AsyncRuntimeHandle, WasmNotSend};
pub use self::blueprint_helpers::{
    BlueprintContext, blueprint_timeline, blueprint_timepoint_for_writes,
};
pub use self::blueprint_id::{
    BlueprintId, BlueprintIdRegistry, ContainerId, GLOBAL_VIEW_ID, ViewId,
};
pub use self::cache::{
    Cache, Caches, ImageDecodeCache, ImageStatsCache, SharablePlayableVideoStream,
    TensorStatsCache, TransformDatabaseStoreCache, VideoAssetCache, VideoStreamCache,
    VideoStreamProcessingError,
};
pub use self::collapsed_id::{CollapseItem, CollapseScope, CollapsedId};
pub use self::command_sender::{
    CommandReceiver, CommandSender, EditRedapServerModalCommand, SystemCommand,
    SystemCommandSender, command_channel,
};
pub use self::component_fallbacks::{
    ComponentFallbackError, FallbackProviderRegistry, typed_fallback_for,
};
pub use self::component_ui_registry::{
    ComponentUiRegistry, ComponentUiTypes, EditTarget, VariantName,
};
pub use self::contents::{Contents, ContentsName, blueprint_id_to_tile_id};
pub use self::display_mode::DisplayMode;
pub use self::drag_and_drop::{DragAndDropFeedback, DragAndDropManager, DragAndDropPayload};
pub use self::file_dialog::sanitize_file_name;
pub use self::global_context::{AuthContext, GlobalContext};
pub use self::heuristics::suggest_view_for_each_entity;
pub use self::image_info::{
    ColormapWithRange, ImageInfo, StoredBlobCacheKey, resolution_of_image_at,
};
pub use self::item::{Item, resolve_mono_instance_path, resolve_mono_instance_path_item};
pub use self::item_collection::{ItemCollection, ItemContext};
pub use self::maybe_mut_ref::MaybeMutRef;
pub use self::query_context::{
    DataQueryResult, DataResultHandle, DataResultNode, DataResultTree, QueryContext,
};
pub use self::query_range::QueryRange;
pub use self::recording_or_table::RecordingOrTable;
pub use self::selection_state::{
    ApplicationSelectionState, HoverHighlight, InteractionHighlight, SelectionChange,
    SelectionHighlight,
};
pub use self::storage_context::StorageContext;
pub use self::store_context::StoreContext;
pub use self::store_hub::StoreHub;
pub use self::tables::{TableStore, TableStores};
pub use self::tensor::{ImageStats, TensorStats};
pub use self::time_control::{
    TIME_PANEL_PATH, TimeControl, TimeControlCommand, TimeControlResponse, TimeView,
    time_panel_blueprint_entity_path,
};
pub use self::typed_entity_collections::{
    DatatypeMatchInfo, DatatypeMatchKind, IndicatedEntities, PerVisualizerInstruction,
    PerVisualizerType, PerVisualizerTypeInViewClass, VisualizableEntities, VisualizableReason,
};
pub use self::undo::BlueprintUndoState;
pub use self::utils::{
    auto_color_egui, auto_color_for_entity_path, level_to_rich_text, video_stream_time_from_query,
    video_timestamp_component_to_video_time,
};
pub use self::view::{
    AnyPhysicalDatatypeRequirement, DataResult, IdentifiedViewSystem, OptionalViewEntityHighlight,
    PerSystemDataResults, PerSystemEntities, RecommendedView, RecommendedVisualizers,
    RequiredComponents, SystemExecutionOutput, ViewClass, ViewClassExt, ViewClassLayoutPriority,
    ViewClassPlaceholder, ViewClassRegistry, ViewClassRegistryError, ViewContext,
    ViewContextCollection, ViewContextSystem, ViewContextSystemOncePerFrameResult,
    ViewEntityHighlight, ViewHighlights, ViewOutlineMasks, ViewQuery, ViewSpawnHeuristics,
    ViewState, ViewStateExt, ViewStates, ViewSystemExecutionError, ViewSystemIdentifier,
    ViewSystemRegistrator, VisualizerCollection, VisualizerComponentMappings,
    VisualizerComponentSource, VisualizerExecutionErrorState, VisualizerExecutionOutput,
    VisualizerInstruction, VisualizerQueryInfo, VisualizerSystem,
};
pub use self::viewer_context::ViewerContext;
pub use self::visitor_flow_control::VisitorControlFlow; // Historical reasons

pub mod external {
    #[cfg(not(target_arch = "wasm32"))]
    pub use tokio;
    pub use {nohash_hasher, re_chunk_store, re_entity_db, re_log_types, re_query, re_ui};
}

// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NeedsRepaint {
    Yes,
    No,
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

/// The style to use for displaying this view name in the UI.
pub fn contents_name_style(name: &ContentsName) -> re_ui::LabelStyle {
    match name {
        ContentsName::Named(_) => re_ui::LabelStyle::Normal,
        ContentsName::Placeholder(_) => re_ui::LabelStyle::Unnamed,
    }
}

/// Info given to egui when taking a screenshot.
///
/// Specified what we are screenshotting.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenshotInfo {
    /// What portion of the UI to take a screenshot of (in ui points).
    pub ui_rect: Option<egui::Rect>,
    pub pixels_per_point: f32,

    /// Name of the screenshot (e.g. view name), excluding file extension.
    pub name: String,

    /// Where to put the screenshot.
    pub target: ScreenshotTarget,
}

/// Where to put the screenshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScreenshotTarget {
    /// The screenshot will be copied to the clipboard.
    CopyToClipboard,

    /// The screenshot will be saved to disk (prompting the user for a location).
    SaveToPathFromFileDialog,

    /// The screenshot will be saved to the specified file path.
    SaveToPath(camino::Utf8PathBuf),
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
