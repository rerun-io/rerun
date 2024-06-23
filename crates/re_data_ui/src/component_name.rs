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

                // Only show the first line of the docs:
                if let Some(markdown) = re_types_registry::components::registry()
                    .get(self)
                    .and_then(|info| info.docstring_md.lines().next())
                {
                    ui.markdown_ui(egui::Id::new(self), markdown);
                }

                if let Some(url) = self.doc_url() {
                    ui.re_hyperlink("Full documentation", url);
                }
            });
        }
    }
}
