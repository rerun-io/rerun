use egui::Ui;
use re_chunk_store::LatestAtQuery;
use re_log_types::{EntityPath, ResolvedTimeRange};
use re_space_view::view_property_ui;
use re_types::blueprint::components::{LatestAtQueries, Timeline};
use re_types::blueprint::{archetypes, components, datatypes};
use re_types_core::datatypes::{TimeRange, Utf8};
use re_types_core::SpaceViewClassIdentifier;
use re_ui::list_item;
use re_viewer_context::{
    QueryContext, QueryRange, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewState, SpaceViewStateExt, SpaceViewSystemExecutionError, SystemExecutionOutput,
    TypedComponentFallbackProvider, ViewQuery, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;
use std::any::Any;

use crate::view_query::{Query, QueryMode};
use crate::{
    latest_at_table::latest_at_table_ui, time_range_table::time_range_table_ui,
    visualizer_system::EmptySystem,
};

/// State for the Dataframe view.
///
/// We use this to carry information from `ui()` to `default_query_range()` as a workaround for
/// `https://github.com/rerun-io/rerun/issues/6918`.
#[derive(Debug, Default)]
struct DataframeViewState {
    mode: components::QueryKind,
}

impl SpaceViewState for DataframeViewState {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
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

The Dataframe view operates in two modes: the _latest at_ mode and the _time range_ mode. You can
select the mode in the selection panel.

In the _latest at_ mode, the view displays the latest data for the timeline and time set in the time
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
        Box::<DataframeViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn default_query_range(&self, state: &dyn SpaceViewState) -> QueryRange {
        // TODO(#6918): passing the mode via view state is a hacky work-around, until we're able to
        // pass more context to this function.
        let mode = state
            .downcast_ref::<DataframeViewState>()
            .map(|state| state.mode)
            .inspect_err(|err| re_log::warn_once!("Unexpected view type: {err}"))
            .unwrap_or_default();

        match mode {
            components::QueryKind::LatestAt => QueryRange::LatestAt,
            components::QueryKind::TimeRange => QueryRange::TimeRange(TimeRange::EVERYTHING),
        }
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
        ui: &mut Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        crate::view_query::query_ui(ctx, ui, state, space_view_id)?;

        list_item::list_item_scope(ui, "dataframe_view_selection_ui", |ui| {
            //TODO: fix this :scream:
            let view_query = Query::try_from_blueprint(ctx, space_view_id)?;

            ui.add_enabled_ui(
                matches!(view_query.mode(ctx), QueryMode::Range { .. }),
                |ui| {
                    view_property_ui::<archetypes::TimeRangeTableOrder>(
                        ctx,
                        ui,
                        space_view_id,
                        self,
                        state,
                    );
                },
            );

            Ok(())
        })
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

        let view_query = super::view_query::Query::try_from_blueprint(ctx, query.space_view_id)?;
        let timeline_name = view_query.timeline_name(ctx);
        let query_mode = view_query.mode(ctx);

        let Some(timeline) = ctx
            .recording()
            .timelines()
            .find(|t| t.name() == &timeline_name)
        else {
            re_log::warn_once!("Could not find timeline {:?}.", timeline_name.as_str());
            //TODO(ab): we should have an error for that
            return Ok(());
        };

        match query_mode {
            QueryMode::LatestAt { time } => {
                latest_at_table_ui(ctx, ui, query, LatestAtQuery::new(*timeline, time))
            }
            QueryMode::Range { from, to } => {
                let time_range_table_order =
                    ViewProperty::from_archetype::<archetypes::TimeRangeTableOrder>(
                        ctx.blueprint_db(),
                        ctx.blueprint_query,
                        query.space_view_id,
                    );
                let sort_key = time_range_table_order
                    .component_or_fallback::<components::SortKey>(ctx, self, state)?;
                let sort_order = time_range_table_order
                    .component_or_fallback::<components::SortOrder>(ctx, self, state)?;

                time_range_table_ui(
                    ctx,
                    ui,
                    query,
                    sort_key,
                    sort_order,
                    timeline,
                    ResolvedTimeRange::new(from, to),
                );
            }
        }

        Ok(())
    }
}

// //TODO: probably no longer needed
// impl TypedComponentFallbackProvider<components::Timeline> for DataframeSpaceView {
//     fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Timeline {
//         //TODO: add helper to Timeline component
//         Timeline(Utf8::from(
//             ctx.viewer_ctx
//                 .rec_cfg
//                 .time_ctrl
//                 .read()
//                 .timeline()
//                 .name()
//                 .as_str(),
//         ))
//     }
// }
//
// //TODO: probably no longer needed
// impl TypedComponentFallbackProvider<components::LatestAtQueries> for DataframeSpaceView {
//     fn fallback_for(&self, ctx: &QueryContext<'_>) -> LatestAtQueries {
//         let current_time = ctx.viewer_ctx.rec_cfg.time_ctrl.read();
//
//         let latest_at_query = datatypes::LatestAtQuery {
//             timeline: Utf8::from(current_time.timeline().name().as_str()),
//             time: re_types_core::datatypes::TimeInt::from(
//                 current_time
//                     .time_int()
//                     .unwrap_or(re_log_types::TimeInt::MAX),
//             ),
//         };
//         LatestAtQueries::from(vec![latest_at_query])
//     }
// }
//
// re_viewer_context::impl_component_fallback_provider!(DataframeSpaceView => [Timeline, LatestAtQueries]);
