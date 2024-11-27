use std::sync::Arc;

use egui::{
    Color32, FontSelection, Frame, Galley, Pos2, Rect, Response, RichText, Sense, Stroke,
    TextWrapMode, Ui, Vec2, WidgetText,
};
use re_viewer_context::{InteractionHighlight, SelectionHighlight};

use crate::{graph::NodeInstanceImplicit, visualizers::NodeInstance};

pub enum DrawableNode {
    Circle(CircleNode),
    Text(TextNode),
}

pub struct TextNode {
    frame: Frame,
    galley: Arc<Galley>,
}

pub struct CircleNode {
    radius: f32,
    color: Color32,
}

impl DrawableNode {
    pub fn size(&self) -> Vec2 {
        match self {
            DrawableNode::Circle(CircleNode { radius, .. }) => Vec2::splat(radius * 2.0),
            DrawableNode::Text(TextNode { galley, frame }) => {
                frame.inner_margin.sum() + galley.size() + Vec2::splat(frame.stroke.width * 2.0)
            }
        }
    }

    pub fn circle(ui: &Ui, radius: Option<f32>, color: Option<Color32>) -> Self {
        Self::Circle(CircleNode {
            radius: radius.unwrap_or(4.0),
            color: color.unwrap_or_else(|| ui.style().visuals.text_color()),
        })
    }

    pub fn text(
        ui: &Ui,
        text: impl Into<String>,
        color: Option<Color32>,
        highlight: InteractionHighlight,
    ) -> Self {
        // TODO(grtlr): Handle selections.

        let galley = WidgetText::from(
            RichText::new(text).color(color.unwrap_or_else(|| ui.style().visuals.text_color())),
        )
        .into_galley(
            ui,
            Some(TextWrapMode::Extend),
            f32::INFINITY,
            FontSelection::Default,
        );

        let frame = Frame::default()
            .inner_margin(Vec2::new(6.0, 4.0))
            .fill(ui.style().visuals.widgets.noninteractive.bg_fill)
            .stroke(Stroke::new(1.0, ui.style().visuals.text_color()));

        Self::Text(TextNode { frame, galley })
    }

    pub fn draw(self, ui: &mut Ui) -> Response {
        let sense = Sense::drag();
        match self {
            Self::Circle(CircleNode { radius, color }) => {
                let (resp, painter) = ui.allocate_painter(Vec2::splat(radius * 2.0), sense);
                painter.circle(resp.rect.center(), radius, color, Stroke::NONE);
                resp
            }
            Self::Text(TextNode { galley, frame }) => {
                frame
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(galley)
                                .selectable(false)
                                .sense(sense),
                        )
                    })
                    .response
            }
        }
    }
}

// /// The `world_to_ui_scale` parameter is used to convert between world and ui coordinates.
// pub fn draw_explicit(
//     ui: &mut Ui,
//     ctx: &SceneContext,
//     node: &NodeInstance,
//     highlight: InteractionHighlight,
// ) -> Response {
//     let visuals = &ui.style().visuals;

//     let fg = node.color.unwrap_or_else(|| visuals.text_color());

//     let response = if let (Some(label), true) = (node.label.as_ref(), node.show_label) {
//         // Draw a text node.

//         let bg = visuals.widgets.noninteractive.bg_fill;
//         let stroke = match highlight.selection {
//             SelectionHighlight::Selection => visuals.selection.stroke,
//             _ => Stroke::new(1.0, visuals.text_color()),
//         };

//         let text = RichText::new(label.as_str()).color(fg);
//         let label = Label::new(text).wrap_mode(TextWrapMode::Extend);

//         Frame::default()
//             .rounding(4.0)
//             .stroke(stroke)
//             .inner_margin(Vec2::new(6.0, 4.0))
//             .fill(bg)
//             .show(ui, |ui| ui.add(label))
//             .response
//     } else {
//         // Draw a circle node.
//         let r = node.radius.map(|r| ctx.radius_to_world(r)).unwrap_or(4.0);
//         debug_assert!(r.is_sign_positive(), "radius must be greater than zero");

//         Frame::default()
//             .show(ui, |ui| {
//                 ui.add(|ui: &mut Ui| {
//                     let (rect, response) = ui.allocate_at_least(
//                         Vec2::splat(2.0 * r),
//                         Sense::hover(), // Change this to allow dragging.
//                     ); // Frame size
//                     ui.painter().circle(rect.center(), r, fg, Stroke::NONE);
//                     response
//                 })
//             })
//             .response
//     };

//     if let Some(label) = node.label.as_ref() {
//         response.on_hover_text(format!(
//             "Graph Node: {}\nLabel: {label}",
//             node.node.as_str(),
//         ))
//     } else {
//         response.on_hover_text(format!("Graph Node: {}", node.node.as_str(),))
//     }
// }

// /// Draws an implicit node instance (dummy node).
// pub fn draw_implicit(ui: &mut egui::Ui, node: &NodeInstanceImplicit) -> Response {
//     let fg = ui.style().visuals.gray_out(ui.style().visuals.text_color());
//     let r = 4.0;

//     Frame::default()
//         .show(ui, |ui| {
//             ui.add(|ui: &mut Ui| {
//                 let (rect, response) = ui.allocate_at_least(
//                     Vec2::splat(2.0 * r),
//                     Sense::hover(), // Change this to allow dragging.
//                 ); // Frame size
//                 ui.painter().circle(rect.center(), r, fg, Stroke::NONE);
//                 response
//             })
//         })
//         .response
//         .on_hover_text(format!("Implicit Node: `{}`", node.node.as_str(),))
// }
