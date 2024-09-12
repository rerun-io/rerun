use egui::Ui;

use crate::dataframe_ui::dataframe_ui;
use crate::{query_kind::QueryKind, view_query::Query, visualizer_system::EmptySystem};
use re_log_types::{EntityPath, EntityPathFilter, ResolvedTimeRange};
use re_space_view::view_property_ui;
use re_types::blueprint::archetypes;
use re_types_core::SpaceViewClassIdentifier;
use re_ui::list_item;
use re_viewer_context::{
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewState,
    SpaceViewSystemExecutionError, SystemExecutionOutput, ViewQuery, ViewerContext,
};
use re_viewport_blueprint::SpaceViewContents;

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
        Box::<()>::default()
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
        ui: &mut Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        crate::view_query::query_ui(ctx, ui, state, space_view_id)?;

        list_item::list_item_scope(ui, "dataframe_view_selection_ui", |ui| {
            let view_query = Query::try_from_blueprint(ctx, space_view_id)?;
            //TODO(#7070): column order and sorting needs much love
            ui.add_enabled_ui(
                matches!(view_query.kind(ctx), QueryKind::Range { .. }),
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
        _state: &mut dyn SpaceViewState,
        query: &ViewQuery<'_>,
        _system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let view_query = super::view_query::Query::try_from_blueprint(ctx, query.space_view_id)?;
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

        match query_mode {
            QueryKind::LatestAt { time } => {
                let query = re_chunk_store::LatestAtQueryExpression {
                    entity_path_filter,
                    timeline: *timeline,
                    at: time,
                };

                //TODO(ab): specify which columns
                let query_handle = query_engine.latest_at(&query, None);

                dataframe_ui(ctx, ui, query_handle);
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
                    pov: re_chunk_store::ComponentColumnDescriptor {
                        entity_path: pov_entity.clone(),
                        archetype_name: None,
                        archetype_field_name: None,
                        component_name: pov_component,
                        // this is actually ignored:
                        datatype: re_chunk_store::external::arrow2::datatypes::DataType::Null,
                        is_static: false,
                    },
                };

                //TODO(ab): specify which columns should be displayed or not
                dataframe_ui(ctx, ui, query_engine.range(&query, None));
            }
        };

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
