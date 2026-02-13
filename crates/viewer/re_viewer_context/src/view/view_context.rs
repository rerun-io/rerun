use re_chunk_store::LatestAtQuery;
use re_log_types::{EntityPath, TimePoint};
use re_query::StorageEngineReadGuard;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_sdk_types::{AsComponents, ComponentBatch, ComponentDescriptor, ViewClassIdentifier};

use super::VisualizerCollection;
use crate::blueprint_helpers::BlueprintContext as _;
use crate::{DataQueryResult, DataResult, QueryContext, ViewId};

/// The context associated with a view.
///
/// This combines our [`crate::ViewerContext`] with [`crate::ViewState`]
/// and other view-specific information. This is used as the interface for
/// execution of view systems and selection panel UI elements that happen
/// within the context of a view to simplify plumbing of the necessary
/// information to resolve a query with possible overrides and fallback values.
pub struct ViewContext<'a> {
    pub viewer_ctx: &'a crate::ViewerContext<'a>,
    pub view_id: ViewId,
    pub view_class_identifier: ViewClassIdentifier,
    // TODO(RR-3076): Eventually we want to get rid of the _general_ concept of `space_origin`.
    // Until then, we make this available here so that fallback providers have access to it.
    pub space_origin: &'a EntityPath,
    pub view_state: &'a dyn crate::ViewState,
    pub query_result: &'a DataQueryResult,
}

impl<'a> ViewContext<'a> {
    #[inline]
    pub fn query_context(
        &'a self,
        data_result: &'a DataResult,
        query: LatestAtQuery,
        instruction_id: VisualizerInstructionId,
    ) -> QueryContext<'a> {
        QueryContext {
            view_ctx: self,
            target_entity_path: &data_result.entity_path,
            instruction_id: instruction_id.into(),
            archetype_name: None,
            query,
        }
    }

    #[inline]
    pub fn query_context_without_visualizer(
        &'a self,
        data_result: &'a DataResult,
        query: LatestAtQuery,
    ) -> QueryContext<'a> {
        QueryContext {
            view_ctx: self,
            target_entity_path: &data_result.entity_path,
            instruction_id: None,
            archetype_name: None,
            query,
        }
    }

    #[inline]
    pub fn render_ctx(&self) -> &re_renderer::RenderContext {
        self.viewer_ctx.global_context.render_ctx
    }

    #[inline]
    pub fn egui_ctx(&self) -> &egui::Context {
        self.viewer_ctx.global_context.egui_ctx
    }

    pub fn tokens(&self) -> &'static re_ui::DesignTokens {
        self.viewer_ctx.tokens()
    }

    /// The active recording.
    #[inline]
    pub fn recording(&self) -> &re_entity_db::EntityDb {
        self.viewer_ctx.recording()
    }

    /// The `StorageEngine` for the active recording.
    #[inline]
    pub fn recording_engine(&self) -> StorageEngineReadGuard<'_> {
        self.viewer_ctx.recording_engine()
    }

    /// The active blueprint.
    #[inline]
    pub fn blueprint_db(&self) -> &re_entity_db::EntityDb {
        self.viewer_ctx.blueprint_db()
    }

    /// The blueprint query used for resolving blueprint in this frame
    #[inline]
    pub fn blueprint_query(&self) -> &LatestAtQuery {
        self.viewer_ctx.blueprint_query
    }

    /// The `StoreId` of the active recording.
    #[inline]
    pub fn store_id(&self) -> &re_log_types::StoreId {
        self.viewer_ctx.store_id()
    }

    /// Returns the current selection.
    #[inline]
    pub fn selection(&self) -> &crate::ItemCollection {
        self.viewer_ctx.selection()
    }

    /// Returns the currently hovered objects.
    #[inline]
    pub fn hovered(&self) -> &crate::ItemCollection {
        self.viewer_ctx.hovered()
    }

    #[inline]
    pub fn selection_state(&self) -> &crate::ApplicationSelectionState {
        self.viewer_ctx.selection_state()
    }

    /// The current time query, based on the current time control.
    #[inline]
    pub fn current_query(&self) -> LatestAtQuery {
        self.viewer_ctx.current_query()
    }

    #[inline]
    pub fn lookup_query_result(&self, id: ViewId) -> &DataQueryResult {
        self.viewer_ctx.lookup_query_result(id)
    }

    #[inline]
    pub fn save_blueprint_array(
        &self,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
        array: arrow::array::ArrayRef,
    ) {
        self.viewer_ctx
            .save_blueprint_array(entity_path, component_descr, array);
    }

    #[inline]
    pub fn save_blueprint_archetype(&self, entity_path: EntityPath, components: &dyn AsComponents) {
        self.viewer_ctx
            .save_blueprint_archetype(entity_path, components);
    }

    #[inline]
    pub fn save_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_desc: &ComponentDescriptor,
        component_batch: &dyn ComponentBatch,
    ) {
        self.viewer_ctx
            .save_blueprint_component(entity_path, component_desc, component_batch);
    }

    #[inline]
    pub fn reset_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
    ) {
        self.viewer_ctx
            .reset_blueprint_component(entity_path, component_descr);
    }

    /// Clears a component in the blueprint store by logging an empty array if it exists.
    #[inline]
    pub fn clear_blueprint_component(
        &self,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
    ) {
        self.viewer_ctx
            .clear_blueprint_component(entity_path, component_descr);
    }

    #[inline]
    pub fn blueprint_timepoint_for_writes(&self) -> TimePoint {
        self.viewer_ctx
            .store_context
            .blueprint_timepoint_for_writes()
    }

    /// Iterates over all visualizers that are registered for this view.
    ///
    /// Note that these are newly instantiated visualizers, therefore their internal
    /// state is likely not of any use.
    pub fn new_visualizer_collection(&self) -> VisualizerCollection {
        self.viewer_ctx
            .view_class_registry()
            .new_visualizer_collection(self.view_class_identifier)
    }

    /// Returns the view class for the currently active view.
    pub fn view_class(&self) -> &dyn crate::ViewClass {
        self.viewer_ctx
            .view_class_registry()
            .get_class_or_log_error(self.view_class_identifier)
    }

    /// Returns the view class for the currently active view.
    pub fn view_class_entry(&self) -> &crate::view::view_class_registry::ViewClassRegistryEntry {
        self.viewer_ctx
            .view_class_registry()
            .get_class_entry_or_log_error(self.view_class_identifier)
    }
}
