use re_ui::{DesignTokens, UiExt as _};

pub(super) const DOCS_URL: &str = "https://www.rerun.io/docs";
pub(super) const WELCOME_SCREEN_TITLE: &str = "Visualize multimodal data";
pub(super) const WELCOME_SCREEN_BULLET_TEXT: &[&str] = &[
    "Log data with the Rerun SDK in C++, Python, or Rust",
    "Visualize and explore live or recorded data",
    "Configure the viewer interactively or through code",
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

        ui.add_space(18.0);

        let bullet_text = |ui: &mut egui::Ui, text: &str| {
            ui.horizontal(|ui| {
                ui.add_space(1.0);
                ui.bullet(ui.visuals().strong_text_color());
                ui.add_space(5.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(text)
                            .color(ui.visuals().widgets.active.text_color())
                            .text_style(DesignTokens::welcome_screen_body()),
                    )
                    .wrap(),
                );
            });
            ui.add_space(4.0);
        };

        for text in WELCOME_SCREEN_BULLET_TEXT {
            bullet_text(ui, text);
        }

        ui.add_space(9.0);

        ui.scope(|ui| {
            ui.style_mut().override_text_style = Some(DesignTokens::welcome_screen_body());
            ui.re_hyperlink("Go to documentation", DOCS_URL, true);
        });
    });
}
