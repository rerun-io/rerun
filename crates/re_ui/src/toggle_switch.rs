//! Adapted from `egui_demo_lib/src/demo/toggle_switch.rs`

fn toggle_switch_ui(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let interactive_size = egui::vec2(12.0, ui.spacing().interact_size.y);
    let (interact_rect, mut response) =
        ui.allocate_exact_size(interactive_size, egui::Sense::click());
    let visual_size = egui::vec2(12.0, 8.0); // 12x7 in figma, but 12x8 looks _much_ better in epaint
    let visual_rect =
        egui::Align2::CENTER_CENTER.align_size_within_rect(visual_size, interact_rect);

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
        let circle_radius = 2.5 * expanded_rect.height() / visual_size.y;
        ui.painter()
            .circle(circle_center, circle_radius, fg_fill, egui::Stroke::NONE);
    }

    response
}

// A wrapper that allows the more idiomatic usage pattern: `ui.add(toggle_switch(&mut my_bool))`
/// iOS-style toggle switch.
///
/// ## Example:
/// ``` ignore
/// ui.add(toggle_switch(&mut my_bool));
/// ```
pub fn toggle_switch(on: &mut bool) -> impl egui::Widget + '_ {
    move |ui: &mut egui::Ui| toggle_switch_ui(ui, on)
}
