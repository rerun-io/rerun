mod blueprint;
mod ui;

use re_chunk_store::ColumnDescriptor;
use re_sdk_types::blueprint::archetypes;
use re_viewer_context::{ViewId, ViewSystemExecutionError, ViewerContext};
use re_viewport_blueprint::ViewProperty;

/// Wrapper over the `DataframeQuery` blueprint archetype that can also display some UI.
pub struct Query {
    query_property: ViewProperty,
}

impl Query {
    /// Create a query object from the blueprint store.
    ///
    /// See the `blueprint_io` module for more related accessors.
    pub fn from_blueprint(ctx: &ViewerContext<'_>, view_id: ViewId) -> Self {
        Self {
            query_property: ViewProperty::from_archetype::<archetypes::DataframeQuery>(
                ctx.blueprint_db(),
                ctx.blueprint_query,
                view_id,
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
        view_id: ViewId,
        view_columns: Option<&[ColumnDescriptor]>,
    ) -> Result<(), ViewSystemExecutionError> {
        ui.add_space(4.0);

        let timeline_name = self.timeline_name(ctx)?;
        self.timeline_ui(ctx, ui, timeline_name)?;

        let timeline = self.timeline(ctx)?;

        ui.separator();
        self.filter_range_ui(ctx, ui, timeline.as_ref())?;
        ui.separator();
        self.filter_is_not_null_ui(ctx, ui, timeline.map(|tl| *tl.name()).as_ref(), view_id)?;
        ui.separator();
        self.column_visibility_ui(ctx, ui, timeline.as_ref(), view_columns)?;
        ui.separator();
        self.latest_at_ui(ctx, ui)?;

        Ok(())
    }
}
