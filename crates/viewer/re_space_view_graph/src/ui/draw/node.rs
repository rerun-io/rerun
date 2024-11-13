use egui::{Frame, Label, Response, RichText, Sense, Stroke, TextWrapMode, Ui, Vec2};

use crate::{graph::NodeInstanceImplicit, ui::canvas::CanvasContext, visualizers::NodeInstance};

/// The `world_to_ui_scale` parameter is used to convert between world and ui coordinates.
pub fn draw_explicit(ui: &mut Ui, ctx: &CanvasContext, node: &NodeInstance) -> Response {
    let visuals = &ui.style().visuals;

    let fg = node.color.unwrap_or_else(|| visuals.text_color());

    if let Some(ref label) = node.label {
        // Draw a text node.

        let bg = visuals.widgets.noninteractive.bg_fill;
        let border = visuals.text_color();

        let text = RichText::new(label.as_str()).color(fg);
        let label = Label::new(text).wrap_mode(TextWrapMode::Extend);

        Frame::default()
            .rounding(4.0)
            .stroke(Stroke::new(1.0, border))
            .inner_margin(Vec2::new(6.0, 4.0))
            .fill(bg)
            .show(ui, |ui| ui.add(label))
            .response
    } else {
        // Draw a circle node.
        let r = node.radius.map(|r| ctx.radius_to_world(r)).unwrap_or(4.0);
        debug_assert!(r.is_sign_positive(), "radius must be greater than zero");

        Frame::default()
            .show(ui, |ui| {
                ui.add(|ui: &mut Ui| {
                    let (rect, response) = ui.allocate_at_least(
                        Vec2::splat(2.0 * r),
                        Sense::hover(), // Change this to allow dragging.
                    ); // Frame size
                    ui.painter().circle(rect.center(), r, fg, Stroke::NONE);
                    response
                })
            })
            .response
    }
    .on_hover_text(format!("Node: `{}`", node.node.as_str()))
}

/// Draws an implicit node instance (dummy node).
pub fn draw_implicit(ui: &mut egui::Ui, node: &NodeInstanceImplicit) -> Response {
    let fg = ui.style().visuals.gray_out(ui.style().visuals.text_color());
    let r = 4.0;

    Frame::default()
        .show(ui, |ui| {
            ui.add(|ui: &mut Ui| {
                let (rect, response) = ui.allocate_at_least(
                    Vec2::splat(2.0 * r),
                    Sense::hover(), // Change this to allow dragging.
                ); // Frame size
                ui.painter().circle(rect.center(), r, fg, Stroke::NONE);
                response
            })
        })
        .response
        .on_hover_text(format!("Implicit Node: `{}`", node.node.as_str(),))
}
