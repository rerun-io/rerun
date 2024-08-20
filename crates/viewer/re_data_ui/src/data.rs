use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;

impl DataUi for re_types::datatypes::Mat3x3 {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        egui::Grid::new("mat3").num_columns(3).show(ui, |ui| {
            ui.monospace(self[0].to_string());
            ui.monospace(self[3].to_string());
            ui.monospace(self[6].to_string());
            ui.end_row();

            ui.monospace(self[1].to_string());
            ui.monospace(self[4].to_string());
            ui.monospace(self[7].to_string());
            ui.end_row();

            ui.monospace(self[2].to_string());
            ui.monospace(self[5].to_string());
            ui.monospace(self[8].to_string());
            ui.end_row();
        });
    }
}
