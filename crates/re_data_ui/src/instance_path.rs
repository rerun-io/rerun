use re_entity_db::InstancePath;
use re_log_types::ComponentPath;
use re_query::get_component_with_instances;
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::DataUi;
use crate::item_ui;

impl DataUi for InstancePath {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
        store: &re_data_store::DataStore,
    ) {
        let Self {
            entity_path,
            instance_key,
        } = self;

        let Some(components) = store.all_components(&query.timeline, entity_path) else {
            if ctx.entity_db.is_known_entity(entity_path) {
                // This is fine - e.g. we're looking at `/world` and the user has only logged to `/world/car`.
                ui.label(format!(
                    "No components logged on timeline {:?}",
                    query.timeline.name()
                ));
            } else {
                ui.label(
                    ctx.re_ui
                        .error_text(format!("Unknown entity: {entity_path:?}")),
                );
            }
            return;
        };

        let mut components = crate::ui_visible_components(&components);

        // Put indicator components first:
        components.sort_by_key(|c| !c.is_indicator_component());

        let split = components.partition_point(|c| c.is_indicator_component());
        let normal_components = components.split_off(split);
        let indicator_components = components;

        let show_indicator_comps = match verbosity {
            UiVerbosity::Small | UiVerbosity::Reduced => {
                // Skip indicator components in hover ui (unless there are no other
                // types of components).
                !normal_components.is_empty()
            }
            UiVerbosity::LimitHeight | UiVerbosity::Full => true,
        };

        // First show indicator components, outside the grid:
        if show_indicator_comps {
            for component_name in indicator_components {
                crate::temporary_style_ui_for_component(ui, &component_name, |ui| {
                    item_ui::component_path_button(
                        ctx,
                        ui,
                        &ComponentPath::new(entity_path.clone(), component_name),
                    );
                });
            }
        }

        // Now show the rest of the components:
        egui::Grid::new("components").num_columns(2).show(ui, |ui| {
            for component_name in normal_components {
                let Some((_, _, component_data)) =
                    get_component_with_instances(store, query, entity_path, component_name)
                else {
                    continue; // no need to show components that are unset at this point in time
                };

                crate::temporary_style_ui_for_component(ui, &component_name, |ui| {
                    item_ui::component_path_button(
                        ctx,
                        ui,
                        &ComponentPath::new(entity_path.clone(), component_name),
                    );
                });

                if instance_key.is_splat() {
                    super::component::EntityComponentWithInstances {
                        entity_path: entity_path.clone(),
                        component_data,
                    }
                    .data_ui(ctx, ui, UiVerbosity::Small, query, store);
                } else {
                    ctx.component_ui_registry.ui(
                        ctx,
                        ui,
                        UiVerbosity::Small,
                        query,
                        store,
                        entity_path,
                        &component_data,
                        instance_key,
                    );
                }

                ui.end_row();
            }
            Some(())
        });
    }
}
