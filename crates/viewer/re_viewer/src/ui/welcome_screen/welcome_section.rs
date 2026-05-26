use re_ui::DesignTokens;

pub(super) const DOCS_URL: &str = "https://www.rerun.io/docs";
pub(super) const WELCOME_SCREEN_TITLE: &str = "The data layer for physical AI";
pub(super) const WELCOME_SCREEN_BULLET_TEXT: &[&str] = &[
    "Log multi-rate, multimodal data with the Rerun SDK in C++, Python, or Rust",
    "Visualize and explore live or recorded data across the pipeline",
    "Query with dataframes or SQL, and stream directly to training",
];

/// Show the welcome section.
pub(super) fn welcome_section_ui(ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        let (style, line_height) = if ui.available_width() > 400.0 {
            (DesignTokens::welcome_screen_h1(), 50.0)
        } else {
            (DesignTokens::welcome_screen_h2(), 36.0)
        };

        ui.add(
            egui::Label::new(
                egui::RichText::new(WELCOME_SCREEN_TITLE)
                    .strong()
                    .line_height(Some(line_height))
                    .text_style(style),
            )
            .wrap(),
        );
    });
}
