use std::any::Any;
use std::collections::HashSet;

use egui::NumExt as _;
use re_chunk_store::ColumnDescriptor;
use re_log_types::{EntityPath, EntityPathFilter, ResolvedTimeRange, TimelineName};
use re_types::blueprint::{archetypes, components, datatypes};
use re_types_core::SpaceViewClassIdentifier;
use re_ui::UiExt as _;
use re_viewer_context::{
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewState, SpaceViewStateExt,
    SpaceViewSystemExecutionError, SystemExecutionOutput, ViewQuery, ViewerContext,
};
use re_viewport_blueprint::{SpaceViewContents, ViewProperty};

use crate::{
    dataframe_ui::dataframe_ui, expanded_rows::ExpandedRowsCache, query_kind::QueryKind,
    visualizer_system::EmptySystem,
};

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
        crate::view_query::query_ui(ctx, ui, state, space_view_id)
    }

    fn extra_title_bar_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<DataframeSpaceViewState>()?;

        let result = ui
            .add_enabled_ui(state.schema.is_some(), |ui| {
                egui::menu::menu_custom_button(
                    ui,
                    ui.small_icon_button_widget(&re_ui::icons::COLUMN_VISIBILITY),
                    |ui| {
                        // This forces the menu to expand when the content grows, which happens when
                        // switching from "All" to "Selected".
                        ui.set_max_height(600.0.at_most(ui.ctx().screen_rect().height() * 0.9));
                        egui::ScrollArea::vertical()
                            .show(ui, |ui| {
                                // do nothing if we don't have a schema (might only happen during the first
                                // frame?)
                                if let Some(schema) = &state.schema {
                                    column_visibility_ui(ctx, ui, space_view_id, schema)?;
                                }

                                Ok(())
                            })
                            .inner
                    },
                )
                .inner
            })
            .inner;

        // Note: we get the `Result<(), SpaceViewSystemExecutionError>` from the inner closure only
        // if it was actually executed.
        result.unwrap_or(Ok(()))
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

        match query_mode {
            QueryKind::LatestAt { time } => {
                let query = re_chunk_store::LatestAtQueryExpression {
                    entity_path_filter,
                    timeline: *timeline,
                    at: time,
                };

                let schema = query_engine.schema_for_query(&query.clone().into());
                let selected_columns =
                    apply_column_visibility_to_schema(ctx, space_view_id, &timeline_name, &schema)?;
                state.schema = Some(schema);

                dataframe_ui(
                    ctx,
                    ui,
                    query_engine.latest_at(&query, selected_columns),
                    &mut state.expended_rows_cache,
                );
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
                        store_datatype: re_chunk_store::external::arrow2::datatypes::DataType::Null,
                        join_encoding: Default::default(),
                        is_static: false,
                    },
                };

                let schema = query_engine.schema_for_query(&query.clone().into());
                let selected_columns =
                    apply_column_visibility_to_schema(ctx, space_view_id, &timeline_name, &schema)?;
                state.schema = Some(schema);

                dataframe_ui(
                    ctx,
                    ui,
                    query_engine.range(&query, selected_columns),
                    &mut state.expended_rows_cache,
                );
            }
        };

        Ok(())
    }
}

/// Reads the blueprint configuration for column visibility, applies it to the schema, and returns
/// a [`re_dataframe::QueryEngine`]-compatible column selection.
fn apply_column_visibility_to_schema(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
    query_timeline_name: &TimelineName,
    schema: &[ColumnDescriptor],
) -> Result<Option<Vec<ColumnDescriptor>>, SpaceViewSystemExecutionError> {
    let property = ViewProperty::from_archetype::<archetypes::DataframeVisibleColumns>(
        ctx.blueprint_db(),
        ctx.blueprint_query,
        space_view_id,
    );

    let column_selection_mode = property
        .component_or_empty::<components::ColumnSelectionMode>()?
        .unwrap_or_default();

    if column_selection_mode == components::ColumnSelectionMode::All {
        return Ok(None);
    }

    let selected_time_columns = property
        .component_array_or_empty::<components::TimelineName>()?
        .into_iter()
        .collect::<HashSet<_>>();

    let selected_component_columns = property
        .component_array_or_empty::<components::ComponentColumnSelector>()?
        .into_iter()
        .collect::<HashSet<_>>();

    let result = schema
        .iter()
        .filter(|column| match column {
            ColumnDescriptor::Control(_) => true,
            ColumnDescriptor::Time(desc) => {
                // we always include the query timeline column because we need it for the dataframe ui
                desc.timeline.name() == query_timeline_name
                    || selected_time_columns.contains(&components::TimelineName::from(
                        desc.timeline.name().as_str(),
                    ))
            }
            ColumnDescriptor::Component(desc) => {
                let blueprint_component_descriptor: components::ComponentColumnSelector =
                    datatypes::ComponentColumnSelector {
                        entity_path: (&desc.entity_path).into(),
                        component_name: desc.component_name.as_str().into(),
                    }
                    .into();

                selected_component_columns.contains(&blueprint_component_descriptor)
            }
        })
        .cloned()
        .collect();

    Ok(Some(result))
}

fn column_visibility_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_id: SpaceViewId,
    schema: &[ColumnDescriptor],
) -> Result<(), SpaceViewSystemExecutionError> {
    //
    // All or selected?
    //

    let property = ViewProperty::from_archetype::<archetypes::DataframeVisibleColumns>(
        ctx.blueprint_db(),
        ctx.blueprint_query,
        space_view_id,
    );

    let column_selection_mode = property
        .component_or_empty::<components::ColumnSelectionMode>()?
        .unwrap_or_default();

    let mut new_column_visibility = column_selection_mode;

    let changed = {
        ui.re_radio_value(
            &mut new_column_visibility,
            components::ColumnSelectionMode::All,
            "All",
        )
        .changed()
    } || {
        ui.re_radio_value(
            &mut new_column_visibility,
            components::ColumnSelectionMode::Selected,
            "Selected",
        )
        .changed()
    };

    if changed {
        property.save_blueprint_component(ctx, &new_column_visibility);
    }

    if column_selection_mode == components::ColumnSelectionMode::All {
        return Ok(());
    }

    //
    // Selected time columns
    //

    // let mut selected_time_columns = property
    //     .component_array_or_empty::<components::TimelineName>()?
    //     .into_iter()
    //     .collect::<HashSet<_>>();

    //TODO: add UI for time columns (note: the query timeline should be forced ticked)

    //
    // Selected component columns
    //

    let mut selected_component_columns = property
        .component_array_or_empty::<components::ComponentColumnSelector>()?
        .into_iter()
        .collect::<HashSet<_>>();

    let mut current_entity = None;
    let mut changed = false;
    for column in schema {
        let ColumnDescriptor::Component(component_column_descriptor) = column else {
            continue;
        };

        if Some(&component_column_descriptor.entity_path) != current_entity.as_ref() {
            current_entity = Some(component_column_descriptor.entity_path.clone());
            ui.label(component_column_descriptor.entity_path.to_string());
        }

        let blueprint_component_descriptor: components::ComponentColumnSelector =
            datatypes::ComponentColumnSelector {
                entity_path: (&component_column_descriptor.entity_path).into(),
                component_name: component_column_descriptor.component_name.as_str().into(),
            }
            .into();

        let mut is_visible = selected_component_columns.contains(&blueprint_component_descriptor);

        if ui
            .re_checkbox(&mut is_visible, column.short_name())
            .changed()
        {
            changed = true;

            if is_visible {
                selected_component_columns.insert(blueprint_component_descriptor);
            } else {
                selected_component_columns.remove(&blueprint_component_descriptor);
            }
        }
    }

    if changed {
        let selected_component_columns = selected_component_columns.into_iter().collect::<Vec<_>>();
        property.save_blueprint_component(ctx, &selected_component_columns);
    }

    Ok(())
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
