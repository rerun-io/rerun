use egui::NumExt as _;
use re_chunk_store::UnitChunkShared;
use re_entity_db::InstancePath;
use re_log_types::{ComponentPath, EntityPath, Instance, TimeInt, TimePoint};
use re_ui::{SyntaxHighlighting as _, UiExt as _};
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
        re_tracing::profile_function!(self.component_path.component);

        ui.sanity_check();

        let tokens = ui.tokens();

        let ComponentPath {
            entity_path,
            component,
        } = &self.component_path;

        let engine = db.storage_engine();

        let component = *component;
        let Some(component_descriptor) = engine
            .store()
            .entity_component_descriptor(entity_path, component)
        else {
            ui.label(format!(
                "Entity {entity_path:?} has no component {component:?}"
            ));
            return;
        };

        let Some(num_instances) = self
            .unit
            .component_batch_raw(component)
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
                    .num_static_events_for_component(entity_path, component);
                if static_message_count > 1 {
                    ui.warning_label(format!(
                        "Static component value was overridden {} times.",
                        static_message_count.saturating_sub(1),
                    ))
                    .on_hover_text(
                        "When a static component is logged multiple times, only the last value \
                        is stored. Previously logged values are overwritten and not \
                        recoverable.",
                    );
                }

                let temporal_message_count = engine
                    .store()
                    .num_temporal_events_for_component_on_all_timelines(entity_path, component);
                if temporal_message_count > 0 {
                    ui.error_label(format!(
                        "Static component has {} event{} logged on timelines.",
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
                let typ = db.timeline_type(&query.timeline());
                let formatted_time = typ.format(time, ctx.app_options().timestamp_format);
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

        if num_instances <= 1 {
            // Allow editing recording properties:
            if entity_path.starts_with(&EntityPath::properties())
                && let Some(array) = self.unit.component_batch_raw(component)
                && ctx.component_ui_registry().try_show_edit_ui(
                    ctx,
                    ui,
                    Some(re_viewer_context::EditTarget {
                        store_id: ctx.store_id().clone(),
                        timepoint: TimePoint::STATIC,
                        entity_path: entity_path.clone(),
                    }),
                    array.as_ref(),
                    component_descriptor.clone(),
                    !ui_layout.is_single_line(),
                )
            {
                return;
            }

            ctx.component_ui_registry().component_ui(
                ctx,
                ui,
                ui_layout,
                query,
                db,
                entity_path,
                &component_descriptor,
                self.unit,
                &Instance::from(0),
            );
        } else if ui_layout.is_single_line() {
            ui.label(format!("{} values", re_format::format_uint(num_instances)));
        } else {
            let table_style = re_ui::TableStyle::Dense;
            ui_layout
                .table(ui)
                .resizable(false)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(egui_extras::Column::auto())
                .column(egui_extras::Column::remainder())
                .header(tokens.deprecated_table_header_height(), |mut header| {
                    re_ui::DesignTokens::setup_table_header(&mut header);
                    header.col(|ui| {
                        ui.label("Index");
                    });
                    header.col(|ui| {
                        ui.label(component_descriptor.display_name());
                    });
                })
                .body(|mut body| {
                    tokens.setup_table_body(&mut body, table_style);
                    let row_height = tokens.table_row_height(table_style);
                    body.rows(row_height, num_displayed_rows, |mut row| {
                        let instance = Instance::from(row.index() as u64);
                        row.col(|ui| {
                            let instance_text = instance.syntax_highlighted(ui.style());
                            if ui.is_tooltip() {
                                // Avoids interactive tooltips,
                                // because that means they stick around when you move your mouse
                                ui.label(instance_text);
                            } else {
                                let instance_path =
                                    InstancePath::instance(entity_path.clone(), instance);
                                item_ui::instance_path_button_to(
                                    ctx,
                                    query,
                                    db,
                                    ui,
                                    None,
                                    &instance_path,
                                    instance_text,
                                );
                            }
                        });
                        row.col(|ui| {
                            ctx.component_ui_registry().component_ui(
                                ctx,
                                ui,
                                UiLayout::List,
                                query,
                                db,
                                entity_path,
                                &component_descriptor,
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

        ui.sanity_check();
    }
}
