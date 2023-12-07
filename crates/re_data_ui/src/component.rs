use egui::NumExt;
use re_data_store::{EntityPath, InstancePath};
use re_query::ComponentWithInstances;
use re_types::ComponentName;
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::{table_for_verbosity, DataUi};
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
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        re_tracing::profile_function!(self.component_name().full_name());

        let instance_keys = self.component_data.instance_keys();
        let num_instances = self.num_instances();

        let one_line = match verbosity {
            UiVerbosity::Small => true,
            UiVerbosity::Reduced | UiVerbosity::LimitHeight | UiVerbosity::Full => false,
        };

        // in some cases, we don't want to display all instances
        let max_row = match verbosity {
            UiVerbosity::Small => 0,
            UiVerbosity::Reduced => num_instances.at_most(4), // includes "…x more" if any
            UiVerbosity::LimitHeight | UiVerbosity::Full => num_instances,
        };

        // Here we enforce that exactly `max_row` rows are displayed, which means that:
        // - For `num_instances == max_row`, then `max_row` rows are displayed.
        // - For `num_instances == max_row + 1`, then `max_row-1` rows are displayed and "…2 more"
        //   is appended.
        //
        // ┏━━━┳━━━┳━━━┳━━━┓
        // ┃ 3 ┃ 4 ┃ 5 ┃ 6 ┃ <- num_instances
        // ┗━━━┻━━━┻━━━┻━━━┛
        // ┌───┬───┬───┬───┐ ┐
        // │ x │ x │ x │ x │ │
        // ├───┼───┼───┼───┤ │
        // │ x │ x │ x │ x │ │
        // ├───┼───┼───┼───┤ ├─ max_row == 4
        // │ x │ x │ x │ x │ │
        // ├───┼───┼───┼───┤ │
        // │   │ x │…+2│…+3│ │
        // └───┴───┴───┴───┘ ┘
        let displayed_row = if num_instances <= max_row {
            num_instances
        } else {
            // this accounts for the "…x more" using a row and handles `num_instances == 0`
            max_row.saturating_sub(1)
        };

        if num_instances == 0 {
            ui.weak("(empty)");
        } else if num_instances == 1 {
            if let Some(instance_key) = instance_keys.first() {
                ctx.component_ui_registry.ui(
                    ctx,
                    ui,
                    verbosity,
                    query,
                    &self.entity_path,
                    &self.component_data,
                    instance_key,
                );
            } else {
                ui.label(ctx.re_ui.error_text("Error: missing instance key"));
            }
        } else if one_line {
            ui.label(format!(
                "{} values",
                re_format::format_number(num_instances)
            ));
        } else {
            table_for_verbosity(verbosity, ui)
                .resizable(false)
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
                    body.rows(row_height, displayed_row, |index, mut row| {
                        if let Some(instance_key) = instance_keys.get(index) {
                            row.col(|ui| {
                                let instance_path =
                                    InstancePath::instance(self.entity_path.clone(), *instance_key);
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
                                    instance_key,
                                );
                            });
                        }
                    });
                });
            if num_instances > displayed_row {
                ui.label(format!(
                    "…and {} more.",
                    re_format::format_number(num_instances - displayed_row)
                ));
            }
        }
    }
}
