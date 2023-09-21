use re_data_store::InstancePath;
use re_log_types::ComponentPath;
use re_query::get_component_with_instances;
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::DataUi;
use crate::item_ui;

const HIDDEN_COMPONENTS_FOR_ALL_VERBOSITY: &[&str] = &["rerun.components.InstanceKey"];
const HIDDEN_COMPONENTS_FOR_LOW_VERBOSITY: &[&str] = &[];

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

        let Some(mut components) = store.all_components(&query.timeline, entity_path) else {
            if ctx.store_db.entity_db.knows_of_entity(entity_path) {
                ui.label(format!(
                    "No components in entity {:?} on timeline {:?}",
                    entity_path,
                    query.timeline.name()
                ));
            } else {
                ui.label(format!("Unknown entity: {entity_path:?}"));
            }
            return;
        };
        components.sort();

        egui::Grid::new("entity_instance")
            .num_columns(2)
            .show(ui, |ui| {
                for component_name in components {
                    let Some((_, component_data)) =
                        get_component_with_instances(store, query, entity_path, component_name)
                    else {
                        continue; // no need to show components that are unset at this point in time
                    };

                    // Certain fields are hidden.
                    if HIDDEN_COMPONENTS_FOR_ALL_VERBOSITY.contains(&component_name.as_ref()) {
                        continue;
                    }
                    match verbosity {
                        UiVerbosity::Small | UiVerbosity::Reduced => {
                            if HIDDEN_COMPONENTS_FOR_LOW_VERBOSITY
                                .contains(&component_name.as_ref())
                            {
                                continue;
                            }
                        }
                        UiVerbosity::All => {}
                    }

                    item_ui::component_path_button(
                        ctx,
                        ui,
                        &ComponentPath::new(entity_path.clone(), component_name),
                    );

                    if instance_key.is_splat() {
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
