use egui::Ui;
use re_ui::{DesignTokens, UiExt as _};

/// Show a minimal welcome section.
pub fn no_data_ui(ui: &mut egui::Ui) {
    ui.center("no_data_ui_contents", |ui| {
        ui.add(
            egui::Label::new(
                egui::RichText::new(super::welcome_section::WELCOME_SCREEN_TITLE)
                    .weak()
                    .line_height(Some(36.0))
                    .text_style(DesignTokens::welcome_screen_h2()),
            )
            .wrap(),
        );

        ui.add_space(10.0);

        let bullet_text = |ui: &mut Ui, text: &str| {
            ui.horizontal(|ui| {
                ui.add_space(1.0);
                ui.bullet(ui.visuals().weak_text_color());
                ui.add_space(5.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(text)
                            .color(ui.visuals().weak_text_color())
                            .text_style(DesignTokens::welcome_screen_body()),
                    )
                    .wrap(),
                );
            });
            ui.add_space(4.0);
        };

        for text in super::welcome_section::WELCOME_SCREEN_BULLET_TEXT {
            bullet_text(ui, text);
        }

        ui.add_space(9.0);
        if ui
            .button(
                egui::RichText::new("Go to documentation →")
                    .weak()
                    .text_style(DesignTokens::welcome_screen_body()),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            ui.open_url(egui::output::OpenUrl {
                url: super::welcome_section::DOCS_URL.to_owned(),
                new_tab: true,
            });
        }
    });
}
