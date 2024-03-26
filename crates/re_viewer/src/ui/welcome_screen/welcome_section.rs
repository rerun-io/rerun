use egui::Ui;

const DOCS_URL: &str = "https://www.rerun.io/docs";

/// Show the welcome section.
pub(super) fn welcome_section_ui(ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        onboarding_content_ui(ui);
    });
}

fn bullet(ui: &mut Ui) {
    static DIAMETER: f32 = 6.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(DIAMETER, DIAMETER), egui::Sense::hover());

    ui.painter().add(egui::epaint::CircleShape {
        center: rect.center(),
        radius: DIAMETER / 2.0,
        fill: ui.visuals().strong_text_color(),
        stroke: egui::Stroke::NONE,
    });
}
fn onboarding_content_ui(ui: &mut Ui) {
    ui.add(egui::Label::new(
        egui::RichText::new("Visualize Multimodal Data")
            .strong()
            .line_height(Some(32.0))
            .text_style(re_ui::ReUi::welcome_screen_h1()),
    ));

    ui.add_space(29.0);

    let bullet_text = |ui: &mut Ui, text: &str| {
        ui.horizontal(|ui| {
            ui.add_space(1.0);
            bullet(ui);
            ui.add_space(5.0);
            ui.label(
                egui::RichText::new(text)
                    .color(ui.visuals().widgets.active.text_color())
                    .text_style(re_ui::ReUi::welcome_screen_body()),
            );
        });
        ui.add_space(4.0);
    };

    bullet_text(ui, "Log with the Rerun SDK in Python, C++, or Rust");
    bullet_text(ui, "Visualize and explore live or recorded data");
    bullet_text(ui, "Customize in the UI or through code");

    ui.add_space(9.0);
    if ui
        .button(
            egui::RichText::new("Go to documentation →")
                .color(egui::Color32::from_hex("#60A0FF").expect("this color is valid"))
                .text_style(re_ui::ReUi::welcome_screen_body()),
        )
        .clicked()
    {
        ui.ctx().output_mut(|o| {
            o.open_url = Some(egui::output::OpenUrl {
                url: DOCS_URL.to_owned(),
                new_tab: true,
            });
        });
    }
}
