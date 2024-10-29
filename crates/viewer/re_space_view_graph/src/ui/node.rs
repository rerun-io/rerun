use re_viewer_context::{HoverHighlight, InteractionHighlight, SelectionHighlight};

use crate::types::NodeInstance;

pub fn draw_node(
    ui: &mut egui::Ui,
    instance: &NodeInstance<'_>,
    highlight: InteractionHighlight,
) -> egui::Response {
    let hcolor = match (
        highlight.hover,
        highlight.selection != SelectionHighlight::None,
    ) {
        (HoverHighlight::None, false) => None,
        (HoverHighlight::None, true) => Some(ui.style().visuals.selection.bg_fill),
        (HoverHighlight::Hovered, ..) => Some(ui.style().visuals.widgets.hovered.bg_fill),
    };

    let bg = match highlight.hover {
        HoverHighlight::None => ui.style().visuals.widgets.noninteractive.bg_fill,
        HoverHighlight::Hovered => ui.style().visuals.widgets.hovered.bg_fill,
    };

    if let (true, Some(label)) = (instance.show_labels, instance.label) {
        let text = egui::RichText::new(label.to_string());

        egui::Frame::default()
            .rounding(egui::Rounding::same(4.0))
            .stroke(egui::Stroke::new(1.0, ui.style().visuals.text_color()))
            .inner_margin(egui::Vec2::new(6.0, 4.0))
            .fill(bg)
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                if let Some(color) = instance.color {
                    ui.add(egui::Button::new(text.color(color)));
                } else {
                    ui.add(egui::Button::new(text));
                }
            })
            .response
    } else {
        egui::Frame::default()
            .show(ui, |ui| {
                let r = 4.0;
                ui.add(|ui: &mut egui::Ui| {
                    let (rect, response) = ui
                        .allocate_at_least(egui::Vec2::new(2.0 * r, 2.0 * r), egui::Sense::drag()); // Frame size
                    ui.painter().circle(
                        rect.center(),
                        // pos + egui::Vec2::new(r, r),
                        r,
                        instance.color.unwrap_or(ui.style().visuals.text_color()),
                        hcolor.map_or(egui::Stroke::NONE, |c| egui::Stroke::new(2.0, c)),
                    );
                    response
                })
            })
            .response
    }
    .on_hover_text(format!(
        "Node ID: `{}` in `{}`",
        instance.node_id.as_str(),
        instance.entity_path
    ))
}
