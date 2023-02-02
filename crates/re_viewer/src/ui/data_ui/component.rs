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
            crate::UiVerbosity::Reduced => 10,
            crate::ui::UiVerbosity::All => 20,
        };

        match self.iter_instance_keys() {
            Ok(mut instance_keys) => {
                if num_instances == 0 {
                    ui.weak("(empty)");
                } else if num_instances == 1 {
                    if let Some(instance_key) = instance_keys.next() {
                        ctx.component_ui_registry.ui(
                            ctx,
                            ui,
                            verbosity,
                            query,
                            self,
                            &instance_key,
                        );
                    } else {
                        ui.label(ctx.re_ui.error_text("Error: missing instance key"));
                    }
                } else if max_elems == 1 {
                    ui.label(format!("{num_instances} values"));
                } else {
                    egui::Grid::new("component_instances")
                        .num_columns(2)
                        .show(ui, |ui| {
                            for instance_key in instance_keys.take(max_elems) {
                                ui.label(instance_key.to_string());
                                ctx.component_ui_registry.ui(
                                    ctx,
                                    ui,
                                    verbosity,
                                    query,
                                    self,
                                    &instance_key,
                                );
                                ui.end_row();
                            }
                        });
                    // TODO(andreas): There should be a button leaving to a full view.
                    //                  Or once we figure out how to do full views of this just show everything in a scroll area
                    if num_instances > max_elems {
                        ui.label(format!("...plus {} more", num_instances - max_elems));
                    }
                }
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(format!("Error: {err}")));
            }
        }
    }
}
