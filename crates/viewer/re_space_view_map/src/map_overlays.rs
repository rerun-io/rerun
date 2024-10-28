use walkers::{sources::Attribution, MapMemory};

const MIN_MAP_SIZE_FOR_ZOOM_BUTTONS: f32 = 110.0;
const MIN_MAP_SIZE_FOR_ZOOM_BUTTONS_FADE: f32 = 150.0;

pub fn zoom_buttons_overlay(ui: &mut egui::Ui, map_rect: &egui::Rect, map_memory: &mut MapMemory) {
    let min_dim = map_rect.width().min(map_rect.height());

    if min_dim < MIN_MAP_SIZE_FOR_ZOOM_BUTTONS {
        return;
    }

    let opacity = egui::emath::inverse_lerp(
        MIN_MAP_SIZE_FOR_ZOOM_BUTTONS..=MIN_MAP_SIZE_FOR_ZOOM_BUTTONS_FADE,
        min_dim,
    )
    .expect("`const` input range that will always be valid")
    .clamp(0., 1.);

    let right_top = map_rect.right_top() + egui::vec2(-15.0, 15.0);
    let button_rect = egui::Rect::from_two_pos(right_top, right_top + egui::vec2(-50.0, 40.0));

    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(button_rect));
    ui.multiply_opacity(opacity);

    egui::Frame::none()
        .fill(ui.visuals().window_fill)
        .inner_margin(egui::Margin::same(5.0))
        .rounding(6.5)
        .show(&mut ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new("➕").heading()).clicked() {
                    let _ = map_memory.zoom_in();
                }

                if ui.button(egui::RichText::new("➖").heading()).clicked() {
                    let _ = map_memory.zoom_out();
                }
            });
        });
}

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

    egui::Frame::none()
        .fill(ui.visuals().window_fill)
        .inner_margin(egui::Margin::same(2.0))
        .show(&mut ui, |ui| {
            let text = egui::WidgetText::from(attribution.text).small();
            ui.hyperlink_to(text, attribution.url);
        });
}
