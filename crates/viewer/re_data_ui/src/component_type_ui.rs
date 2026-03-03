use re_types_core::ComponentType;
use re_ui::UiExt as _;
use re_viewer_context::{StoreViewContext, UiLayout};

use super::DataUi;

impl DataUi for ComponentType {
    fn data_ui(&self, ctx: &StoreViewContext<'_>, ui: &mut egui::Ui, ui_layout: UiLayout) {
        if ui_layout.is_single_line() {
            ui.label(self.full_name());
        } else {
            ui.scope(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

                if ui_layout.is_selection_panel() {
                    ui.label(format!("Full name: {}", self.full_name()));
                } else {
                    ui.label(self.full_name());
                }

                // Only show the first line of the docs:
                if let Some(markdown) = ctx
                    .app_ctx
                    .reflection
                    .components
                    .get(self)
                    .map(|info| info.docstring_md)
                {
                    if ui_layout.is_selection_panel() {
                        ui.markdown_ui(markdown);
                    } else if let Some(first_line) = markdown.lines().next() {
                        ui.markdown_ui(first_line);
                    }
                }

                if let Some(url) = self.doc_url() {
                    // Always open in a new tab
                    ui.re_hyperlink("Full documentation", url, true);
                }
            });
        }
    }
}
