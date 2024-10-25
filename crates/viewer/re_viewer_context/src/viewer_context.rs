use ahash::HashMap;
use parking_lot::RwLock;

use re_chunk_store::LatestAtQuery;
use re_entity_db::entity_db::EntityDb;

use crate::{
    query_context::DataQueryResult, AppOptions, ApplicableEntities, ApplicationSelectionState,
    Caches, CommandSender, ComponentUiRegistry, IndicatedEntities, ItemCollection, PerVisualizer,
    SpaceViewClassRegistry, SpaceViewId, StoreContext, SystemCommandSender as _, TimeControl,
};

/// Common things needed by many parts of the viewer.
pub struct ViewerContext<'a> {
    /// Global options for the whole viewer.
    pub app_options: &'a AppOptions,

    /// Things that need caching and are shared across the whole viewer.
    ///
    /// Use this only for things that you expected be shared across different panels and/or space views.
    pub cache: &'a Caches,

    /// Runtime info about components and archetypes.
    ///
    /// The component placeholder values for components are to be used when [`crate::ComponentFallbackProvider::try_provide_fallback`]
    /// is not able to provide a value.
    ///
    /// ⚠️ In almost all cases you should not use this directly, but instead use the currently best fitting
    /// [`crate::ComponentFallbackProvider`] and call [`crate::ComponentFallbackProvider::fallback_for`] instead.
    pub reflection: &'a re_types_core::reflection::Reflection,

    /// How to display components.
    pub component_ui_registry: &'a ComponentUiRegistry,

    /// Registry of all known classes of space views.
    pub space_view_class_registry: &'a SpaceViewClassRegistry,

    /// The current view of the store
    pub store_context: &'a StoreContext<'a>,

    /// Mapping from class and system to entities for the store
    ///
    /// TODO(andreas): This should have a generation id, allowing to update heuristics(?)/visualizable entities etc.
    pub applicable_entities_per_visualizer: &'a PerVisualizer<ApplicableEntities>,

    /// For each visualizer, the set of entities that have at least one matching indicator component.
    ///
    /// TODO(andreas): Should we always do the intersection with `applicable_entities_per_visualizer`
    ///                 or are we ever interested in a non-applicable but indicator-matching entity?
    pub indicated_entities_per_visualizer: &'a PerVisualizer<IndicatedEntities>,

    /// All the query results for this frame.
    pub query_results: &'a HashMap<SpaceViewId, DataQueryResult>,

    /// UI config for the current recording (found in [`EntityDb`]).
    pub rec_cfg: &'a RecordingConfig,

    /// UI config for the current blueprint.
    pub blueprint_cfg: &'a RecordingConfig,

    /// Selection & hovering state.
    pub selection_state: &'a ApplicationSelectionState,

    /// The blueprint query used for resolving blueprint in this frame
    pub blueprint_query: &'a LatestAtQuery,

    /// The [`egui::Context`].
    pub egui_ctx: &'a egui::Context,

    /// The global `re_renderer` context, holds on to all GPU resources.
    pub render_ctx: Option<&'a re_renderer::RenderContext>,

    /// Interface for sending commands back to the app
    pub command_sender: &'a CommandSender,

    /// Item that got focused on the last frame if any.
    ///
    /// The focused item is cleared every frame, but views may react with side-effects
    /// that last several frames.
    pub focused_item: &'a Option<crate::Item>,
}

impl<'a> ViewerContext<'a> {
    /// The active recording.
    #[inline]
    pub fn recording(&self) -> &EntityDb {
        self.store_context.recording
    }

    /// The active blueprint.
    #[inline]
    pub fn blueprint_db(&self) -> &re_entity_db::EntityDb {
        self.store_context.blueprint
    }

    /// The chunk store of the active recording.
    #[inline]
    pub fn recording_store(&self) -> &re_chunk_store::ChunkStore {
        self.store_context.recording.store()
    }

    /// The chunk store of the active blueprint.
    #[inline]
    pub fn blueprint_store(&self) -> &re_chunk_store::ChunkStore {
        self.store_context.blueprint.store()
    }

    /// The `StoreId` of the active recording.
    #[inline]
    pub fn recording_id(&self) -> &re_log_types::StoreId {
        self.store_context.recording.store_id()
    }

    /// Returns the current selection.
    pub fn selection(&self) -> &ItemCollection {
        self.selection_state.selected_items()
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &ItemCollection {
        self.selection_state.hovered_items()
    }

    pub fn selection_state(&self) -> &ApplicationSelectionState {
        self.selection_state
    }

    /// The current time query, based on the current time control.
    pub fn current_query(&self) -> re_chunk_store::LatestAtQuery {
        self.rec_cfg.time_ctrl.read().current_query()
    }

    /// Set hover/select/focus for a given selection based on an egui response.
    pub fn select_hovered_on_click(
        &self,
        response: &egui::Response,
        selection: impl Into<ItemCollection>,
    ) {
        re_tracing::profile_function!();

        let selection = selection.into().into_mono_instance_path_items(self);
        let selection_state = self.selection_state();

        if response.hovered() {
            selection_state.set_hovered(selection.clone());
        }

        if response.double_clicked() {
            if let Some(item) = selection.first_item() {
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

    /// Returns a placeholder value for a given component, solely identified by its name.
    ///
    /// A placeholder is an array of the component type with a single element which takes on some default value.
    /// It can be set as part of the reflection information, see [`re_types_core::reflection::ComponentReflection::custom_placeholder`].
    /// Note that automatically generated placeholders ignore any extension types.
    ///
    /// This requires the component name to be known by either datastore or blueprint store and
    /// will return a placeholder for a nulltype otherwise, logging an error.
    /// The rationale is that to get into this situation, we need to know of a component name for which
    /// we don't have a datatype, meaning that we can't make any statement about what data this component should represent.
    // TODO(andreas): Are there cases where this is expected and how to handle this?
    pub fn placeholder_for(
        &self,
        component: re_chunk::ComponentName,
    ) -> Box<dyn re_chunk::ArrowArray> {
        self.reflection.components.get(&component).and_then(|info| info.custom_placeholder.as_ref()).cloned()

        .unwrap_or_else(||
            {
                // TODO(andreas): Is this operation common enough to cache the result? If so, here or in the reflection data?
                // The nice thing about this would be that we could always give out references (but updating said cache wouldn't be easy in that case).
        let datatype = self
        .recording_store()
        .lookup_datatype(&component)
        .or_else(|| self.blueprint_store().lookup_datatype(&component))
        .unwrap_or_else(|| {
            re_log::error_once!("Could not find datatype for component {component}. Using null array as placeholder.");
            &re_chunk::external::arrow2::datatypes::DataType::Null
        });
            re_types::reflection::generic_placeholder_for_datatype(datatype)
    })
    }
}

// ----------------------------------------------------------------------------

/// UI config for the current recording (found in [`EntityDb`]).
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct RecordingConfig {
    /// The current time of the time panel, how fast it is moving, etc.
    pub time_ctrl: RwLock<TimeControl>,
}
