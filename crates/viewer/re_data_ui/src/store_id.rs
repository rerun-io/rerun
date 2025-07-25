use re_viewer_context::{UiLayout, ViewerContext};

impl crate::DataUi for re_log_types::StoreId {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        crate::item_ui::store_id_button_ui(ctx, ui, self, ui_layout);
    }
}
