use egui::Ui;
use std::any::Any;

use re_log_types::EntityPath;
use re_space_view::view_property_ui;
use re_types::blueprint::{archetypes, components};
use re_types_core::datatypes::TimeRange;
use re_types_core::SpaceViewClassIdentifier;
use re_ui::list_item;
use re_viewer_context::{
    QueryRange, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewState,
    SpaceViewStateExt, SpaceViewSystemExecutionError, SystemExecutionOutput, ViewQuery,
    ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

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
    mode: components::DataframeViewMode,
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
            components::DataframeViewMode::LatestAt => QueryRange::LatestAt,
            components::DataframeViewMode::TimeRange => {
                QueryRange::TimeRange(TimeRange::EVERYTHING)
            }
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
        let settings = ViewProperty::from_archetype::<archetypes::DataframeViewMode>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            space_view_id,
        );

        let mode =
            settings.component_or_fallback::<components::DataframeViewMode>(ctx, self, state)?;

        list_item::list_item_scope(ui, "dataframe_view_selection_ui", |ui| {
            //TODO(ab): ideally we'd drop the "Dataframe" part in the UI label
            view_property_ui::<archetypes::DataframeViewMode>(ctx, ui, space_view_id, self, state);

            ui.add_enabled_ui(mode == components::DataframeViewMode::TimeRange, |ui| {
                view_property_ui::<archetypes::TimeRangeTableOrder>(
                    ctx,
                    ui,
                    space_view_id,
                    self,
                    state,
                );
            });
        });

        Ok(())
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

        let settings = ViewProperty::from_archetype::<archetypes::DataframeViewMode>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );

        let mode =
            settings.component_or_fallback::<components::DataframeViewMode>(ctx, self, state)?;

        // update state
        let state = state.downcast_mut::<DataframeViewState>()?;
        state.mode = mode;

        match mode {
            components::DataframeViewMode::LatestAt => latest_at_table_ui(ctx, ui, query),

            components::DataframeViewMode::TimeRange => {
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

                time_range_table_ui(ctx, ui, query, sort_key, sort_order);
            }
        };

        Ok(())
    }
}

re_viewer_context::impl_component_fallback_provider!(DataframeSpaceView => []);
