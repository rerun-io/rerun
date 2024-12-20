use std::sync::Arc;

use re_chunk_store::LatestAtQuery;
use re_log_types::{EntityPath, TimePoint};
use re_query::StorageEngineReadGuard;
use re_types::{AsComponents, ComponentBatch, ComponentName};

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
    pub view_state: &'a dyn crate::ViewState,
    pub visualizer_collection: Arc<crate::VisualizerCollection>,
    pub query_result: &'a DataQueryResult,
}

impl<'a> ViewContext<'a> {
    #[inline]
    pub fn query_context(
        &'a self,
        data_result: &'a DataResult,
        query: &'a LatestAtQuery,
    ) -> QueryContext<'a> {
        QueryContext {
            viewer_ctx: self.viewer_ctx,
            target_entity_path: &data_result.entity_path,
            archetype_name: None,
            query,
            view_state: self.view_state,
            view_ctx: Some(self),
        }
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

    /// The `StoreId` of the active recording.
    #[inline]
    pub fn recording_id(&self) -> re_log_types::StoreId {
        self.viewer_ctx.recording_id()
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
        entity_path: &EntityPath,
        component_name: ComponentName,
        array: arrow::array::ArrayRef,
    ) {
        self.viewer_ctx
            .save_blueprint_array(entity_path, component_name, array);
    }

    #[inline]
    pub fn save_blueprint_archetype(
        &self,
        entity_path: &EntityPath,
        components: &dyn AsComponents,
    ) {
        self.viewer_ctx
            .save_blueprint_archetype(entity_path, components);
    }

    #[inline]
    pub fn save_blueprint_component(
        &self,
        entity_path: &EntityPath,
        components: &dyn ComponentBatch,
    ) {
        self.viewer_ctx
            .save_blueprint_component(entity_path, components);
    }

    #[inline]
    pub fn save_empty_blueprint_component<C>(&self, entity_path: &EntityPath)
    where
        C: re_types::Component + 'a,
    {
        self.viewer_ctx
            .save_empty_blueprint_component::<C>(entity_path);
    }

    #[inline]
    pub fn reset_blueprint_component_by_name(
        &self,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) {
        self.viewer_ctx
            .reset_blueprint_component_by_name(entity_path, component_name);
    }

    /// Clears a component in the blueprint store by logging an empty array if it exists.
    #[inline]
    pub fn clear_blueprint_component_by_name(
        &self,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) {
        self.viewer_ctx
            .clear_blueprint_component_by_name(entity_path, component_name);
    }

    #[inline]
    pub fn blueprint_timepoint_for_writes(&self) -> TimePoint {
        self.viewer_ctx
            .store_context
            .blueprint_timepoint_for_writes()
    }
}
