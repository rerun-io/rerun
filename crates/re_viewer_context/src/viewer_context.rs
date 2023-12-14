use ahash::HashMap;
use parking_lot::RwLock;

use re_data_store::{store_db::StoreDb, EntityTree, TimeHistogramPerTimeline};

use crate::{
    item::resolve_mono_instance_path_item, query_context::DataQueryResult, AppOptions, Caches,
    CommandSender, ComponentUiRegistry, DataQueryId, EntitiesPerSystemPerClass, Item,
    ItemCollection, SelectionState, SpaceViewClassRegistry, StoreContext, TimeControl,
};

/// Common things needed by many parts of the viewer.
pub struct ViewerContext<'a> {
    /// Global options for the whole viewer.
    pub app_options: &'a AppOptions,

    /// Things that need caching and are shared across the whole viewer.
    ///
    /// Use this only for things that you expected be shared across different panels and/or space views.
    pub cache: &'a Caches,

    /// How to display components.
    pub component_ui_registry: &'a ComponentUiRegistry,

    /// Registry of all known classes of space views.
    pub space_view_class_registry: &'a SpaceViewClassRegistry,

    /// The current recording.
    /// TODO(jleibs): This can go away
    pub store_db: &'a StoreDb,

    /// The current view of the store
    pub store_context: &'a StoreContext<'a>,

    /// Mapping from class and system to entities for the store
    pub entities_per_system_per_class: &'a EntitiesPerSystemPerClass,

    /// All the query results for this frame
    pub query_results: &'a HashMap<DataQueryId, DataQueryResult>,

    /// UI config for the current recording (found in [`StoreDb`]).
    pub rec_cfg: &'a RecordingConfig,

    /// The look and feel of the UI.
    pub re_ui: &'a re_ui::ReUi,

    /// The global `re_renderer` context, holds on to all GPU resources.
    pub render_ctx: &'a re_renderer::RenderContext,

    /// Interface for sending commands back to the app
    pub command_sender: &'a CommandSender,
}

impl<'a> ViewerContext<'a> {
    /// Sets a single selection on the next frame, updating history as needed.
    pub fn set_single_selection(&self, item: &Item) {
        self.rec_cfg
            .selection_state
            .set_single_selection(resolve_mono_instance_path_item(
                &self.rec_cfg.time_ctrl.read().current_query(),
                self.store_db.store(),
                item,
            ));
    }

    /// Returns the current selection.
    pub fn selection(&self) -> &ItemCollection {
        self.rec_cfg.selection_state.current()
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &ItemCollection {
        self.rec_cfg.selection_state.hovered()
    }

    /// Set the hovered objects. Will be in [`Self::hovered`] on the next frame.
    pub fn set_hovered<'b>(&self, hovered: impl Iterator<Item = &'b Item>) {
        self.rec_cfg
            .selection_state
            .set_hovered(hovered.map(|item| {
                resolve_mono_instance_path_item(
                    &self.rec_cfg.time_ctrl.read().current_query(),
                    self.store_db.store(),
                    item,
                )
            }));
    }

    pub fn selection_state(&self) -> &SelectionState {
        &self.rec_cfg.selection_state
    }

    /// The current time query, based on the current time control.
    pub fn current_query(&self) -> re_arrow_store::LatestAtQuery {
        self.rec_cfg.time_ctrl.read().current_query()
    }

    /// Returns whether the given tree has any data logged in the current timeline,
    /// or has any timeless messages.
    pub fn tree_has_data_in_current_timeline(&self, tree: &EntityTree) -> bool {
        let top_time_histogram = &tree.subtree.time_histogram;
        top_time_histogram.has_timeline(self.rec_cfg.time_ctrl.read().timeline())
            || top_time_histogram.num_timeless_messages() > 0
    }

    /// Returns whether the given component has any data logged in the current timeline.
    pub fn component_has_data_in_current_timeline(
        &self,
        component_stat: &TimeHistogramPerTimeline,
    ) -> bool {
        component_stat.has_timeline(self.rec_cfg.time_ctrl.read().timeline())
            || component_stat.num_timeless_messages() > 0
    }
}

// ----------------------------------------------------------------------------

/// UI config for the current recording (found in [`StoreDb`]).
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct RecordingConfig {
    /// The current time of the time panel, how fast it is moving, etc.
    pub time_ctrl: RwLock<TimeControl>,

    /// Selection & hovering state.
    pub selection_state: SelectionState,
}
