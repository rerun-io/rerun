use re_data_store::InstancePath;
use re_log_types::ComponentPath;
use re_query::get_component_with_instances;
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::DataUi;
use crate::item_ui;

impl DataUi for InstancePath {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let Self {
            entity_path,
            instance_key,
        } = self;

        let store = &ctx.store_db.entity_db.data_store;

        let Some(components) = store.all_components(&query.timeline, entity_path) else {
            if ctx.store_db.entity_db.is_known_entity(entity_path) {
                ui.label(format!(
                    "No components in entity {:?} on timeline {:?}",
                    entity_path,
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

        let all_are_indicators = components.iter().all(|c| c.is_indicator_component());

        egui::Grid::new("entity_instance")
            .num_columns(2)
            .show(ui, |ui| {
                for &component_name in crate::ui_visible_components(&components) {
                    if verbosity != UiVerbosity::All
                        && component_name.is_indicator_component()
                        && !all_are_indicators
                    {
                        // Skip indicator components in hover ui (unless there are no other types of components).
                        continue;
                    }

                    let Some((_, component_data)) =
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

                    if let Some(archetype_name) = component_name.indicator_component_archetype() {
                        ui.weak(format!(
                            "Indicator component for the {archetype_name} archetype"
                        ));
                    } else if instance_key.is_splat() {
                        super::component::EntityComponentWithInstances {
                            entity_path: entity_path.clone(),
                            component_data,
                        }
                        .data_ui(ctx, ui, UiVerbosity::Small, query);
                    } else {
                        ctx.component_ui_registry.ui(
                            ctx,
                            ui,
                            UiVerbosity::Small,
                            query,
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
