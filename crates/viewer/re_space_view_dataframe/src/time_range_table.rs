use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use re_chunk_store::{Chunk, RangeQuery, RowId};
use re_data_ui::item_ui::entity_path_button;
use re_log_types::{EntityPath, ResolvedTimeRange, TimeInt, Timeline};
use re_types::blueprint::components::{SortOrder, TableGroupBy};
use re_types_core::datatypes::TimeRange;
use re_types_core::ComponentName;
use re_viewer_context::{QueryRange, UiLayout, ViewQuery, ViewerContext};

use crate::table_ui::{row_id_ui, table_ui};

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

pub(crate) fn time_range_table_ui(
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
