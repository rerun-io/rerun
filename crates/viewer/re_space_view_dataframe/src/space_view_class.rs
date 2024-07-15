use std::collections::BTreeSet;

use egui_extras::Column;

use re_chunk_store::{ChunkStore, LatestAtQuery};
use re_data_ui::item_ui::instance_path_button;
use re_entity_db::InstancePath;
use re_log_types::{EntityPath, Instance, Timeline};
use re_types_core::SpaceViewClassIdentifier;
use re_viewer_context::{
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewState, SpaceViewSystemExecutionError,
    SystemExecutionOutput, UiLayout, ViewQuery, ViewerContext,
};

use crate::visualizer_system::EmptySystem;

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
        //TODO(ab): fix that icon
        &re_ui::icons::SPACE_VIEW_DATAFRAME
    }

    fn help_markdown(&self, _egui_ctx: &egui::Context) -> String {
        "# Dataframe view

Show the data contained in entities in a table. Each entity is represented by as many rows as it has instances."
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

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut dyn SpaceViewState,

        query: &ViewQuery<'_>,
        _system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        // These are the entity paths whose content we must display.
        let sorted_entity_paths: BTreeSet<_> = query
            .iter_all_data_results()
            .filter(|data_result| data_result.is_visible(ctx))
            .map(|data_result| &data_result.entity_path)
            .cloned()
            .collect();

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

            // Produce a sorted list of all components that are present in one or more entities. This
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
                instance_path_button(ctx, &latest_at_query, ctx.recording(), ui, None, instance);
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
                        // This is a duplicate of the one above, but this ok since this codes runs
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

        {
            re_tracing::profile_scope!("table UI");

            egui::ScrollArea::both()
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
                                1 + sorted_components.len(),
                            )
                            .resizable(true)
                            .vscroll(false)
                            .auto_shrink([false, true])
                            .striped(true)
                            .header(re_ui::DesignTokens::table_line_height(), header_ui)
                            .body(|body| {
                                body.rows(
                                    re_ui::DesignTokens::table_line_height(),
                                    sorted_instance_paths.len(),
                                    row_ui,
                                );
                            });
                    });
                });
        }

        Ok(())
    }
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
