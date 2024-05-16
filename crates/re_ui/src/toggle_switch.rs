//! Adapted from `egui_demo_lib/src/demo/toggle_switch.rs`

fn toggle_switch_ui(ui: &mut egui::Ui, height: f32, on: &mut bool) -> egui::Response {
    let width = (height / 2. * 3.).ceil();
    let size = egui::vec2(width, height); // 12x7 in figma, but 12x8 looks _much_ better in epaint

    let (interact_rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());

    let visual_rect = egui::Align2::CENTER_CENTER.align_size_within_rect(size, interact_rect);

    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *on, ""));

    if ui.is_rect_visible(visual_rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let visuals = ui.style().interact(&response);
        let expanded_rect = visual_rect.expand(visuals.expansion);
        let fg_fill = visuals.bg_fill;
        let bg_fill = visuals.text_color();
        let rounding = 0.5 * expanded_rect.height();
        ui.painter()
            .rect(expanded_rect, rounding, bg_fill, egui::Stroke::NONE);
        let circle_x = egui::lerp(
            (expanded_rect.left() + rounding)..=(expanded_rect.right() - rounding),
            how_on,
        );

        let circle_center = egui::pos2(circle_x, expanded_rect.center().y);
        let circle_radius = 0.3 * expanded_rect.height();
        ui.painter()
            .circle(circle_center, circle_radius, fg_fill, egui::Stroke::NONE);
    }

    response
}

/// A wrapper that allows the more idiomatic usage pattern: `ui.add(toggle_switch(&mut my_bool))`
/// iOS-style toggle switch.
///
/// ## Example:
/// ``` ignore
/// ui.add(toggle_switch(8.0, &mut my_bool));
/// ```
#[allow(clippy::needless_pass_by_ref_mut)] // False positive, toggle_switch_ui needs &mut
pub fn toggle_switch(height: f32, on: &mut bool) -> impl egui::Widget + '_ {
    move |ui: &mut egui::Ui| toggle_switch_ui(ui, height, on)
}
