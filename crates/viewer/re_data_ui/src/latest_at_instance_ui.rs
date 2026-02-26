use egui::NumExt as _;
use re_chunk_store::UnitChunkShared;
use re_entity_db::InstancePath;
use re_format::format_plural_s;
use re_log_types::{EntityPath, Instance, TimePoint};
use re_sdk_types::ComponentIdentifier;
use re_ui::{SyntaxHighlighting as _, UiExt as _};
use re_viewer_context::{UiLayout, ViewerContext};

use crate::item_ui;

use super::DataUi;

/// All the values of a specific [`re_log_types::ComponentPath`].
#[derive(Clone)]
pub struct LatestAtInstanceResult<'a> {
    /// `camera / "left" / points / #42`
    pub entity_path: EntityPath,

    /// e.g. `Points3D:color`
    pub component: ComponentIdentifier,

    /// A specific instance (e.g. point in a point cloud), or [`Instance::ALL`] of them.
    pub instance: Instance,

    pub unit: &'a UnitChunkShared,
}

impl DataUi for LatestAtInstanceResult<'_> {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let Self {
            entity_path,
            component,
            instance,
            unit,
        } = self.clone();

        re_tracing::profile_function!(component);

        ui.sanity_check();

        let tokens = ui.tokens();

        let engine = db.storage_engine();

        let Some(component_descriptor) = engine
            .store()
            .entity_component_descriptor(&entity_path, component)
        else {
            ui.label(format!(
                "Entity {entity_path:?} has no component {component:?}"
            ));
            return;
        };

        let num_instances = unit.num_instances(component);

        // in some cases, we don't want to display all instances
        let max_row = match ui_layout {
            UiLayout::List => 0,
            UiLayout::Tooltip => num_instances.at_most(4), // includes "…x more" if any
            UiLayout::SelectionPanel => num_instances,
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

        if num_instances == 1 || instance.is_specific() {
            // Allow editing recording properties:
            if num_instances == 1
                && entity_path.starts_with(&EntityPath::properties())
                && let Some(array) = unit.component_batch_raw(component)
                && ctx.component_ui_registry().try_show_edit_ui(
                    ctx,
                    ui,
                    re_viewer_context::EditTarget {
                        store_id: db.store_id().clone(),
                        timepoint: TimePoint::STATIC,
                        entity_path: entity_path.clone(),
                    },
                    array.as_ref(),
                    component_descriptor.clone(),
                    !ui_layout.is_single_line(),
                ) != re_viewer_context::TryShowEditUiResult::NotShown
            {
                return;
            }

            ctx.component_ui_registry().component_ui(
                ctx,
                ui,
                ui_layout,
                query,
                db,
                &entity_path,
                &component_descriptor,
                unit,
                &instance,
            );
        } else if ui_layout.is_single_line() {
            ui.label(format_plural_s(num_instances, "value"));
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
                    body.rows(row_height, num_displayed_rows as _, |mut row| {
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
                                &entity_path,
                                &component_descriptor,
                                unit,
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
