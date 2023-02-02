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
        crate::profile_function!(self.name().full_name());

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
                    egui_extras::TableBuilder::new(ui)
                        .resizable(false)
                        .vscroll(true)
                        .auto_shrink([false, true])
                        .max_scroll_height(300.0)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .columns(egui_extras::Column::auto(), 2)
                        .header(re_ui::ReUi::table_header_height(), |mut header| {
                            re_ui::ReUi::setup_table_header(&mut header);
                            header.col(|ui| {
                                ui.label("Instance key");
                            });
                            header.col(|ui| {
                                ui.label(self.name().short_name());
                            });
                        })
                        .body(|mut body| {
                            re_ui::ReUi::setup_table_body(&mut body);
                            let row_height = re_ui::ReUi::table_line_height();
                            body.rows(row_height, num_instances, |index, mut row| {
                                if let Some(instance_key) = self
                                    .iter_instance_keys()
                                    .ok()
                                    .and_then(|mut keys| keys.nth(index))
                                {
                                    row.col(|ui| {
                                        ui.label(instance_key.to_string());
                                    });
                                    row.col(|ui| {
                                        ctx.component_ui_registry.ui(
                                            ctx,
                                            ui,
                                            crate::ui::UiVerbosity::Small,
                                            query,
                                            self,
                                            &instance_key,
                                        );
                                    });
                                }
                            });
                        });
                }
            }
            Err(err) => {
                ui.label(ctx.re_ui.error_text(format!("Error: {err}")));
            }
        }
    }
}
