//! Rerun Viewer context
//!
//! This crate contains data structures that are shared with most modules of the viewer.

mod annotations;
mod app_options;
mod blueprint_helpers;
mod blueprint_id;
mod cache;
mod collapsed_id;
mod command_sender;
mod component_fallbacks;
mod component_ui_registry;
mod contents;
mod data_result_node_or_path;
mod drag_and_drop;
mod file_dialog;
mod image_info;
mod item;
mod maybe_mut_ref;
mod query_context;
mod query_range;
mod selection_state;
mod store_context;
pub mod store_hub;
mod tensor;
pub mod test_context; //TODO(ab): this should be behind #[cfg(test)], but then ` cargo clippy --all-targets` fails
mod time_control;
mod time_drag_value;
mod typed_entity_collections;
mod undo;
mod utils;
mod view;
mod viewer_context;

// TODO(andreas): Move to its own crate?
pub mod gpu_bridge;

pub use self::{
    annotations::{AnnotationMap, Annotations, ResolvedAnnotationInfo, ResolvedAnnotationInfos},
    app_options::AppOptions,
    blueprint_helpers::{blueprint_timeline, blueprint_timepoint_for_writes},
    blueprint_id::{BlueprintId, BlueprintIdRegistry, ContainerId, ViewId},
    cache::{Cache, Caches, ImageDecodeCache, ImageStatsCache, TensorStatsCache, VideoCache},
    collapsed_id::{CollapseItem, CollapseScope, CollapsedId},
    command_sender::{
        command_channel, CommandReceiver, CommandSender, SystemCommand, SystemCommandSender,
    },
    component_fallbacks::{
        ComponentFallbackError, ComponentFallbackProvider, ComponentFallbackProviderResult,
        TypedComponentFallbackProvider,
    },
    component_ui_registry::{ComponentUiRegistry, ComponentUiTypes, UiLayout},
    contents::{blueprint_id_to_tile_id, Contents, ContentsName},
    data_result_node_or_path::DataResultNodeOrPath,
    drag_and_drop::{DragAndDropFeedback, DragAndDropManager, DragAndDropPayload},
    file_dialog::santitize_file_name,
    image_info::{ColormapWithRange, ImageInfo},
    item::Item,
    maybe_mut_ref::MaybeMutRef,
    query_context::{
        DataQueryResult, DataResultHandle, DataResultNode, DataResultTree, QueryContext,
    },
    query_range::QueryRange,
    selection_state::{
        ApplicationSelectionState, HoverHighlight, InteractionHighlight, ItemCollection,
        ItemSpaceContext, SelectionHighlight,
    },
    store_context::StoreContext,
    store_hub::StoreHub,
    tensor::{ImageStats, TensorStats},
    time_control::{Looping, PlayState, TimeControl, TimeView},
    time_drag_value::TimeDragValue,
    typed_entity_collections::{
        ApplicableEntities, IndicatedEntities, PerVisualizer, VisualizableEntities,
    },
    undo::BlueprintUndoState,
    utils::{auto_color_egui, auto_color_for_entity_path, level_to_rich_text},
    view::{
        DataResult, IdentifiedViewSystem, OptionalViewEntityHighlight, OverridePath,
        PerSystemDataResults, PerSystemEntities, PropertyOverrides, RecommendedView,
        SmallVisualizerSet, SystemExecutionOutput, ViewClass, ViewClassExt,
        ViewClassLayoutPriority, ViewClassRegistry, ViewClassRegistryError, ViewContext,
        ViewContextCollection, ViewContextSystem, ViewEntityHighlight, ViewHighlights,
        ViewOutlineMasks, ViewQuery, ViewSpawnHeuristics, ViewState, ViewStateExt, ViewStates,
        ViewSystemExecutionError, ViewSystemIdentifier, ViewSystemRegistrator,
        VisualizableFilterContext, VisualizerAdditionalApplicabilityFilter, VisualizerCollection,
        VisualizerQueryInfo, VisualizerSystem,
    },
    viewer_context::{RecordingConfig, ViewerContext},
};

#[cfg(not(target_arch = "wasm32"))]
mod clipboard;

#[cfg(not(target_arch = "wasm32"))]
pub use clipboard::Clipboard;

pub mod external {
    pub use nohash_hasher;
    pub use {re_chunk_store, re_entity_db, re_log_types, re_query, re_ui};
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenshotTarget {
    /// The screenshot will be copied to the clipboard.
    #[cfg(not(target_arch = "wasm32"))] // TODO(#8264): copy-to-screenshot on web
    CopyToClipboard,

    /// The screenshot will be saved to disk.
    SaveToDisk,
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
