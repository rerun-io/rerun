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
mod file_dialog;
mod image_info;
mod item;
mod maybe_mut_ref;
mod query_context;
mod query_range;
mod selection_history;
mod selection_state;
mod space_view;
mod store_context;
pub mod store_hub;
mod tensor;
pub mod test_context; //TODO(ab): this should be behind #[cfg(test)], but then ` cargo clippy --all-targets` fails
mod time_control;
mod time_drag_value;
mod typed_entity_collections;
mod undo;
mod utils;
mod viewer_context;

// TODO(andreas): Move to its own crate?
pub mod gpu_bridge;

pub use annotations::{
    AnnotationMap, Annotations, ResolvedAnnotationInfo, ResolvedAnnotationInfos,
};
pub use app_options::AppOptions;
pub use blueprint_helpers::{blueprint_timeline, blueprint_timepoint_for_writes};
pub use blueprint_id::{BlueprintId, BlueprintIdRegistry, ContainerId, SpaceViewId};
pub use cache::{Cache, Caches, ImageDecodeCache, ImageStatsCache, TensorStatsCache, VideoCache};
pub use collapsed_id::{CollapseItem, CollapseScope, CollapsedId};
pub use command_sender::{
    command_channel, CommandReceiver, CommandSender, SystemCommand, SystemCommandSender,
};
pub use component_fallbacks::{
    ComponentFallbackError, ComponentFallbackProvider, ComponentFallbackProviderResult,
    TypedComponentFallbackProvider,
};
pub use component_ui_registry::{ComponentUiRegistry, ComponentUiTypes, UiLayout};
pub use contents::{blueprint_id_to_tile_id, Contents, ContentsName};
pub use image_info::{ColormapWithRange, ImageInfo};
pub use item::Item;
pub use maybe_mut_ref::MaybeMutRef;
pub use query_context::{
    DataQueryResult, DataResultHandle, DataResultNode, DataResultTree, QueryContext,
};
pub use query_range::QueryRange;
pub use selection_history::SelectionHistory;
pub use selection_state::{
    ApplicationSelectionState, HoverHighlight, InteractionHighlight, ItemCollection,
    ItemSpaceContext, SelectionHighlight,
};
pub use space_view::{
    DataResult, IdentifiedViewSystem, OptionalSpaceViewEntityHighlight, OverridePath,
    PerSystemDataResults, PerSystemEntities, PropertyOverrides, RecommendedSpaceView,
    SmallVisualizerSet, SpaceViewClass, SpaceViewClassExt, SpaceViewClassLayoutPriority,
    SpaceViewClassRegistry, SpaceViewClassRegistryError, SpaceViewEntityHighlight,
    SpaceViewHighlights, SpaceViewOutlineMasks, SpaceViewSpawnHeuristics, SpaceViewState,
    SpaceViewStateExt, SpaceViewSystemExecutionError, SpaceViewSystemRegistrator,
    SystemExecutionOutput, ViewContext, ViewContextCollection, ViewContextSystem, ViewQuery,
    ViewStates, ViewSystemIdentifier, VisualizableFilterContext,
    VisualizerAdditionalApplicabilityFilter, VisualizerCollection, VisualizerQueryInfo,
    VisualizerSystem,
};
pub use store_context::StoreContext;
pub use store_hub::StoreHub;
pub use tensor::{ImageStats, TensorStats};
pub use time_control::{Looping, PlayState, TimeControl, TimeView};
pub use time_drag_value::TimeDragValue;
pub use typed_entity_collections::{
    ApplicableEntities, IndicatedEntities, PerVisualizer, VisualizableEntities,
};
pub use undo::BlueprintUndoState;
pub use utils::{auto_color_egui, auto_color_for_entity_path, level_to_rich_text};
pub use viewer_context::{RecordingConfig, ViewerContext};

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

/// The style to use for displaying this space view name in the UI.
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
    pub space_view: Option<SpaceViewId>,
    pub ui_rect: Option<egui::Rect>,
    pub pixels_per_point: f32,
}
