use re_log_types::{
    ArrowMsg, BlueprintActivationCommand, DataTable, LogMsg, SetStoreInfo, StoreInfo,
};
use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;
use crate::item_ui;

impl DataUi for LogMsg {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        match self {
            LogMsg::SetStoreInfo(msg) => msg.data_ui(ctx, ui, ui_layout, query, db),
            LogMsg::ArrowMsg(_, msg) => msg.data_ui(ctx, ui, ui_layout, query, db),
            LogMsg::BlueprintActivationCommand(BlueprintActivationCommand {
                blueprint_id,
                make_active,
                make_default,
            }) => {
                ui.label(format!(
                    "BlueprintActivationCommand({blueprint_id}, make_active: {make_active}, make_default: {make_default})"
                ));
            }
        }
    }
}

impl DataUi for SetStoreInfo {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        let SetStoreInfo { row_id: _, info } = self;
        let StoreInfo {
            application_id,
            store_id,
            cloned_from,
            started,
            store_source,
            is_official_example,
        } = info;

        let re_ui = &ctx.re_ui;

        ui.code("SetStoreInfo");

        egui::Grid::new("fields").num_columns(2).show(ui, |ui| {
            re_ui.grid_left_hand_label(ui, "application_id:");
            ui.label(application_id.to_string());
            ui.end_row();

            re_ui.grid_left_hand_label(ui, "store_id:");
            ui.label(format!("{store_id:?}"));
            ui.end_row();

            re_ui.grid_left_hand_label(ui, "cloned_from");
            if let Some(cloned_from) = cloned_from {
                crate::item_ui::store_id_button_ui(ctx, ui, cloned_from);
            }
            ui.end_row();

            re_ui.grid_left_hand_label(ui, "started:");
            ui.label(started.format(ctx.app_options.time_zone));
            ui.end_row();

            re_ui.grid_left_hand_label(ui, "store_source:");
            ui.label(format!("{store_source}"));
            ui.end_row();

            re_ui.grid_left_hand_label(ui, "is_official_example:");
            ui.label(format!("{is_official_example}"));
            ui.end_row();

            re_ui.grid_left_hand_label(ui, "store_kind:");
            ui.label(format!("{}", store_id.kind));
            ui.end_row();
        });
    }
}

impl DataUi for ArrowMsg {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let table = match DataTable::from_arrow_msg(self) {
            Ok(table) => table,
            Err(err) => {
                ui.label(
                    ctx.re_ui
                        .error_text(format!("Error parsing ArrowMsg: {err}")),
                );
                return;
            }
        };

        // TODO(cmc): Come up with something a bit nicer once data tables become a common sight.
        for row in table.to_rows() {
            match row {
                Ok(row) => {
                    egui::Grid::new("fields").num_columns(2).show(ui, |ui| {
                        ui.monospace("entity_path:");
                        item_ui::entity_path_button(ctx, query, db, ui, None, row.entity_path());
                        ui.end_row();

                        ui.monospace("time_point:");
                        row.timepoint().data_ui(ctx, ui, ui_layout, query, db);
                        ui.end_row();

                        ui.monospace("components:");
                        row.cells().data_ui(ctx, ui, ui_layout, query, db);
                        ui.end_row();
                    });
                }
                Err(err) => {
                    ui.label(ctx.re_ui.error_text(err.to_string()));
                }
            }
        }
    }
}
