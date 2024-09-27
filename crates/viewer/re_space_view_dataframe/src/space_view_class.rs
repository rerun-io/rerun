use std::any::Any;

use crate::{
    dataframe_ui::dataframe_ui, expanded_rows::ExpandedRowsCache, query_kind::QueryKind,
    view_query_v2, visualizer_system::EmptySystem,
};
use re_chunk_store::ColumnDescriptor;
use re_log_types::{EntityPath, EntityPathFilter, ResolvedTimeRange};
use re_types_core::SpaceViewClassIdentifier;
use re_ui::UiExt as _;
use re_viewer_context::{
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewState, SpaceViewStateExt,
    SpaceViewSystemExecutionError, SystemExecutionOutput, ViewQuery, ViewerContext,
};
use re_viewport_blueprint::SpaceViewContents;

#[derive(Default)]
struct DataframeSpaceViewState {
    /// Cache for the expanded rows.
    expended_rows_cache: ExpandedRowsCache,

    /// Schema for the current query, cached here for the column visibility UI.
    schema: Option<Vec<ColumnDescriptor>>,
}

impl SpaceViewState for DataframeSpaceViewState {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Default)]
pub struct DataframeSpaceView;

impl SpaceViewClass for DataframeSpaceView {
    fn identifier() -> SpaceViewClassIdentifier {
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
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<EmptySystem>()
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<DataframeSpaceViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        _ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::SpaceViewSpawnHeuristics {
        // Doesn't spawn anything by default.
        Default::default()
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        crate::view_query::query_ui(ctx, ui, state, space_view_id)?;

        //TODO(ab): just display the UI for now, this has no effect on the view itself yet.
        ui.separator();
        let state = state.downcast_mut::<DataframeSpaceViewState>()?;
        let view_query = view_query_v2::QueryV2::from_blueprint(ctx, space_view_id);
        let Some(schema) = &state.schema else {
            // Shouldn't happen, except maybe on the first frame, which is too early
            // for the user to click the menu anyway.
            return Ok(());
        };
        view_query.selection_panel_ui(ctx, ui, space_view_id, schema)
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        query: &ViewQuery<'_>,
        _system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();
        let state = state.downcast_mut::<DataframeSpaceViewState>()?;
        let space_view_id = query.space_view_id;

        let view_query = super::view_query::Query::try_from_blueprint(ctx, space_view_id)?;
        let timeline_name = view_query.timeline_name(ctx);
        let query_mode = view_query.kind(ctx);

        let Some(timeline) = ctx
            .recording()
            .timelines()
            .find(|t| t.name() == &timeline_name)
        else {
            re_log::warn_once!("Could not find timeline {:?}.", timeline_name.as_str());
            //TODO(ab): we should have an error for that
            return Ok(());
        };

        let query_engine = ctx.recording().query_engine();

        let entity_path_filter =
            Self::entity_path_filter(ctx, query.space_view_id, query.space_origin);

        // use the new query for column visibility
        let query_v2 = view_query_v2::QueryV2::from_blueprint(ctx, query.space_view_id);

        let (schema, hide_column_actions) = match query_mode {
            QueryKind::LatestAt { time } => {
                let query = re_chunk_store::LatestAtQueryExpression {
                    entity_path_filter,
                    timeline: *timeline,
                    at: time,
                };

                let schema = query_engine.schema_for_query(&query.clone().into());
                let selected_columns = query_v2.apply_column_visibility_to_schema(ctx, &schema)?;

                let hide_column_actions = dataframe_ui(
                    ctx,
                    ui,
                    query_engine.latest_at(&query, selected_columns),
                    &mut state.expended_rows_cache,
                );

                (schema, hide_column_actions)
            }
            QueryKind::Range {
                pov_entity,
                pov_component,
                from,
                to,
            } => {
                let query = re_chunk_store::RangeQueryExpression {
                    entity_path_filter,
                    timeline: *timeline,
                    time_range: ResolvedTimeRange::new(from, to),
                    //TODO(#7365): using ComponentColumnDescriptor to specify PoV needs to go
                    pov: re_chunk_store::ComponentColumnSelector {
                        entity_path: pov_entity.clone(),
                        component: pov_component,
                        join_encoding: Default::default(),
                    },
                };

                let schema = query_engine.schema_for_query(&query.clone().into());
                let selected_columns = query_v2.apply_column_visibility_to_schema(ctx, &schema)?;

                let hide_column_actions = dataframe_ui(
                    ctx,
                    ui,
                    query_engine.range(&query, selected_columns),
                    &mut state.expended_rows_cache,
                );

                (schema, hide_column_actions)
            }
        };

        query_v2.handle_hide_column_actions(ctx, &schema, hide_column_actions)?;

        // make schema accessible to the column visibility UI
        state.schema = Some(schema);

        Ok(())
    }
}

impl DataframeSpaceView {
    fn entity_path_filter(
        ctx: &ViewerContext<'_>,
        space_view_id: SpaceViewId,
        space_origin: &EntityPath,
    ) -> EntityPathFilter {
        //TODO(ab): this feels a little bit hacky but there isn't currently another way to get to
        //the original entity path filter.
        SpaceViewContents::from_db_or_default(
            space_view_id,
            ctx.blueprint_db(),
            ctx.blueprint_query,
            Self::identifier(),
            &re_log_types::EntityPathSubs::new_with_origin(space_origin),
        )
        .entity_path_filter
    }
}

re_viewer_context::impl_component_fallback_provider!(DataframeSpaceView => []);
