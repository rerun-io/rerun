//! Rerun Viewer context
//!
//! This crate contains data structures that are shared with most modules of the viewer.

mod annotations;
mod app_options;
mod blueprint_helpers;
mod blueprint_id;
mod caches;
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
mod utils;
mod viewer_context;

// TODO(andreas): Move to its own crate?
pub mod gpu_bridge;

pub use annotations::{
    AnnotationMap, Annotations, ResolvedAnnotationInfo, ResolvedAnnotationInfos,
};
pub use app_options::AppOptions;
pub use blueprint_id::{BlueprintId, BlueprintIdRegistry, DataQueryId, SpaceViewId};
pub use caches::{Cache, Caches};
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
    AutoSpawnHeuristic, DataResult, DynSpaceViewClass, HeuristicFilterContext,
    IdentifiedViewSystem, PerSystemDataResults, PerSystemEntities, PropertyOverrides,
    SpaceViewClass, SpaceViewClassIdentifier, SpaceViewClassLayoutPriority, SpaceViewClassRegistry,
    SpaceViewClassRegistryError, SpaceViewEntityHighlight, SpaceViewHighlights,
    SpaceViewOutlineMasks, SpaceViewState, SpaceViewSystemExecutionError,
    SpaceViewSystemRegistrator, SystemExecutionOutput, ViewContextCollection, ViewContextSystem,
    ViewPartCollection, ViewPartSystem, ViewQuery, ViewSystemIdentifier,
    VisualizerAdditionalApplicabilityFilter,
};
pub use store_context::StoreContext;
pub use tensor::{TensorDecodeCache, TensorStats, TensorStatsCache};
pub use time_control::{Looping, PlayState, TimeControl, TimeView};
pub use utils::{auto_color, level_to_rich_text, DefaultColor};
pub use viewer_context::{RecordingConfig, ViewerContext};

#[cfg(not(target_arch = "wasm32"))]
mod clipboard;
#[cfg(not(target_arch = "wasm32"))]
pub use clipboard::Clipboard;

pub mod external {
    pub use nohash_hasher;
    pub use {re_arrow_store, re_data_store, re_log_types, re_query, re_ui};
}

// ---------------------------------------------------------------------------

use nohash_hasher::{IntMap, IntSet};
use re_log_types::EntityPath;

pub type EntitiesPerSystem = IntMap<ViewSystemIdentifier, IntSet<EntityPath>>;

pub type EntitiesPerSystemPerClass = IntMap<SpaceViewClassIdentifier, EntitiesPerSystem>;

slotmap::new_key_type! {
    /// Identifier for a blueprint group.
    pub struct DataBlueprintGroupHandle;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NeedsRepaint {
    Yes,
    No,
}
