use crate::{DataQueryResult, SpaceViewId};

/// The context associated with a view.
///
/// This combines our [`crate::ViewerContext`] with [`crate::SpaceViewState`]
/// and other view-specific information. This is used as the interface for
/// execution of view systems and selection panel UI elements that happen
/// within the context of a view to simplify plumbing of the necessary
/// information to resolve a query with possible overrides and fallback values.
pub struct ViewContext<'a> {
    pub viewer_ctx: &'a crate::ViewerContext<'a>,
    pub view_state: &'a dyn crate::SpaceViewState,
    pub visualizer_collection: &'a crate::VisualizerCollection,
}

impl<'a> ViewContext<'a> {
    /// The active recording.
    #[inline]
    pub fn recording(&self) -> &re_entity_db::EntityDb {
        self.viewer_ctx.recording()
    }

    /// The data store of the active recording.
    #[inline]
    pub fn recording_store(&self) -> &re_data_store::DataStore {
        self.viewer_ctx.recording_store()
    }

    /// The active blueprint.
    #[inline]
    pub fn blueprint_db(&self) -> &re_entity_db::EntityDb {
        self.viewer_ctx.blueprint_db()
    }

    /// The `StoreId` of the active recording.
    #[inline]
    pub fn recording_id(&self) -> &re_log_types::StoreId {
        self.viewer_ctx.recording_id()
    }

    /// Returns the current selection.
    pub fn selection(&self) -> &crate::ItemCollection {
        self.viewer_ctx.selection()
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &crate::ItemCollection {
        self.viewer_ctx.hovered()
    }

    pub fn selection_state(&self) -> &crate::ApplicationSelectionState {
        self.viewer_ctx.selection_state()
    }

    /// The current time query, based on the current time control.
    pub fn current_query(&self) -> re_data_store::LatestAtQuery {
        self.viewer_ctx.current_query()
    }

    /// Set hover/select/focus for a given selection based on an egui response.
    pub fn select_hovered_on_click(
        &self,
        response: &egui::Response,
        selection: impl Into<crate::ItemCollection>,
    ) {
        self.viewer_ctx.select_hovered_on_click(response, selection);
    }

    pub fn lookup_query_result(&self, id: SpaceViewId) -> &DataQueryResult {
        self.viewer_ctx.lookup_query_result(id)
    }
}
