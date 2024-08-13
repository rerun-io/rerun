use std::collections::BTreeSet;

use re_chunk_store::LatestAtQuery;
use re_data_ui::item_ui::instance_path_button;
use re_entity_db::InstancePath;
use re_log_types::Instance;
use re_viewer_context::{Item, UiLayout, ViewQuery, ViewerContext};

use crate::{
    table_ui::table_ui,
    utils::{sorted_instance_paths_for, sorted_visible_entity_path},
};

/// Display a "latest at" table.
///
/// This table has entity instances as rows and components as columns. That data is the result of a
/// "latest at" query based on the current timeline and time.
pub(crate) fn latest_at_table_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query: &ViewQuery<'_>,
    latest_at_query: &LatestAtQuery,
) {
    re_tracing::profile_function!();

    //
    // DATA
    //

    // These are the entity paths whose content we must display.
    let sorted_entity_paths = sorted_visible_entity_path(ctx, query);

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
                    &latest_at_query.timeline(),
                    latest_at_query,
                )
            })
            .collect();

        // Produce a sorted list of all components that are present in one or more entities. These
        // will be the columns of the table.
        sorted_components = sorted_entity_paths
            .iter()
            .flat_map(|entity_path| {
                ctx.recording_store()
                    .all_components_on_timeline(&latest_at_query.timeline(), entity_path)
                    .unwrap_or_default()
            })
            // TODO(#4466): make showing/hiding indicators components an explicit optional
            .filter(|comp| !comp.is_indicator_component())
            .collect();
    }

    //
    // SCROLL TO ROW
    //

    let index_for_instance_path = |instance_path: &InstancePath| {
        let instance_path = if instance_path.instance == Instance::ALL {
            InstancePath::instance(instance_path.entity_path.clone(), 0.into())
        } else {
            instance_path.clone()
        };

        sorted_instance_paths.binary_search(&instance_path).ok()
    };

    let scroll_to_row = ctx.focused_item.as_ref().and_then(|item| match item {
        Item::AppId(_)
        | Item::DataSource(_)
        | Item::StoreId(_)
        | Item::ComponentPath(_) //TODO(ab): implement scroll to column?
        | Item::SpaceView(_)
        | Item::Container(_) => None,

        Item::InstancePath(instance_path) => index_for_instance_path(instance_path),
        Item::DataResult(space_view_id, instance_path) => {
            // We want to scroll only if the focus action originated from outside the table. We
            // allow the case of `Instance::ALL` for when the entity is double-clicked in the
            // blueprint tree.
            //TODO(#6906): we should have an explicit way to track the "source" of the focus event.
            let should_scroll =
                (space_view_id != &query.space_view_id) || instance_path.instance.is_all();

            should_scroll
                .then(|| index_for_instance_path(instance_path))
                .flatten()
        }
    });

    //
    // DRAW
    //

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
        let instance_path = &sorted_instance_paths[row.index()];

        row.col(|ui| {
            instance_path_button(
                ctx,
                latest_at_query,
                ctx.recording(),
                ui,
                Some(query.space_view_id),
                instance_path,
            );
        });

        // Note: a lot of duplicated querying potentially happens here, but this is ok since this
        // code runs *only* for visible rows.
        for component_name in &sorted_components {
            row.col(|ui| {
                // TODO(ab, cmc): use the suitable API from re_query when it becomes available.

                let result = ctx
                    .recording_store()
                    .latest_at_relevant_chunks(
                        latest_at_query,
                        &instance_path.entity_path,
                        *component_name,
                    )
                    .into_iter()
                    .filter_map(|chunk| {
                        let (index, unit) = chunk
                            .latest_at(latest_at_query, *component_name)
                            .into_unit()
                            .and_then(|unit| {
                                unit.index(&latest_at_query.timeline())
                                    .map(|index| (index, unit))
                            })?;

                        unit.component_batch_raw(component_name)
                            .map(|array| (index, array))
                    })
                    .max_by_key(|(index, _array)| *index);

                // TODO(#4466): it would be nice to display the time and row id somewhere, since we
                //              have them.
                if let Some(((_time, row_id), array)) = result {
                    let instance_index = instance_path.instance.get() as usize;

                    if array.is_empty() {
                        ui.weak("-");
                    } else {
                        let (data, clamped) = if instance_index >= array.len() {
                            (array.sliced(array.len() - 1, 1), true)
                        } else {
                            (array.sliced(instance_index, 1), false)
                        };

                        ui.add_enabled_ui(!clamped, |ui| {
                            ctx.component_ui_registry.ui_raw(
                                ctx,
                                ui,
                                UiLayout::List,
                                latest_at_query,
                                ctx.recording(),
                                &instance_path.entity_path,
                                *component_name,
                                Some(row_id),
                                &*data,
                            );
                        });
                    }
                } else {
                    ui.weak("-");
                }
            });
        }
    };

    table_ui(
        ui,
        &sorted_components,
        1, // entity column
        header_ui,
        sorted_instance_paths.len(),
        row_ui,
        scroll_to_row,
    );
}
