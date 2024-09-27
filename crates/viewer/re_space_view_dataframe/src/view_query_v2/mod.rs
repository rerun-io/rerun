mod blueprint_io;
mod ui;

use re_chunk_store::ColumnDescriptor;
use re_log_types::EntityPath;
use re_types::blueprint::archetypes;
use re_types_core::ComponentName;
use re_viewer_context::{SpaceViewId, SpaceViewSystemExecutionError, ViewerContext};
use re_viewport_blueprint::ViewProperty;

/// Struct to hold the point-of-view column used for the filter by event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct EventColumn {
    pub(crate) entity_path: EntityPath,
    pub(crate) component_name: ComponentName,
}

/// Wrapper over the `DataframeQueryV2` blueprint archetype that can also display some UI.
pub(crate) struct QueryV2 {
    query_property: ViewProperty,
}

impl QueryV2 {
    /// Create a query object from the blueprint store.
    ///
    /// See the `blueprint_io` module for more related accessors.
    pub(crate) fn from_blueprint(ctx: &ViewerContext<'_>, space_view_id: SpaceViewId) -> Self {
        Self {
            query_property: ViewProperty::from_archetype::<archetypes::DataframeQueryV2>(
                ctx.blueprint_db(),
                ctx.blueprint_query,
                space_view_id,
            ),
        }
    }

    /// Display the selection panel ui for this query.
    ///
    /// Implementation is in the `ui` module.
    pub(crate) fn selection_panel_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_view_id: SpaceViewId,
        schema: &[ColumnDescriptor],
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let timeline = self.timeline(ctx)?;

        self.timeline_ui(ctx, ui, &timeline)?;
        ui.separator();
        self.filter_range_ui(ctx, ui, &timeline)?;
        ui.separator();
        self.filter_event_ui(ctx, ui, &timeline, space_view_id)?;
        ui.separator();
        self.column_visibility_ui(ctx, ui, &timeline, schema)?;
        ui.separator();
        self.latest_at_ui(ctx, ui)?;

        Ok(())
    }
}
