use egui::emath::TSTransform;
use re_types::components::GraphNode;
use re_viewer_context::{HoverHighlight, InteractionHighlight, SelectionHighlight};

use crate::types::NodeInstance;

// TODO: Handle Scene/UI radius correctly!
fn draw_circle_node(
    ui: &mut egui::Ui,
    radius: f32,
    fill_color: egui::Color32,
    stroke: egui::Stroke,
) -> egui::Response {
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
    world_to_window: &TSTransform,
    instance: &NodeInstance,
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

    if let Some(label) = &instance.label {
        let text = egui::RichText::new(label.to_string());

        egui::Frame::default()
            .rounding(egui::Rounding::same(4.0))
            .stroke(egui::Stroke::new(1.0, ui.style().visuals.text_color()))
            .inner_margin(egui::Vec2::new(6.0, 4.0))
            .fill(bg)
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                if let Some(color) = instance.color {
                    ui.add(egui::Label::new(text.color(color)));
                } else {
                    ui.add(egui::Label::new(text));
                }
            })
            .response
    } else {
        let mut radius = instance.radius.map(|r| r.0.into()).unwrap_or(4.0);

        if radius < 0.0 {
            let view_radius = radius.abs();
            let world_radius = world_to_window
                .inverse()
                .mul_pos(egui::Pos2::new(view_radius, 0.0))
                .x;
            radius = world_radius;
        }

        draw_circle_node(
            ui,
            radius,
            instance.color.unwrap_or(ui.style().visuals.text_color()),
            hcolor.map_or(egui::Stroke::NONE, |c| egui::Stroke::new(2.0, c)),
        )
    }
    .on_hover_text(format!("Node: `{}`", instance.node.as_str(),))
}
