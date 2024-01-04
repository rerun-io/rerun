use re_log_types::{ArrowMsg, DataTable, LogMsg, SetStoreInfo, StoreInfo};
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::DataUi;
use crate::item_ui;

impl DataUi for LogMsg {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
    ) {
        match self {
            LogMsg::SetStoreInfo(msg) => msg.data_ui(ctx, ui, verbosity, query),
            LogMsg::ArrowMsg(_, msg) => msg.data_ui(ctx, ui, verbosity, query),
        }
    }
}

impl DataUi for SetStoreInfo {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
    ) {
        ui.code("SetStoreInfo");
        let SetStoreInfo { row_id: _, info } = self;
        let StoreInfo {
            application_id,
            store_id,
            started,
            store_source,
            is_official_example,
            store_kind,
        } = info;

        egui::Grid::new("fields").num_columns(2).show(ui, |ui| {
            ui.monospace("application_id:");
            ui.label(application_id.to_string());
            ui.end_row();

            ui.monospace("store_id:");
            ui.label(format!("{store_id:?}"));
            ui.end_row();

            ui.monospace("started:");
            ui.label(started.format(_ctx.app_options.time_zone_for_timestamps));
            ui.end_row();

            ui.monospace("store_source:");
            ui.label(format!("{store_source}"));
            ui.end_row();

            ui.monospace("is_official_example:");
            ui.label(format!("{is_official_example}"));
            ui.end_row();

            ui.monospace("store_kind:");
            ui.label(format!("{store_kind}"));
            ui.end_row();
        });
    }
}

impl DataUi for ArrowMsg {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
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
                        item_ui::entity_path_button(ctx, ui, None, row.entity_path());
                        ui.end_row();

                        ui.monospace("time_point:");
                        row.timepoint().data_ui(ctx, ui, verbosity, query);
                        ui.end_row();

                        ui.monospace("components:");
                        row.cells().data_ui(ctx, ui, verbosity, query);
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
