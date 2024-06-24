use re_types_core::ComponentName;
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;

impl DataUi for ComponentName {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
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

                if ui_layout.is_selection_panel() {
                    ui.label(format!("Full name: {}", self.full_name()));
                };

                // Only show the first line of the docs:
                if let Some(markdown) = ctx
                    .reflection
                    .components
                    .get(self)
                    .map(|info| info.docstring_md)
                {
                    if ui_layout.is_selection_panel() {
                        ui.markdown_ui(egui::Id::new((self, "full")), markdown);
                    } else if let Some(first_line) = markdown.lines().next() {
                        ui.markdown_ui(egui::Id::new((self, "first_line")), first_line);
                    }
                }

                if let Some(url) = self.doc_url() {
                    ui.re_hyperlink("Full documentation", url);
                }
            });
        }
    }
}
