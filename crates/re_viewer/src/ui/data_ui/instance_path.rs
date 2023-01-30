use re_data_store::InstancePath;
use re_log_types::ComponentPath;
use re_query::{get_component_with_instances, QueryError};

use crate::{
    misc::ViewerContext,
    ui::{format_component_name, UiVerbosity},
};

use super::{component::arrow_component_elem_ui, DataUi};

impl DataUi for InstancePath {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let store = &ctx.log_db.entity_db.arrow_store;

        let Some(mut components) = store.all_components(&query.timeline, &self.entity_path) else {
            ui.label("No Components");
            return ;
        };
        components.sort();

        egui::Grid::new("entity_instance")
            .num_columns(2)
            .show(ui, |ui| {
                for component_name in components {
                    let component_data = get_component_with_instances(
                        store,
                        query,
                        &self.entity_path,
                        component_name,
                    );

                    if matches!(component_data, Err(QueryError::PrimaryNotFound)) {
                        continue; // no need to show components that are unset at this point in time
                    }

                    ctx.component_path_button_to(
                        ui,
                        format_component_name(&component_name),
                        &ComponentPath::new(self.entity_path.clone(), component_name),
                    );

                    match component_data {
                        Err(err) => {
                            ui.label(format!("Error: {}", err)); // TODO: error formatting
                        }
                        Ok(component_data) => {
                            arrow_component_elem_ui(
                                ctx,
                                ui,
                                UiVerbosity::Small,
                                query,
                                &component_data,
                                &self.instance_index,
                            );
                        }
                    }

                    ui.end_row();
                }
                Some(())
            });
    }
}
