use re_types_core::ComponentName;
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;

impl DataUi for ComponentName {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        if ui_layout == UiLayout::List {
            ui.label(self.full_name());
        } else {
            ui.scope(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(format!("Full name: {}", self.full_name()));

                if let Some(url) = self.doc_url() {
                    ui.re_hyperlink("Full documentation", url);
                }
            });
        }
    }
}
