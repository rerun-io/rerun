use re_viewer_context::{UiLayout, ViewerContext};

impl crate::DataUi for re_log_types::StoreId {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        if let Some(entity_db) = ctx.storage_context.bundle.get(self) {
            entity_db.data_ui(ctx, ui, ui_layout, query, db);
        } else {
            ui_layout.label(ui, "<unknown store>").on_hover_ui(|ui| {
                ui.label(format!("{self:?}"));
            });
        }
    }
}
