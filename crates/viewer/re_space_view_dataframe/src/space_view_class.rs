use egui::Ui;

use re_log_types::EntityPath;
use re_space_view::view_property_ui;
use re_types::blueprint::{
    archetypes::TableRowOrder,
    components::{SortKey, SortOrder},
};
use re_types_core::SpaceViewClassIdentifier;
use re_ui::list_item;
use re_viewer_context::{
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewState,
    SpaceViewSystemExecutionError, SystemExecutionOutput, ViewQuery, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use crate::{
    latest_at_table::latest_at_table_ui, time_range_table::time_range_table_ui,
    visualizer_system::EmptySystem,
};

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

The _time range_ mode, the view displays all the data logged within the time range set for each
view entity. In this mode, each row corresponds to an entity and time pair. Rows are further split
if multiple `rr.log()` calls were made for the same entity/time. Static data is also displayed.

The view switches to _time range_ mode as soon as a single one of its entities has its visible time
range set to _Override_. Each entity may have its own time range setting. (To set the same time range
for all entities, it is preferable to override the view-level visible time range at the view.)"
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
        list_item::list_item_scope(ui, "dataframe_view_selection_ui", |ui| {
            view_property_ui::<TableRowOrder>(ctx, ui, space_view_id, self, state);
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

        let row_order = ViewProperty::from_archetype::<TableRowOrder>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );
        let sort_key = row_order.component_or_fallback::<SortKey>(ctx, self, state)?;
        let sort_order = row_order.component_or_fallback::<SortOrder>(ctx, self, state)?;

        let mode = self.table_mode(query);

        match mode {
            TableMode::LatestAtTable => latest_at_table_ui(ctx, ui, query),
            TableMode::TimeRangeTable => time_range_table_ui(ctx, ui, query, sort_key, sort_order),
        };

        Ok(())
    }
}

re_viewer_context::impl_component_fallback_provider!(DataframeSpaceView => []);

/// The two modes of the dataframe view.
enum TableMode {
    LatestAtTable,
    TimeRangeTable,
}

impl DataframeSpaceView {
    /// Determine which [`TableMode`] is currently active.
    // TODO(ab): we probably want a less "implicit" way to switch from temporal vs. latest at tables.
    #[allow(clippy::unused_self)]
    fn table_mode(&self, query: &ViewQuery<'_>) -> TableMode {
        let is_range_query = query
            .iter_all_data_results()
            .any(|data_result| data_result.property_overrides.query_range.is_time_range());

        if is_range_query {
            TableMode::TimeRangeTable
        } else {
            TableMode::LatestAtTable
        }
    }
}
