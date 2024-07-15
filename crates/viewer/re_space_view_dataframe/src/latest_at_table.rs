use std::collections::BTreeSet;

use re_data_ui::item_ui::instance_path_button;
use re_viewer_context::{UiLayout, ViewQuery, ViewerContext};

use crate::{
    table_ui::table_ui,
    utils::{sorted_instance_paths_for, sorted_visible_entity_path},
};

/// Display a "latest at" table.
///
/// This table has entity instances as rows and components as column. That data is the result of a
/// "latest at" query based on the current timeline and time.
pub(crate) fn latest_at_table_ui(
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
