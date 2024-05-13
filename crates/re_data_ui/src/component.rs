use std::sync::Arc;

use egui::NumExt;

use re_entity_db::{external::re_query::LatestAtComponentResults, EntityPath, InstancePath};
use re_log_types::Instance;
use re_types::ComponentName;
use re_ui::SyntaxHighlighting as _;
use re_viewer_context::{UiLayout, ViewerContext};

use super::{table_for_ui_layout, DataUi};
use crate::item_ui;

/// All the values of a specific [`re_log_types::ComponentPath`].
pub struct EntityLatestAtResults {
    pub entity_path: EntityPath,
    pub component_name: ComponentName,
    pub results: Arc<LatestAtComponentResults>,
}

impl DataUi for EntityLatestAtResults {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
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

        let one_line = match ui_layout {
            UiLayout::List => true,
            UiLayout::Tooltip
            | UiLayout::SelectionPanelLimitHeight
            | UiLayout::SelectionPanelFull => false,
        };

        // in some cases, we don't want to display all instances
        let max_row = match ui_layout {
            UiLayout::List => 0,
            UiLayout::Tooltip => num_instances.at_most(4), // includes "…x more" if any
            UiLayout::SelectionPanelLimitHeight | UiLayout::SelectionPanelFull => num_instances,
        };

        // Display data time and additional diagnostic information for static components.
        if ui_layout != UiLayout::List {
            ui.label(format!(
                "Data time: {}",
                query
                    .timeline()
                    .typ()
                    .format(self.results.index().0, ctx.app_options.time_zone),
            ));

            // if the component is static, we display extra diagnostic information
            if self.results.is_static() {
                if let Some(histogram) = db
                    .tree()
                    .subtree(&self.entity_path)
                    .and_then(|tree| tree.entity.components.get(&self.component_name))
                {
                    if histogram.num_static_messages() > 1 {
                        ui.label(ctx.re_ui.warning_text(format!(
                            "Static component value was overridden {} times",
                            histogram.num_static_messages().saturating_sub(1),
                        )))
                        .on_hover_text(
                            "When a static component is logged multiple times, only the last value \
                            is stored. Previously logged values are overwritten and not \
                            recoverable.",
                        );
                    }

                    let timeline_message_count = histogram.num_temporal_messages();
                    if timeline_message_count > 0 {
                        ui.label(ctx.re_ui.error_text(format!(
                            "Static component has {} event{} logged on timelines",
                            timeline_message_count,
                            if timeline_message_count > 1 { "s" } else { "" }
                        )))
                        .on_hover_text(
                            "Components should be logged either as static or on timelines, but \
                            never both. Values for static components logged to timelines cannot be \
                            displayed.",
                        );
                    }
                }
            }
        }

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
                ui_layout,
                query,
                db,
                &self.entity_path,
                &self.results,
                &Instance::from(0),
            );
        } else if one_line {
            ui.label(format!("{} values", re_format::format_uint(num_instances)));
        } else {
            table_for_ui_layout(ui_layout, ui)
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
                        let instance = Instance::from(row.index() as u64);
                        row.col(|ui| {
                            let instance_path =
                                InstancePath::instance(self.entity_path.clone(), instance);
                            item_ui::instance_path_button_to(
                                ctx,
                                query,
                                db,
                                ui,
                                None,
                                &instance_path,
                                instance.syntax_highlighted(ui.style()),
                            );
                        });
                        row.col(|ui| {
                            ctx.component_ui_registry.ui(
                                ctx,
                                ui,
                                UiLayout::List,
                                query,
                                db,
                                &self.entity_path,
                                &self.results,
                                &instance,
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
