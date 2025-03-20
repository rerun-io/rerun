use std::any::Any;

use crate::{
    dataframe_ui::dataframe_ui, expanded_rows::ExpandedRowsCache, view_query,
    visualizer_system::EmptySystem,
};
use re_chunk_store::{ColumnDescriptor, SparseFillStrategy};
use re_dataframe::QueryEngine;
use re_log_types::{EntityPath, ResolvedEntityPathFilter};
use re_types_core::ViewClassIdentifier;
use re_ui::{Help, UiExt as _};
use re_viewer_context::{
    Item, SystemExecutionOutput, ViewClass, ViewClassRegistryError, ViewId, ViewQuery, ViewState,
    ViewStateExt as _, ViewSystemExecutionError, ViewerContext,
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
pub struct DataframeView;

impl ViewClass for DataframeView {
    fn identifier() -> ViewClassIdentifier {
        "Dataframe".into()
    }

    fn display_name(&self) -> &'static str {
        "Dataframe"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_DATAFRAME
    }

    fn help(&self, _egui_ctx: &egui::Context) -> Help {
        Help::new("Dataframe view")
            .docs_link("https://rerun.io/docs/reference/types/views/dataframe_view")
            .markdown(
                "This view displays entity content in a tabular form.

Configure in the selection panel:
 - Handling of empty cells
 - Column visibility
 - Row filtering by time range",
            )
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
        _suggested_filter: &ResolvedEntityPathFilter,
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
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<DataframeViewState>()?;
        let view_query = view_query::Query::from_blueprint(ctx, view_id);
        view_query.selection_panel_ui(ctx, ui, view_id, state.view_columns.as_deref())
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
        let view_query = view_query::Query::from_blueprint(ctx, query.view_id);

        // Make sure we know which timeline to query or display an error message.
        let timeline = view_query.timeline(ctx)?;

        let Some(timeline) = timeline else {
            timeline_not_found_ui(ctx, ui, query.view_id);
            return Ok(());
        };

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
            filtered_index: Some(*timeline.name()),
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

        let view_columns = query_engine
            .schema_for_query(&dataframe_query)
            .indices_and_components();
        dataframe_query.selection =
            view_query.apply_column_visibility_to_view_columns(ctx, &view_columns)?;

        let query_handle = query_engine.query(dataframe_query);

        let hide_column_actions = dataframe_ui(
            ctx,
            ui,
            &query_handle,
            &mut state.expended_rows_cache,
            &query.view_id,
        );

        view_query.handle_hide_column_actions(ctx, &view_columns, hide_column_actions)?;

        state.view_columns = Some(view_columns);
        Ok(())
    }
}

fn timeline_not_found_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, view_id: ViewId) {
    let full_view_rect = ui.available_rect_before_wrap();

    egui::Frame::new()
        .inner_margin(re_ui::DesignTokens::view_padding())
        .show(ui, |ui| {
            ui.warning_label("Unknown timeline");

            ui.label(
                "The timeline currently configured for this view does not exist in the current \
                recording. Select another timeline in the view properties found in the selection \
                panel.",
            )
        });

    // select the view when clicked
    if ui
        .interact(
            full_view_rect,
            egui::Id::from("dataframe_view_empty").with(view_id),
            egui::Sense::click(),
        )
        .clicked()
    {
        ctx.selection_state.set_selection(Item::View(view_id));
    }
}

re_viewer_context::impl_component_fallback_provider!(DataframeView => []);

#[test]
fn test_help_view() {
    re_viewer_context::test_context::TestContext::test_help_view(|ctx| DataframeView.help(ctx));
}
