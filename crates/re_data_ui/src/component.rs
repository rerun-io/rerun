use std::sync::Arc;

use egui::NumExt;

use re_entity_db::{
    external::re_query::CachedLatestAtComponentResults, EntityPath, InstancePath,
};
use re_types::ComponentName;
use re_ui::SyntaxHighlighting as _;
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::{table_for_verbosity, DataUi};
use crate::item_ui;

/// All the values of a specific [`re_log_types::ComponentPath`].
pub struct EntityLatestAtResults {
    pub entity_path: EntityPath,
    pub component_name: ComponentName,
    pub results: Arc<CachedLatestAtComponentResults>,
}

impl DataUi for EntityLatestAtResults {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        re_tracing::profile_function!(self.component_name);

        // TODO(#5607): what should happen if the promise is still pending?
        let Some(num_instances) = self
            .results
            .raw(db.resolver(), self.component_name)
            .map(|data| data.len())
        else {
            ui.weak("<pending>");
            return;
        };

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
        let num_displayed_rows = if num_instances <= max_row {
            num_instances
        } else {
            // this accounts for the "…x more" using a row and handles `num_instances == 0`
            max_row.saturating_sub(1)
        };

        if num_instances == 0 {
            ui.weak("(empty)");
        } else if num_instances == 1 {
            ctx.component_ui_registry.ui(
                ctx,
                ui,
                verbosity,
                query,
                db,
                &self.entity_path,
                &self.results,
                &re_types::components::InstanceKey(0),
            );
        } else if one_line {
            ui.label(format!("{} values", re_format::format_uint(num_instances)));
        } else {
            table_for_verbosity(verbosity, ui)
                .resizable(false)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(egui_extras::Column::auto())
                .column(egui_extras::Column::remainder())
                .header(re_ui::ReUi::table_header_height(), |mut header| {
                    re_ui::ReUi::setup_table_header(&mut header);
                    header.col(|ui| {
                        ui.label("Index");
                    });
                    header.col(|ui| {
                        ui.label(self.component_name.short_name());
                    });
                })
                .body(|mut body| {
                    re_ui::ReUi::setup_table_body(&mut body);
                    let row_height = re_ui::ReUi::table_line_height();
                    body.rows(row_height, num_displayed_rows, |mut row| {
                        let instance_key = re_types::components::InstanceKey(row.index() as _);
                        row.col(|ui| {
                            let instance_path =
                                InstancePath::instance(self.entity_path.clone(), instance_key);
                            item_ui::instance_path_button_to(
                                ctx,
                                query,
                                db,
                                ui,
                                None,
                                &instance_path,
                                instance_key.syntax_highlighted(ui.style()),
                            );
                        });
                        row.col(|ui| {
                            ctx.component_ui_registry.ui(
                                ctx,
                                ui,
                                UiVerbosity::Small,
                                query,
                                db,
                                &self.entity_path,
                                &self.results,
                                &instance_key,
                            );
                        });
                    });
                });

            if num_instances > num_displayed_rows {
                ui.label(format!(
                    "…and {} more.",
                    re_format::format_uint(num_instances - num_displayed_rows)
                ));
            }
        }
    }
}
