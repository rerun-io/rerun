use std::collections::BTreeSet;

use egui_extras::Column;

use re_data_store::{DataStore, LatestAtQuery};
use re_data_ui::item_ui::instance_path_button;
use re_entity_db::{EntityProperties, InstancePath};
use re_log_types::{EntityPath, Timeline};
use re_query::get_component_with_instances;
use re_viewer_context::{
    SpaceViewClass, SpaceViewClassIdentifier, SpaceViewClassRegistryError, SpaceViewState,
    SpaceViewSystemExecutionError, SystemExecutionOutput, UiVerbosity, ViewQuery, ViewerContext,
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

    fn help_text(&self, _re_ui: &re_ui::ReUi) -> egui::WidgetText {
        "Show the data contained in entities in a table.\n\n\
        Each entity is represented by as many rows as it has instances. This includes out-of-bound \
        instances—instances from secondary components that cannot be joined to the primary \
        component—that are typically not represented in other space views. Also, splats are merged \
        into the entity's instance."
            .into()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<EmptySystem>()
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
        _root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
        _system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        // These are the entity paths whose content we must display.
        let sorted_entity_paths: BTreeSet<_> = query
            .iter_all_data_results()
            .filter(|data_result| data_result.accumulated_properties().visible)
            .map(|data_result| &data_result.entity_path)
            .cloned()
            .collect();

        let store = ctx.entity_db.store();
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
                    sorted_instance_paths_for(entity_path, store, &query.timeline, &latest_at_query)
                })
                .collect();

            // Produce a sorted list of all components that are present in one or more entities. This
            // will be the columns of the table.
            sorted_components = sorted_entity_paths
                .iter()
                .flat_map(|entity_path| {
                    store
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

            // TODO(#4466): make it explicit if that instance key is "out
            // of bounds" (aka cannot be joined to a primary component).

            row.col(|ui| {
                instance_path_button(ctx, &latest_at_query, store, ui, None, instance);
            });

            for comp in &sorted_components {
                row.col(|ui| {
                    // TODO(#4466): make it explicit if that value results
                    // from a splat joint.

                    if let Some((_, _, comp_inst)) =
                        // This is a duplicate of the one above, but this ok since this codes runs
                        // *only* for visible rows.
                        get_component_with_instances(
                            store,
                            &latest_at_query,
                            &instance.entity_path,
                            *comp,
                        )
                    {
                        ctx.component_ui_registry.ui(
                            ctx,
                            ui,
                            UiVerbosity::Small,
                            &latest_at_query,
                            store,
                            &instance.entity_path,
                            &comp_inst,
                            &instance.instance_key,
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
                    ui.style_mut().wrap = Some(false);

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
                            .header(re_ui::ReUi::table_line_height(), header_ui)
                            .body(|body| {
                                body.rows(
                                    re_ui::ReUi::table_line_height(),
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
///
/// This includes _any_ instance key in all components logged under this entity path, excluding
/// splats.
fn sorted_instance_paths_for<'a>(
    entity_path: &'a EntityPath,
    store: &'a DataStore,
    timeline: &'a Timeline,
    latest_at_query: &'a LatestAtQuery,
) -> impl Iterator<Item = InstancePath> + 'a {
    store
        .all_components(timeline, entity_path)
        .unwrap_or_default()
        .into_iter()
        .filter(|comp| !comp.is_indicator_component())
        .flat_map(|comp| {
            get_component_with_instances(store, latest_at_query, entity_path, comp)
                .map(|(_, _, comp_inst)| comp_inst.instance_keys())
                .unwrap_or_default()
        })
        .filter(|instance_key| !instance_key.is_splat())
        .collect::<BTreeSet<_>>() // dedup and sort
        .into_iter()
        .map(|instance_key| InstancePath::instance(entity_path.clone(), instance_key))
}
