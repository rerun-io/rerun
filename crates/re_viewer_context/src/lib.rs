//! Rerun Viewer context
//!
//! This crate contains data structures that are shared with most modules of the viewer.

mod annotations;
mod app_options;
mod blueprint_helpers;
mod blueprint_id;
mod caches;
mod collapsed_id;
mod command_sender;
mod component_ui_registry;
mod item;
mod query_context;
mod selection_history;
mod selection_state;
mod space_view;
mod store_context;
mod tensor;
mod time_control;
mod typed_entity_collections;
mod utils;
mod viewer_context;

// TODO(andreas): Move to its own crate?
pub mod gpu_bridge;

pub use annotations::{
    AnnotationMap, Annotations, ResolvedAnnotationInfo, ResolvedAnnotationInfos,
};
pub use app_options::AppOptions;
pub use blueprint_helpers::{blueprint_timeline, blueprint_timepoint_for_writes};
pub use blueprint_id::{BlueprintId, BlueprintIdRegistry, ContainerId, DataQueryId, SpaceViewId};
pub use caches::{Cache, Caches};
pub use collapsed_id::{BlueprintCollapsedId, CollapsedId, CollapsedIdScope, StreamsCollapsedId};
pub use command_sender::{
    command_channel, CommandReceiver, CommandSender, SystemCommand, SystemCommandSender,
};
pub use component_ui_registry::{ComponentUiRegistry, UiVerbosity};
pub use item::Item;
pub use query_context::{DataQueryResult, DataResultHandle, DataResultNode, DataResultTree};
pub use selection_history::SelectionHistory;
pub use selection_state::{
    ApplicationSelectionState, HoverHighlight, InteractionHighlight, SelectedSpaceContext,
    Selection, SelectionHighlight,
};
pub use space_view::{
    DataResult, DynSpaceViewClass, IdentifiedViewSystem, PerSystemDataResults, PerSystemEntities,
    PropertyOverrides, RecommendedSpaceView, SmallVisualizerSet, SpaceViewClass,
    SpaceViewClassIdentifier, SpaceViewClassLayoutPriority, SpaceViewClassRegistry,
    SpaceViewClassRegistryError, SpaceViewEntityHighlight, SpaceViewHighlights,
    SpaceViewOutlineMasks, SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewSystemExecutionError,
    SpaceViewSystemRegistrator, SystemExecutionOutput, ViewContextCollection, ViewContextSystem,
    ViewQuery, ViewSystemIdentifier, VisualizableFilterContext,
    VisualizerAdditionalApplicabilityFilter, VisualizerCollection, VisualizerQueryInfo,
    VisualizerSystem,
};
pub use store_context::StoreContext;
pub use tensor::{TensorDecodeCache, TensorStats, TensorStatsCache};
pub use time_control::{Looping, PlayState, TimeControl, TimeView};
pub use typed_entity_collections::{
    ApplicableEntities, IndicatedEntities, PerVisualizer, VisualizableEntities,
};
pub use utils::{auto_color, level_to_rich_text, DefaultColor};
pub use viewer_context::{RecordingConfig, ViewerContext};

#[cfg(not(target_arch = "wasm32"))]
mod clipboard;
#[cfg(not(target_arch = "wasm32"))]
pub use clipboard::Clipboard;

pub mod external {
    pub use nohash_hasher;
    pub use {re_data_store, re_entity_db, re_log_types, re_query, re_ui};
}

// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NeedsRepaint {
    Yes,
    No,
}
