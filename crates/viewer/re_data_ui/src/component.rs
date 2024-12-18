use egui::NumExt;

use re_chunk_store::UnitChunkShared;
use re_entity_db::InstancePath;
use re_log_types::{ComponentPath, Instance, TimeInt};
use re_ui::{ContextExt as _, SyntaxHighlighting as _, UiExt};
use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;
use crate::item_ui;

/// All the values of a specific [`re_log_types::ComponentPath`].
pub struct ComponentPathLatestAtResults<'a> {
    pub component_path: ComponentPath,
    pub unit: &'a UnitChunkShared,
}

impl DataUi for ComponentPathLatestAtResults<'_> {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        re_tracing::profile_function!(self.component_path.component_name);

        let ComponentPath {
            entity_path,
            component_name,
        } = &self.component_path;

        let Some(num_instances) = self
            .unit
            .component_batch_raw(component_name)
            .map(|data| data.len())
        else {
            ui.weak("<pending>");
            return;
        };

        // in some cases, we don't want to display all instances
        let max_row = match ui_layout {
            UiLayout::List => 0,
            UiLayout::Tooltip => num_instances.at_most(4), // includes "…x more" if any
            UiLayout::SelectionPanel => num_instances,
        };

        let engine = db.storage_engine();

        // Display data time and additional diagnostic information for static components.
        if !ui_layout.is_single_line() {
            let time = self
                .unit
                .index(&query.timeline())
                .map_or(TimeInt::STATIC, |(time, _)| time);

            // if the component is static, we display extra diagnostic information
            if time.is_static() {
                let static_message_count = engine
                    .store()
                    .num_static_events_for_component(entity_path, *component_name);
                if static_message_count > 1 {
                    ui.label(ui.ctx().warning_text(format!(
                        "Static component value was overridden {} times",
                        static_message_count.saturating_sub(1),
                    )))
                    .on_hover_text(
                        "When a static component is logged multiple times, only the last value \
                        is stored. Previously logged values are overwritten and not \
                        recoverable.",
                    );
                }

                let temporal_message_count = engine
                    .store()
                    .num_temporal_events_for_component_on_all_timelines(
                        entity_path,
                        *component_name,
                    );
                if temporal_message_count > 0 {
                    ui.error_label(format!(
                        "Static component has {} event{} logged on timelines",
                        temporal_message_count,
                        if temporal_message_count > 1 { "s" } else { "" }
                    ))
                    .on_hover_text(
                        "Components should be logged either as static or on timelines, but \
                        never both. Values for static components logged to timelines cannot be \
                        displayed.",
                    );
                }
            } else {
                let formatted_time = query
                    .timeline()
                    .typ()
                    .format(time, ctx.app_options.time_zone);
                ui.horizontal(|ui| {
                    ui.add(re_ui::icons::COMPONENT_TEMPORAL.as_image());
                    ui.label(format!("Temporal component at {formatted_time}"));
                });
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
                entity_path,
                *component_name,
                self.unit,
                &Instance::from(0),
            );
        } else if ui_layout.is_single_line() {
            ui.label(format!("{} values", re_format::format_uint(num_instances)));
        } else {
            ui_layout
                .table(ui)
                .resizable(false)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(egui_extras::Column::auto())
                .column(egui_extras::Column::remainder())
                .header(re_ui::DesignTokens::table_header_height(), |mut header| {
                    re_ui::DesignTokens::setup_table_header(&mut header);
                    header.col(|ui| {
                        ui.label("Index");
                    });
                    header.col(|ui| {
                        ui.label(component_name.short_name());
                    });
                })
                .body(|mut body| {
                    re_ui::DesignTokens::setup_table_body(&mut body);
                    let row_height = re_ui::DesignTokens::table_line_height();
                    body.rows(row_height, num_displayed_rows, |mut row| {
                        let instance = Instance::from(row.index() as u64);
                        row.col(|ui| {
                            let instance_path =
                                InstancePath::instance(entity_path.clone(), instance);
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
                                entity_path,
                                *component_name,
                                self.unit,
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
