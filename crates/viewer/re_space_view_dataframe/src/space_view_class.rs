use std::any::Any;
use std::collections::HashSet;

use re_chunk_store::{ColumnDescriptor, ColumnSelector};
use re_log_types::{EntityPath, EntityPathFilter, ResolvedTimeRange, TimelineName};
use re_types::blueprint::{archetypes, components};
use re_types_core::SpaceViewClassIdentifier;
use re_ui::UiExt as _;
use re_viewer_context::{
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewState, SpaceViewStateExt,
    SpaceViewSystemExecutionError, SystemExecutionOutput, ViewQuery, ViewerContext,
};
use re_viewport_blueprint::{SpaceViewContents, ViewProperty};

use crate::dataframe_ui::HideColumnAction;
use crate::{
    dataframe_ui::dataframe_ui, expanded_rows::ExpandedRowsCache, query_kind::QueryKind,
    view_query_v2, visualizer_system::EmptySystem,
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
                        let Some(schema) = &state.schema else {
                            // Shouldn't happen, except maybe on the first frame, which is too early
                            // for the user to click the menu anyway.
                            return Ok(());
                        };

                        let view_query =
                            super::view_query::Query::try_from_blueprint(ctx, space_view_id)?;
                        let query_timeline_name = view_query.timeline_name(ctx);

                        column_visibility_ui(ctx, ui, space_view_id, schema, &query_timeline_name)
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

        let (schema, hide_column_actions) = match query_mode {
            QueryKind::LatestAt { time } => {
                let query = re_chunk_store::LatestAtQueryExpression {
                    entity_path_filter,
                    timeline: *timeline,
                    at: time,
                };

                let schema = query_engine.schema_for_query(&query.clone().into());
                let selected_columns =
                    apply_column_visibility_to_schema(ctx, space_view_id, &timeline_name, &schema)?;

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
                let selected_columns =
                    apply_column_visibility_to_schema(ctx, space_view_id, &timeline_name, &schema)?;

                let hide_column_actions = dataframe_ui(
                    ctx,
                    ui,
                    query_engine.range(&query, selected_columns),
                    &mut state.expended_rows_cache,
                );

                (schema, hide_column_actions)
            }
        };

        handle_hide_column_actions(ctx, space_view_id, &schema, hide_column_actions)?;

        // make schema accessible to the column visibility UI
        state.schema = Some(schema);

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
) -> Result<Option<Vec<ColumnSelector>>, SpaceViewSystemExecutionError> {
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
                    || selected_time_columns
                        .contains(&components::TimelineName::from_timeline(&desc.timeline))
            }
            ColumnDescriptor::Component(desc) => {
                let blueprint_component_descriptor = components::ComponentColumnSelector::new(
                    &desc.entity_path,
                    desc.component_name,
                );

                selected_component_columns.contains(&blueprint_component_descriptor)
            }
        })
        .cloned()
        .map(ColumnSelector::from)
        .collect();

    Ok(Some(result))
}

/// Act upon any action triggered by the dataframe UI.
fn handle_hide_column_actions(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
    schema: &[ColumnDescriptor],
    actions: Vec<HideColumnAction>,
) -> Result<(), SpaceViewSystemExecutionError> {
    if actions.is_empty() {
        return Ok(());
    }

    let property = ViewProperty::from_archetype::<archetypes::DataframeVisibleColumns>(
        ctx.blueprint_db(),
        ctx.blueprint_query,
        space_view_id,
    );

    let mut column_selection_mode = property
        .component_or_empty::<components::ColumnSelectionMode>()?
        .unwrap_or_default();

    // We are hiding some columns, so we need to handle the switch from "All" to "Selected". When
    // that happens, we default to selecting all time columns and all component columns.
    let (mut selected_time_columns, mut selected_component_columns) =
        if column_selection_mode == components::ColumnSelectionMode::All {
            column_selection_mode = components::ColumnSelectionMode::Selected;
            property.save_blueprint_component(ctx, &column_selection_mode);

            let selected_time_columns = schema
                .iter()
                .filter_map(|column| match column {
                    ColumnDescriptor::Time(desc) => {
                        Some(components::TimelineName::from_timeline(&desc.timeline))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();

            let selected_component_columns = schema
                .iter()
                .filter_map(|column| match column {
                    ColumnDescriptor::Component(desc) => {
                        Some(components::ComponentColumnSelector::new(
                            &desc.entity_path,
                            desc.component_name,
                        ))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();

            (selected_time_columns, selected_component_columns)
        } else {
            (
                property.component_array_or_empty::<components::TimelineName>()?,
                property.component_array_or_empty::<components::ComponentColumnSelector>()?,
            )
        };

    for action in actions {
        match action {
            HideColumnAction::HideTimeColumn { timeline_name } => {
                selected_time_columns
                    .retain(|name| name != &components::TimelineName::from(timeline_name.as_str()));
            }

            HideColumnAction::HideComponentColumn {
                entity_path,
                component_name,
            } => {
                let blueprint_component_descriptor =
                    components::ComponentColumnSelector::new(&entity_path, component_name);
                selected_component_columns.retain(|desc| desc != &blueprint_component_descriptor);
            }
        }
    }

    property.save_blueprint_component(ctx, &selected_time_columns);
    property.save_blueprint_component(ctx, &selected_component_columns);

    Ok(())
}

fn column_visibility_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_id: SpaceViewId,
    schema: &[ColumnDescriptor],
    query_timeline_name: &TimelineName,
) -> Result<(), SpaceViewSystemExecutionError> {
    let property = ViewProperty::from_archetype::<archetypes::DataframeVisibleColumns>(
        ctx.blueprint_db(),
        ctx.blueprint_query,
        space_view_id,
    );

    let menu_ui = |ui: &mut egui::Ui| -> Result<(), SpaceViewSystemExecutionError> {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

        //
        // All or selected?
        //

        let mut column_selection_mode = property
            .component_or_empty::<components::ColumnSelectionMode>()?
            .unwrap_or_default();

        let changed = {
            ui.re_radio_value(
                &mut column_selection_mode,
                components::ColumnSelectionMode::All,
                "All",
            )
            .changed()
        } | {
            ui.re_radio_value(
                &mut column_selection_mode,
                components::ColumnSelectionMode::Selected,
                "Selected",
            )
            .changed()
        };

        if changed {
            property.save_blueprint_component(ctx, &column_selection_mode);
        }

        //
        // Selected time columns
        //

        let mut selected_time_columns = property
            .component_array_or_empty::<components::TimelineName>()?
            .into_iter()
            .collect::<HashSet<_>>();

        ui.label("Timelines");

        let mut changed = false;
        for column in schema {
            let ColumnDescriptor::Time(time_column_descriptor) = column else {
                continue;
            };

            let is_query_timeline = time_column_descriptor.timeline.name() == query_timeline_name;
            let is_enabled = !is_query_timeline
                && column_selection_mode == components::ColumnSelectionMode::Selected;
            let mut is_visible = is_query_timeline
                || selected_time_columns.contains(&components::TimelineName::from_timeline(
                    &time_column_descriptor.timeline,
                ));

            ui.add_enabled_ui(is_enabled, |ui| {
                if ui
                    .re_checkbox(&mut is_visible, column.short_name())
                    .on_disabled_hover_text("The query timeline must always be visible")
                    .changed()
                {
                    changed = true;

                    let timeline_name =
                        components::TimelineName::from_timeline(&time_column_descriptor.timeline);
                    if is_visible {
                        selected_time_columns.insert(timeline_name);
                    } else {
                        selected_time_columns.remove(&timeline_name);
                    }
                }
            });
        }

        if changed {
            let selected_time_columns = selected_time_columns.into_iter().collect::<Vec<_>>();
            property.save_blueprint_component(ctx, &selected_time_columns);
        }

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

            let blueprint_component_descriptor = components::ComponentColumnSelector::new(
                &component_column_descriptor.entity_path,
                component_column_descriptor.component_name,
            );

            let is_enabled = column_selection_mode == components::ColumnSelectionMode::Selected;
            let mut is_visible =
                selected_component_columns.contains(&blueprint_component_descriptor);

            ui.add_enabled_ui(is_enabled, |ui| {
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
            });
        }

        if changed {
            let selected_component_columns =
                selected_component_columns.into_iter().collect::<Vec<_>>();
            property.save_blueprint_component(ctx, &selected_component_columns);
        }

        Ok(())
    };

    egui::ScrollArea::vertical().show(ui, menu_ui).inner
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
