use re_query::ComponentWithInstances;

use super::DataUi;

impl DataUi for ComponentWithInstances {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: super::UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let num_instances = self.len();

        let max_elems = match verbosity {
            crate::ui::UiVerbosity::Small | crate::ui::UiVerbosity::MaxHeight(_) => 1,
            crate::ui::UiVerbosity::Large => 20,
        };

        match self.iter_instance_keys() {
            Ok(mut instance_keys) => {
                if num_instances == 0 {
                    ui.weak("(empty)");
                } else if num_instances == 1 {
                    if let Some(instance) = instance_keys.next() {
                        ctx.component_ui_registry
                            .ui(ctx, ui, verbosity, query, self, &instance);
                    } else {
                        ui.label(ctx.re_ui.error_text("Error: missing instance key"));
                    }
                } else if num_instances <= max_elems {
                    egui::Grid::new("component_instances")
                        .num_columns(2)
                        .show(ui, |ui| {
                            for instance in instance_keys {
                                ui.label(instance.to_string());
                                ctx.component_ui_registry
                                    .ui(ctx, ui, verbosity, query, self, &instance);
                                ui.end_row();
                            }
                        });
                } else {
                    ui.label(format!("{num_instances} values"));
                }
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(format!("Error: {err}")));
            }
        }
    }
}
