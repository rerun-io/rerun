use re_types::components::GraphNode;

pub fn draw_circle_node(
    ui: &mut egui::Ui,
    radius: f32,
    fill_color: egui::Color32,
    stroke: egui::Stroke,
) -> egui::Response {
    debug_assert!(radius > 0.0, "radius must be greater than zero");

    egui::Frame::default()
        .show(ui, |ui| {
            ui.add(|ui: &mut egui::Ui| {
                let (rect, response) = ui.allocate_at_least(
                    egui::Vec2::new(2.0 * radius, 2.0 * radius),
                    egui::Sense::drag(),
                ); // Frame size
                ui.painter()
                    .circle(rect.center(), radius, fill_color, stroke);
                response
            })
        })
        .response
}

pub fn draw_dummy(ui: &mut egui::Ui, node: &GraphNode) -> egui::Response {
    draw_circle_node(
        ui,
        4.0,
        ui.style().visuals.gray_out(ui.style().visuals.text_color()),
        egui::Stroke::NONE,
    )
    .on_hover_text(format!("Implicit Node: `{}`", node.as_str(),))
}

pub fn draw_node(
    ui: &mut egui::Ui,
    label: &str,
    fg_color: Option<egui::Color32>,
    bg_color: egui::Color32,
) -> egui::Response {
    let text = egui::RichText::new(label);

    egui::Frame::default()
        .rounding(egui::Rounding::same(4.0))
        .stroke(egui::Stroke::new(1.0, ui.style().visuals.text_color()))
        .inner_margin(egui::Vec2::new(6.0, 4.0))
        .fill(bg_color)
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            if let Some(color) = fg_color {
                ui.add(egui::Label::new(text.color(color)));
            } else {
                ui.add(egui::Label::new(text));
            }
        })
        .response
}
