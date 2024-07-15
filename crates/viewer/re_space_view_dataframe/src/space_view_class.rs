use crate::visualizer_system::EmptySystem;
use egui::Ui;
use egui_extras::{Column, TableRow};
use re_chunk_store::{Chunk, ChunkStore, LatestAtQuery, RangeQuery, RowId};
use re_data_ui::item_ui::{entity_path_button, instance_path_button};
use re_entity_db::InstancePath;
use re_log_types::{EntityPath, Instance, ResolvedTimeRange, TimeInt, Timeline};
use re_space_view::view_property_ui;
use re_types::blueprint::archetypes::TableRowOrder;
use re_types::blueprint::components::{SortOrder, TableGroupBy};
use re_types_core::datatypes::TimeRange;
use re_types_core::{ComponentName, SpaceViewClassIdentifier};
use re_ui::list_item;
use re_viewer_context::{
    QueryRange, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewState,
    SpaceViewSystemExecutionError, SystemExecutionOutput, UiLayout, ViewQuery, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

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
if multiple `rr.log()` calls were made  for the same entity/time. Static data is also displayed.

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
        let group_by = row_order.component_or_fallback::<TableGroupBy>(ctx, self, state)?;
        let sort_order = row_order.component_or_fallback::<SortOrder>(ctx, self, state)?;

        // TODO(ab): we probably want a less "implicit" way to switch from temporal vs. latest at tables.
        let is_range_query = query
            .iter_all_data_results()
            .any(|data_result| data_result.property_overrides.query_range.is_time_range());

        if is_range_query {
            entity_and_time_vs_component_ui(ctx, ui, query, group_by, sort_order);
        } else {
            entity_and_instance_vs_component_ui(ctx, ui, query);
        }

        Ok(())
    }
}

re_viewer_context::impl_component_fallback_provider!(DataframeSpaceView => []);

/// Show a table with entities and time as rows, and components as columns.
///
/// Here, a "row" is a tuple of `(entity_path, time, row_id)`. This means that both "over logging"
/// (i.e. logging multiple times the same entity/component at the same timestamp) and "split
/// logging" (i.e. multiple log calls on the same [entity, time] but with different components)
/// lead to multiple rows. In other words, no joining is done here.
///
/// Also:
/// - View entities have their query range "forced" to a range query. If set to "latest at", they
///   are converted to Rel(0)-Rel(0).
/// - Only the data logged in the query range is displayed. There is no implicit "latest at" like
///   it's done for regular views.
/// - Static data is always shown.
/// - When both static and non-static data exist for the same entity/component, the non-static data
///   is never shown (as per our data model).

fn entity_and_time_vs_component_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query: &ViewQuery<'_>,
    group_by: TableGroupBy,
    sort_order: SortOrder,
) {
    re_tracing::profile_function!();

    //
    // Produce a sorted list of all components we are interested id.
    //

    // TODO(ab): add support for filtering components more narrowly.
    let sorted_components: BTreeSet<_> = {
        re_tracing::profile_scope!("query components");

        // Produce a sorted list of all components that are present in one or more entities.
        // These will be the columns of the table.
        query
            .iter_all_data_results()
            .filter(|data_result| data_result.is_visible(ctx))
            .flat_map(|data_result| {
                ctx.recording_store()
                    .all_components(&query.timeline, &data_result.entity_path)
                    .unwrap_or_default()
            })
            // TODO(#4466): make showing/hiding indicators components an explicit optional
            .filter(|comp| !comp.is_indicator_component())
            .collect()
    };

    //
    // Build the full list of rows, along with the chunk where the data is. Rows are keyed by an
    // (entity, time, row_id) tuple (see function docstring). These keys are mapped to the
    // corresponding chunk that contains the actual data.
    //
    // We build a big, monolithic iterator for all the rows. The following code builds that by
    // breaking it into several functions for sanity purpose, from innermost to outermost.
    //

    type RowKey = (EntityPath, TimeInt, RowId);

    #[inline]
    fn map_chunk_indices_to_key_value_iter<'a>(
        indices_iter: impl Iterator<Item = (TimeInt, RowId)> + 'a,
        chunk: Arc<Chunk>,
        entity_path: EntityPath,
        resolved_time_range: ResolvedTimeRange,
    ) -> impl Iterator<Item = (RowKey, Arc<Chunk>)> + 'a {
        indices_iter
            .filter(move |(time, _)| time.is_static() || resolved_time_range.contains(*time))
            .map(move |(time, row_id)| {
                let chunk = chunk.clone();
                ((entity_path.clone(), time, row_id), chunk)
            })
    }

    #[inline]
    fn entity_components_to_key_value_iter<'a>(
        ctx: &ViewerContext<'_>,
        entity_path: &'a EntityPath,
        component: &'a ComponentName,
        timeline: Timeline,
        resolved_time_range: ResolvedTimeRange,
    ) -> impl Iterator<Item = (RowKey, Arc<Chunk>)> + 'a {
        let range_query = RangeQuery::new(timeline, resolved_time_range);

        ctx.recording_store()
            .range_relevant_chunks(&range_query, entity_path, *component)
            .into_iter()
            // This does two things:
            // 1) Filter out instances where `chunk.iter_indices()` returns `None`.
            // 2) Exploit the fact that the returned iterator (if any) is *not* bound to the
            //    lifetime of the chunk (it has an internal Arc).
            .filter_map(move |chunk| {
                chunk
                    .clone()
                    .iter_indices(&timeline)
                    .map(|iter_indices| (iter_indices, chunk))
            })
            .flat_map(move |(indices_iter, chunk)| {
                map_chunk_indices_to_key_value_iter(
                    indices_iter,
                    chunk,
                    entity_path.clone(),
                    resolved_time_range,
                )
            })
    }

    // all the rows!
    let rows_to_chunk = query
        .iter_all_data_results()
        .filter(|data_result| data_result.is_visible(ctx))
        .flat_map(|data_result| {
            let time_range = match &data_result.property_overrides.query_range {
                QueryRange::TimeRange(time_range) => time_range.clone(),
                QueryRange::LatestAt => TimeRange::AT_CURSOR,
            };

            let resolved_time_range =
                ResolvedTimeRange::from_relative_time_range(&time_range, ctx.current_query().at());

            sorted_components.iter().flat_map(move |component| {
                entity_components_to_key_value_iter(
                    ctx,
                    &data_result.entity_path,
                    component,
                    query.timeline,
                    resolved_time_range,
                )
            })
        })
        .collect::<BTreeMap<_, _>>();

    //
    // Row sorting/grouping based on view properties.
    //

    let mut rows = rows_to_chunk.keys().collect::<Vec<_>>();

    // apply group_by
    match group_by {
        TableGroupBy::Entity => {} // already correctly sorted
        TableGroupBy::Time => rows.sort_by_key(|(entity_path, time, _)| (*time, entity_path)),
    };
    if sort_order == SortOrder::Descending {
        rows.reverse();
    }

    //
    // Drawing code.
    //

    let entity_header = |ui: &mut egui::Ui| {
        ui.strong("Entity");
    };
    let time_header = |ui: &mut egui::Ui| {
        ui.strong("Time");
    };

    // Draw the header row.
    let header_ui = |mut row: egui_extras::TableRow<'_, '_>| {
        match group_by {
            TableGroupBy::Entity => {
                row.col(entity_header);
                row.col(time_header);
            }
            TableGroupBy::Time => {
                row.col(time_header);
                row.col(entity_header);
            }
        }

        row.col(|ui| {
            ui.strong("Row ID");
        });

        for comp in &sorted_components {
            row.col(|ui| {
                ui.strong(comp.short_name());
            });
        }
    };

    let latest_at_query = query.latest_at_query();
    let entity_ui = |ui: &mut egui::Ui, entity_path: &EntityPath| {
        entity_path_button(
            ctx,
            &latest_at_query,
            ctx.recording(),
            ui,
            Some(query.space_view_id),
            entity_path,
        );
    };

    let time_ui = |ui: &mut egui::Ui, time: &TimeInt| {
        ui.label(
            query
                .timeline
                .typ()
                .format(*time, ctx.app_options.time_zone),
        );
    };

    // Draw a single line of the table. This is called for each _visible_ row.
    let latest_at_query = query.latest_at_query();
    let row_ui = |mut row: egui_extras::TableRow<'_, '_>| {
        let row_key = rows[row.index()];
        let row_chunk = &rows_to_chunk[row_key];
        let (entity_path, time, row_id) = row_key;

        match group_by {
            TableGroupBy::Entity => {
                row.col(|ui| entity_ui(ui, entity_path));
                row.col(|ui| time_ui(ui, time));
            }
            TableGroupBy::Time => {
                row.col(|ui| time_ui(ui, time));
                row.col(|ui| entity_ui(ui, entity_path));
            }
        };

        row.col(|ui| {
            row_id_ui(ui, row_id);
        });

        for component_name in &sorted_components {
            row.col(|ui| {
                let content = row_chunk.cell(*row_id, component_name);

                if let Some(content) = content {
                    ctx.component_ui_registry.ui_raw(
                        ctx,
                        ui,
                        UiLayout::List,
                        &latest_at_query,
                        ctx.recording(),
                        entity_path,
                        *component_name,
                        &*content,
                    );
                } else {
                    ui.weak("-");
                }
            });
        }
    };

    table_ui(ui, &sorted_components, header_ui, rows.len(), row_ui);
}

fn entity_and_instance_vs_component_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query: &ViewQuery<'_>,
) {
    re_tracing::profile_function!();

    // These are the entity paths whose content we must display.
    let sorted_entity_paths = sorted_visible_entity_path(ctx, query);
    let latest_at_query = query.latest_at_query();

    let sorted_instance_paths: Vec<_>;
    let sorted_components: BTreeSet<_>;
    {
        re_tracing::profile_scope!("query");

        // Produce a sorted list of each entity with all their instance keys. This will be the rows
        // of the table.
        //
        // Important: our semantics here differs from other built-in space views. "Out-of-bound"
        // instance keys (aka instance keys from a secondary component that cannot be joined with a
        // primary component) are not filtered out. Reasons:
        // - Primary/secondary component distinction only makes sense with archetypes, which we
        //   ignore. TODO(#4466): make archetypes more explicit?
        // - This space view is about showing all user data anyways.
        //
        // Note: this must be a `Vec<_>` because we need random access for `body.rows()`.
        sorted_instance_paths = sorted_entity_paths
            .iter()
            .flat_map(|entity_path| {
                sorted_instance_paths_for(
                    entity_path,
                    ctx.recording_store(),
                    &query.timeline,
                    &latest_at_query,
                )
            })
            .collect();

        // Produce a sorted list of all components that are present in one or more entities. These
        // will be the columns of the table.
        sorted_components = sorted_entity_paths
            .iter()
            .flat_map(|entity_path| {
                ctx.recording_store()
                    .all_components(&query.timeline, entity_path)
                    .unwrap_or_default()
            })
            // TODO(#4466): make showing/hiding indicators components an explicit optional
            .filter(|comp| !comp.is_indicator_component())
            .collect();
    }

    // Draw the header row.
    let header_ui = |mut row: egui_extras::TableRow<'_, '_>| {
        row.col(|ui| {
            ui.strong("Entity");
        });

        for comp in &sorted_components {
            row.col(|ui| {
                ui.strong(comp.short_name());
            });
        }
    };

    // Draw a single line of the table. This is called for each _visible_ row, so it's ok to
    // duplicate some of the querying.
    let row_ui = |mut row: egui_extras::TableRow<'_, '_>| {
        let instance = &sorted_instance_paths[row.index()];

        row.col(|ui| {
            instance_path_button(
                ctx,
                &latest_at_query,
                ctx.recording(),
                ui,
                Some(query.space_view_id),
                instance,
            );
        });

        for component_name in &sorted_components {
            row.col(|ui| {
                let results = ctx.recording().query_caches().latest_at(
                    ctx.recording_store(),
                    &latest_at_query,
                    &instance.entity_path,
                    [*component_name],
                );

                if let Some(results) =
                    // This is a duplicate of the one above, but this is ok since this codes runs
                    // *only* for visible rows.
                    results.components.get(component_name)
                {
                    ctx.component_ui_registry.ui(
                        ctx,
                        ui,
                        UiLayout::List,
                        &latest_at_query,
                        ctx.recording(),
                        &instance.entity_path,
                        results,
                        &instance.instance,
                    );
                } else {
                    ui.weak("-");
                }
            });
        }
    };

    table_ui(
        ui,
        &sorted_components,
        header_ui,
        sorted_instance_paths.len(),
        row_ui,
    );
}

// -------------------------------------------------------------------------------------------------
// Utilities

/// Display a nicely configured table with the provided header ui, row ui, and row count.
fn table_ui(
    ui: &mut egui::Ui,
    sorted_components: &BTreeSet<ComponentName>,
    header_ui: impl FnOnce(egui_extras::TableRow<'_, '_>),
    row_count: usize,
    row_ui: impl FnMut(TableRow<'_, '_>),
) {
    re_tracing::profile_function!();

    egui::ScrollArea::horizontal()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            egui::Frame {
                inner_margin: egui::Margin::same(5.0),
                ..Default::default()
            }
            .show(ui, |ui| {
                egui_extras::TableBuilder::new(ui)
                    .columns(
                        Column::auto_with_initial_suggestion(200.0).clip(true),
                        3 + sorted_components.len(),
                    )
                    .resizable(true)
                    .vscroll(true)
                    //TODO(ab): remove when https://github.com/emilk/egui/pull/4817 is merged/released
                    .max_scroll_height(f32::INFINITY)
                    .auto_shrink([false, false])
                    .striped(true)
                    .header(re_ui::DesignTokens::table_line_height(), header_ui)
                    .body(|body| {
                        body.rows(re_ui::DesignTokens::table_line_height(), row_count, row_ui);
                    });
            });
        });
}

fn row_id_ui(ui: &mut egui::Ui, row_id: &RowId) {
    let s = row_id.to_string();
    let split_pos = s.char_indices().nth_back(5);

    ui.label(match split_pos {
        Some((pos, _)) => &s[pos..],
        None => &s,
    })
    .on_hover_text(s);
}

/// Returns a sorted list of all entities that are visible in the view.
fn sorted_visible_entity_path(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
) -> BTreeSet<EntityPath> {
    query
        .iter_all_data_results()
        .filter(|data_result| data_result.is_visible(ctx))
        .map(|data_result| &data_result.entity_path)
        .cloned()
        .collect()
}

/// Returns a sorted, deduplicated iterator of all instance paths for a given entity.
fn sorted_instance_paths_for<'a>(
    entity_path: &'a EntityPath,
    store: &'a ChunkStore,
    timeline: &'a Timeline,
    latest_at_query: &'a LatestAtQuery,
) -> impl Iterator<Item = InstancePath> + 'a {
    store
        .all_components(timeline, entity_path)
        .unwrap_or_default()
        .into_iter()
        .filter(|component_name| !component_name.is_indicator_component())
        .flat_map(|component_name| {
            let num_instances = store
                .latest_at_relevant_chunks(latest_at_query, entity_path, component_name)
                .into_iter()
                .filter_map(|chunk| {
                    let (data_time, row_id, batch) = chunk
                        .latest_at(latest_at_query, component_name)
                        .iter_rows(timeline, &component_name)
                        .next()?;
                    batch.map(|batch| (data_time, row_id, batch))
                })
                .max_by_key(|(data_time, row_id, _)| (*data_time, *row_id))
                .map_or(0, |(_, _, batch)| batch.len());
            (0..num_instances).map(|i| Instance::from(i as u64))
        })
        .collect::<BTreeSet<_>>() // dedup and sort
        .into_iter()
        .map(|instance| InstancePath::instance(entity_path.clone(), instance))
}
