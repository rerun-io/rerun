use std::any::Any;

use re_chunk_store::{ColumnDescriptor, SparseFillStrategy};
use re_dataframe::QueryEngine;
use re_log_types::EntityPath;
use re_types_core::ViewClassIdentifier;
use re_ui::{Help, UiExt as _};
use re_viewer_context::{
    Item, SystemCommand, SystemCommandSender as _, SystemExecutionOutput, ViewClass,
    ViewClassRegistryError, ViewId, ViewQuery, ViewSpawnHeuristics, ViewState, ViewStateExt as _,
    ViewSystemExecutionError, ViewerContext,
};

use crate::dataframe_ui::dataframe_ui;
use crate::expanded_rows::ExpandedRowsCache;
use crate::view_query;
use crate::visualizer_system::EmptySystem;

#[derive(Default)]
struct DataframeViewState {
    /// Cache for the expanded rows.
    expanded_rows_cache: ExpandedRowsCache,

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

    fn recommendation_order(&self) -> i32 {
        // Put the dataframe view last in recommendations since it is a bit of a catch-all view!
        i32::MAX
    }

    fn display_name(&self) -> &'static str {
        "Dataframe"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_DATAFRAME
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
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
        _include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        // Doesn't spawn anything by default.
        ViewSpawnHeuristics::empty()
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
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
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

        // TODO(andreas): why are we dealing with a ViewerContext and not a ViewContext here? The later would have the query results readily available.
        let query_results = ctx.lookup_query_result(query.view_id);
        let view_contents = query_results
            .tree
            .iter_data_results()
            .map(|data_result| (data_result.entity_path.clone(), None))
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
            include_tombstone_columns: false,
            include_static_columns: re_chunk_store::StaticColumnSelection::Both,
        };

        let view_columns = query_engine
            .schema_for_query(&dataframe_query)
            .indices_and_components();
        let (view_columns, selection) = view_query.apply_column_selection(ctx, &view_columns)?;
        dataframe_query.selection = Some(selection);

        let query_handle = query_engine.query(dataframe_query);

        let hide_column_actions = dataframe_ui(
            ctx,
            ui,
            &query_handle,
            &mut state.expanded_rows_cache,
            &query.view_id,
        );

        view_query.handle_hide_column_actions(ctx, &view_columns, hide_column_actions)?;

        state.view_columns = Some(view_columns);
        Ok(())
    }
}

fn timeline_not_found_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, view_id: ViewId) {
    let full_view_rect = ui.available_rect_before_wrap();
    let tokens = ui.tokens();

    egui::Frame::new()
        .inner_margin(tokens.view_padding())
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
        ctx.command_sender()
            .send_system(SystemCommand::set_selection(Item::View(view_id)));
    }
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| DataframeView.help(ctx));
}
