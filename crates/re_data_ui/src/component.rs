use re_data_store::{ComponentName, EntityPath, InstancePath};
use re_query::ComponentWithInstances;
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::DataUi;
use crate::item_ui;

// We do NOT implement `DataUi` for just `ComponentWithInstances`
// because we also want the context of what entity it is part of!

/// All the values of a specific [`re_log_types::ComponentPath`].
pub struct EntityComponentWithInstances {
    pub entity_path: EntityPath,
    pub component_data: ComponentWithInstances,
}

impl EntityComponentWithInstances {
    pub fn component_name(&self) -> ComponentName {
        self.component_data.name()
    }

    pub fn num_instances(&self) -> usize {
        self.component_data.len()
    }
}

impl DataUi for EntityComponentWithInstances {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        crate::profile_function!(self.component_name().full_name());

        let mut instance_keys = match self.component_data.iter_instance_keys() {
            Ok(instance_keys) => instance_keys,
            Err(err) => {
                ui.label(ctx.re_ui.error_text(format!("Error: {err}")));
                return;
            }
        };

        let num_instances = self.num_instances();

        let one_line = match verbosity {
            UiVerbosity::Small => true,
            UiVerbosity::Reduced | UiVerbosity::All => false,
        };

        if num_instances == 0 {
            ui.weak("(empty)");
        } else if num_instances == 1 {
            if let Some(instance_key) = instance_keys.next() {
                ctx.component_ui_registry.ui(
                    ctx,
                    ui,
                    verbosity,
                    query,
                    &self.entity_path,
                    &self.component_data,
                    &instance_key,
                );
            } else {
                ui.label(ctx.re_ui.error_text("Error: missing instance key"));
            }
        } else if one_line {
            ui.label(format!("{num_instances} values"));
        } else {
            egui_extras::TableBuilder::new(ui)
                .resizable(false)
                .vscroll(true)
                .auto_shrink([false, true])
                .max_scroll_height(100.0)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .columns(egui_extras::Column::auto(), 2)
                .header(re_ui::ReUi::table_header_height(), |mut header| {
                    re_ui::ReUi::setup_table_header(&mut header);
                    header.col(|ui| {
                        ui.label("Instance Key");
                    });
                    header.col(|ui| {
                        ui.label(self.component_name().short_name());
                    });
                })
                .body(|mut body| {
                    re_ui::ReUi::setup_table_body(&mut body);
                    let row_height = re_ui::ReUi::table_line_height();
                    body.rows(row_height, num_instances, |index, mut row| {
                        if let Some(instance_key) = self
                            .component_data
                            .iter_instance_keys()
                            .ok()
                            .and_then(|mut keys| keys.nth(index))
                        {
                            row.col(|ui| {
                                let instance_path =
                                    InstancePath::instance(self.entity_path.clone(), instance_key);
                                item_ui::instance_path_button_to(
                                    ctx,
                                    ui,
                                    None,
                                    &instance_path,
                                    instance_key.to_string(),
                                );
                            });
                            row.col(|ui| {
                                ctx.component_ui_registry.ui(
                                    ctx,
                                    ui,
                                    UiVerbosity::Small,
                                    query,
                                    &self.entity_path,
                                    &self.component_data,
                                    &instance_key,
                                );
                            });
                        }
                    });
                });
        }
    }
}
