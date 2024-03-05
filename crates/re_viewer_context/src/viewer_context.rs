use ahash::HashMap;
use parking_lot::RwLock;

use re_data_store::LatestAtQuery;
use re_entity_db::entity_db::EntityDb;

use crate::{
    query_context::DataQueryResult, AppOptions, ApplicableEntities, ApplicationSelectionState,
    Caches, CommandSender, ComponentUiRegistry, DataQueryId, IndicatedEntities, PerVisualizer,
    Selection, SpaceViewClassRegistry, StoreContext, SystemCommandSender as _, TimeControl,
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
    pub entity_db: &'a EntityDb,

    /// The current view of the store
    pub store_context: &'a StoreContext<'a>,

    /// Mapping from class and system to entities for the store
    ///
    /// TODO(andreas): This should have a generation id, allowing to update heuristics(?)/visualizable_entities etc.
    pub applicable_entities_per_visualizer: &'a PerVisualizer<ApplicableEntities>,

    /// For each visualizer, the set of entities that have at least one matching indicator component.
    ///
    /// TODO(andreas): Should we always do the intersection with `applicable_entities_per_visualizer`
    ///                 or are we ever interested in a non-applicable but indicator-matching entity?
    pub indicated_entities_per_visualizer: &'a PerVisualizer<IndicatedEntities>,

    /// All the query results for this frame
    pub query_results: &'a HashMap<DataQueryId, DataQueryResult>, // TODO: space view id

    /// UI config for the current recording (found in [`EntityDb`]).
    pub rec_cfg: &'a RecordingConfig,

    /// UI config for the current blueprint.
    pub blueprint_cfg: &'a RecordingConfig,

    /// The blueprint query used for resolving blueprint in this frame
    pub blueprint_query: &'a LatestAtQuery,

    /// The look and feel of the UI.
    pub re_ui: &'a re_ui::ReUi,

    /// The global `re_renderer` context, holds on to all GPU resources.
    pub render_ctx: &'a re_renderer::RenderContext,

    /// Interface for sending commands back to the app
    pub command_sender: &'a CommandSender,

    /// Item that got focused on the last frame if any.
    ///
    /// The focused item is cleared every frame, but views may react with side-effects
    /// that last several frames.
    pub focused_item: &'a Option<crate::Item>,
}

impl<'a> ViewerContext<'a> {
    /// Returns the current selection.
    pub fn selection(&self) -> &Selection {
        self.rec_cfg.selection_state.current()
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &Selection {
        self.rec_cfg.selection_state.hovered()
    }

    pub fn selection_state(&self) -> &ApplicationSelectionState {
        &self.rec_cfg.selection_state
    }

    /// The current time query, based on the current time control.
    pub fn current_query(&self) -> re_data_store::LatestAtQuery {
        self.rec_cfg.time_ctrl.read().current_query()
    }

    /// Set hover/select/focus for a given selection based on an egui response.
    pub fn select_hovered_on_click(
        &self,
        response: &egui::Response,
        selection: impl Into<Selection>,
    ) {
        re_tracing::profile_function!();

        let mut selection = selection.into();
        selection.resolve_mono_instance_path_items(self);
        let selection_state = self.selection_state();

        if response.hovered() {
            selection_state.set_hovered(selection.clone());
        }

        if response.double_clicked() {
            if let Some((item, _)) = selection.first() {
                self.command_sender
                    .send_system(crate::SystemCommand::SetFocus(item.clone()));
            }
        }

        if response.clicked() {
            if response.ctx.input(|i| i.modifiers.command) {
                selection_state.toggle_selection(selection);
            } else {
                selection_state.set_selection(selection);
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// UI config for the current recording (found in [`EntityDb`]).
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct RecordingConfig {
    /// The current time of the time panel, how fast it is moving, etc.
    pub time_ctrl: RwLock<TimeControl>,

    /// Selection & hovering state.
    pub selection_state: ApplicationSelectionState,
}
