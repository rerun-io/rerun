/// Show a minimal welcome section.
pub fn no_data_ui(ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        let (style, line_height) = if ui.available_width() > 400.0 {
            (re_ui::ReUi::welcome_screen_h1(), 50.0)
        } else {
            (re_ui::ReUi::welcome_screen_h2(), 36.0)
        };

        ui.add(
            egui::Label::new(
                egui::RichText::new("No Data Loaded")
                    .strong()
                    .line_height(Some(line_height))
                    .text_style(style),
            )
            .wrap(true),
        );
    });
}
