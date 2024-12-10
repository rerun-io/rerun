use std::any::Any;

use crate::{
    dataframe_ui::dataframe_ui, expanded_rows::ExpandedRowsCache, view_query,
    visualizer_system::EmptySystem,
};
use re_chunk_store::{ColumnDescriptor, SparseFillStrategy};
use re_dataframe::QueryEngine;
use re_log_types::EntityPath;
use re_types_core::ViewClassIdentifier;
use re_viewer_context::{
    ViewClass, ViewClassRegistryError, ViewId, ViewState, ViewStateExt,
    ViewSystemExecutionError, SystemExecutionOutput, ViewQuery, ViewerContext,
};

#[derive(Default)]
struct DataframeViewState {
    /// Cache for the expanded rows.
    expended_rows_cache: ExpandedRowsCache,

    /// List of view columns for the current query, cached here for the column visibility UI.
    view_columns: Option<Vec<ColumnDescriptor>>,
}

impl ViewState for DataframeViewState {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Default)]
pub struct DataframeSpaceView;

impl ViewClass for DataframeSpaceView {
    fn identifier() -> ViewClassIdentifier {
        "Dataframe".into()
    }

    fn display_name(&self) -> &'static str {
        "Dataframe"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_DATAFRAME
    }

    fn help_markdown(&self, _egui_ctx: &egui::Context) -> String {
        "# Dataframe view

This view displays the content of the entities it contains in tabular form.

## View types

The Dataframe view operates in two modes: the _latest-at_ mode and the _time range_ mode. You can
select the mode in the selection panel.

In the _latest-at_ mode, the view displays the latest data for the timeline and time set in the time
panel. A row is shown for each entity instance.

In the _time range_ mode, the view displays all the data logged within the time range set for each
view entity. In this mode, each row corresponds to an entity and time pair. Rows are further split
if multiple `rr.log()` calls were made for the same entity/time. Static data is also displayed.

Note that the default visible time range depends on the selected mode. In particular, the time range
mode sets the default time range to _everything_. You can override this in the selection panel."
            .to_owned()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<EmptySystem>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<DataframeViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        _ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        // Doesn't spawn anything by default.
        Default::default()
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        space_view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<DataframeViewState>()?;
        let view_query = view_query::Query::from_blueprint(ctx, space_view_id);
        let Some(view_columns) = &state.view_columns else {
            // Shouldn't happen, except maybe on the first frame, which is too early
            // for the user to click the menu anyway.
            return Ok(());
        };
        view_query.selection_panel_ui(ctx, ui, space_view_id, view_columns)
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        _system_output: SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<DataframeViewState>()?;
        let view_query = view_query::Query::from_blueprint(ctx, query.space_view_id);

        let query_engine = QueryEngine {
            engine: ctx.recording().storage_engine_arc(),
        };

        let view_contents = query
            .iter_all_entities()
            .map(|entity| (entity.clone(), None))
            .collect();

        let sparse_fill_strategy = if view_query.latest_at_enabled()? {
            SparseFillStrategy::LatestAtGlobal
        } else {
            SparseFillStrategy::None
        };

        let mut dataframe_query = re_chunk_store::QueryExpression {
            view_contents: Some(view_contents),
            filtered_index: Some(view_query.timeline(ctx)?),
            filtered_index_range: Some(view_query.filter_by_range()?),
            filtered_is_not_null: view_query.filter_is_not_null()?,
            sparse_fill_strategy,
            selection: None,

            // not yet unsupported by the dataframe view
            filtered_index_values: None,
            using_index_values: None,
            include_semantically_empty_columns: false,
            include_indicator_columns: false,
            include_tombstone_columns: false,
        };

        let view_columns = query_engine.schema_for_query(&dataframe_query);
        dataframe_query.selection =
            view_query.apply_column_visibility_to_view_columns(ctx, &view_columns)?;

        let query_handle = query_engine.query(dataframe_query);

        let hide_column_actions = dataframe_ui(
            ctx,
            ui,
            &query_handle,
            &mut state.expended_rows_cache,
            &query.space_view_id,
        );

        view_query.handle_hide_column_actions(ctx, &view_columns, hide_column_actions)?;

        state.view_columns = Some(view_columns);
        Ok(())
    }
}

re_viewer_context::impl_component_fallback_provider!(DataframeSpaceView => []);
