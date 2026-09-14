use walkers::sources::Attribution;

pub fn acknowledgement_overlay(
    ui: &mut egui::Ui,
    map_rect: &egui::Rect,
    attribution: &Attribution,
) {
    const HEIGHT: f32 = 15.0;

    let rect = egui::Rect::from_min_size(
        map_rect.left_bottom() - egui::vec2(0.0, HEIGHT),
        egui::vec2(map_rect.width(), HEIGHT),
    );

    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    ui.multiply_opacity(0.7);

    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .inner_margin(egui::Margin::same(2))
        .show(&mut ui, |ui| {
            let text = egui::WidgetText::from(attribution.text).small();
            ui.hyperlink_to(text, attribution.url);
        });
}
