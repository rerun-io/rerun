use egui::Ui;
use std::any::Any;

use re_log_types::EntityPath;
use re_space_view::view_property_ui;
use re_types::blueprint::{
    archetypes::DataframeSettings,
    components::{DataframeMode, SortKey, SortOrder},
};
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
    mode: DataframeMode,
}

impl SpaceViewState for DataframeViewState {
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

This view displays the content of the entities it contains in tabular form. Click on the view and
use the _Entity path filter_ to control which entities are displayed.

## View types

The Dataframe view operates in two modes: the _latest at_ mode and the _time range_ mode.

In the _latest at_ mode, the view displays the latest data for the timeline and time set in the time
panel. A row is shown for each entity instance.

In the _time range_ mode, the view displays all the data logged within the time range set for each
view entity. In this mode, each row corresponds to an entity and time pair. Rows are further split
if multiple `rr.log()` calls were made for the same entity/time. Static data is also displayed.

The view switches to _time range_ mode as soon as a single one of its entities has its visible time
range set to _Override_. Each entity may have its own time range setting. (To set the same time range
for all entities, it is preferable to override the view-level visible time range.)"
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
            .unwrap_or_default();

        match mode {
            DataframeMode::LatestAt => QueryRange::LatestAt,
            DataframeMode::TimeRange => QueryRange::TimeRange(TimeRange::EVERYTHING),
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
        list_item::list_item_scope(ui, "dataframe_view_selection_ui", |ui| {
            //TODO(#6919): this bit of UI needs some love
            view_property_ui::<DataframeSettings>(ctx, ui, space_view_id, self, state);
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

        let settings = ViewProperty::from_archetype::<DataframeSettings>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );

        let mode = settings.component_or_fallback::<DataframeMode>(ctx, self, state)?;
        let sort_key = settings.component_or_fallback::<SortKey>(ctx, self, state)?;
        let sort_order = settings.component_or_fallback::<SortOrder>(ctx, self, state)?;

        // update state
        let state = state.downcast_mut::<DataframeViewState>()?;
        state.mode = mode;

        match mode {
            DataframeMode::LatestAt => latest_at_table_ui(ctx, ui, query),
            DataframeMode::TimeRange => time_range_table_ui(ctx, ui, query, sort_key, sort_order),
        };

        Ok(())
    }
}

re_viewer_context::impl_component_fallback_provider!(DataframeSpaceView => []);
