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
        let fg_fill_off = visuals.bg_fill;
        let fg_fill_on = egui::Color32::from_rgba_premultiplied(0, 128, 255, 255);
        let fg_fill = egui::Color32::from_rgba_premultiplied(
            lerp_u8(fg_fill_off.r(), fg_fill_on.r(), how_on),
            lerp_u8(fg_fill_off.g(), fg_fill_on.g(), how_on),
            lerp_u8(fg_fill_off.b(), fg_fill_on.b(), how_on),
            lerp_u8(fg_fill_off.a(), fg_fill_on.a(), how_on),
        );
        let bg_fill_off = visuals.text_color();

        let rounding = 0.5 * expanded_rect.height();
        ui.painter()
            .rect_filled(expanded_rect, rounding, bg_fill_off);
        let circle_x = egui::lerp(
            (expanded_rect.left() + rounding)..=(expanded_rect.right() - rounding),
            how_on,
        );

        let circle_center = egui::pos2(circle_x, expanded_rect.center().y);
        let circle_radius = 0.3 * expanded_rect.height();
        ui.painter()
            .circle_filled(circle_center, circle_radius, fg_fill);
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

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + t * (b as f32 - a as f32)).round() as u8
}
