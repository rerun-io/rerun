use itertools::Itertools as _;

use re_entity_db::EntityDb;
use re_log_types::ApplicationId;
use re_viewer_context::{SystemCommandSender as _, UiLayout, ViewerContext};

use crate::item_ui::entity_db_button_ui;

impl crate::DataUi for ApplicationId {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_data_store2::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        egui::Grid::new("application_id")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Application ID");
                ui.label(self.to_string());
                if self == &ctx.store_context.app_id {
                    ui.label("(active)");
                }
                ui.end_row();
            });

        if ui_layout == UiLayout::List {
            return;
        }

        // ---------------------------------------------------------------------

        // Find all recordings with this app id
        let recordings: Vec<&EntityDb> = ctx
            .store_context
            .bundle
            .recordings()
            .filter(|db| db.app_id() == Some(self))
            .sorted_by_key(|entity_db| entity_db.store_info().map(|info| info.started))
            .collect();

        // Using the same content ui also for tooltips even if it can't be interacted with.
        // (still displays the content we want)
        if !recordings.is_empty() {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                ui.add_space(8.0);
                ui.strong("Loaded recordings for this app");
                for entity_db in recordings {
                    entity_db_button_ui(ctx, ui, entity_db, true);
                }
            });
        }

        // ---------------------------------------------------------------------
        // do not show UI code in tooltips

        if ui_layout != UiLayout::Tooltip {
            ui.add_space(8.0);

            // ---------------------------------------------------------------------

            // Blueprint section.
            let active_blueprint = ctx.store_context.blueprint;
            let default_blueprint = ctx.store_context.hub.default_blueprint_for_app(self);

            let button = egui::Button::image_and_text(
                re_ui::icons::RESET.as_image(),
                "Reset to default blueprint",
            );

            let is_same_as_default = default_blueprint.map_or(false, |default_blueprint| {
                default_blueprint.latest_row_id() == active_blueprint.latest_row_id()
            });

            if is_same_as_default {
                ui.add_enabled(false, button)
                    .on_disabled_hover_text("No modifications have been made");
            } else if default_blueprint.is_none() {
                ui.add_enabled(false, button)
                    .on_disabled_hover_text("There's no default blueprint");
            } else {
                // The active blueprint is different from the default blueprint
                if ui
                    .add(button)
                    .on_hover_text("Reset to the default blueprint for this app")
                    .clicked()
                {
                    ctx.command_sender
                        .send_system(re_viewer_context::SystemCommand::ClearActiveBlueprint);
                }
            }

            if ui.add(egui::Button::image_and_text(
                re_ui::icons::RESET.as_image(),
                "Reset to heuristic blueprint",
            )).on_hover_text("Clear both active and default blueprint, and auto-generate a new blueprint based on heuristics").clicked() {
                ctx.command_sender
                    .send_system(re_viewer_context::SystemCommand::ClearAndGenerateBlueprint);
            }
        }
    }
}
