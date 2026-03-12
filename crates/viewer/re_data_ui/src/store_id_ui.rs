use re_viewer_context::{AppContext, UiLayout};

impl crate::AppUi for re_log_types::StoreId {
    fn app_ui(&self, ctx: &AppContext<'_>, ui: &mut egui::Ui, ui_layout: UiLayout) {
        if let Some(entity_db) = ctx.store_bundle().get(self) {
            entity_db.app_ui(ctx, ui, ui_layout);
        } else {
            ui_layout.label(ui, "<unknown store>").on_hover_ui(|ui| {
                ui.label(format!("{self:?}"));
            });
        }
    }
}
