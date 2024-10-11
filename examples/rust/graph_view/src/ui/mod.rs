use std::collections::HashMap;

use re_log_types::EntityPath;
use re_viewer::external::{
    egui,
    re_viewer_context::{InteractionHighlight, SpaceViewHighlights},
};

mod edge;
pub(crate) use edge::draw_edge;
mod node;
pub(crate) use node::draw_node;
mod state;
pub(crate) use state::GraphSpaceViewState;

use crate::{
    graph::Node,
    types::{NodeIndex, UnknownNodeInstance},
};

pub fn draw_dummy(ui: &mut egui::Ui, instance: &UnknownNodeInstance) -> egui::Response {
    let text = egui::RichText::new(format!("{} @ {}", instance.node_id, instance.entity_path))
        .color(ui.style().visuals.widgets.noninteractive.text_color());
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    ui.add(egui::Button::new(text))
}

pub fn draw_entity(
    ui: &mut egui::Ui,
    clip_rect: egui::Rect,
    layer_id: egui::LayerId,
    rect: egui::Rect,
    entity_path: &EntityPath,
    highlights: &SpaceViewHighlights,
) {
    let painter = egui::Painter::new(ui.ctx().clone(), layer_id, clip_rect);

    let padded = rect.expand(10.0);
    let tc = ui.ctx().style().visuals.text_color();
    painter.rect(
        padded,
        ui.style().visuals.window_rounding,
        egui::Color32::from_rgba_unmultiplied(tc.r(), tc.g(), tc.b(), 4),
        egui::Stroke::NONE,
    );

    if highlights
        .entity_outline_mask(entity_path.hash())
        .overall
        .is_some()
    {
        // TODO(grtlr): text should be presented in window space.
        painter.text(
            padded.left_top(),
            egui::Align2::LEFT_BOTTOM,
            entity_path.to_string(),
            egui::FontId::default(),
            ui.ctx().style().visuals.text_color(),
        );
    }
}

pub fn measure_node_sizes<'a>(
    ui: &mut egui::Ui,
    nodes: impl Iterator<Item = Node<'a>>,
) -> HashMap<NodeIndex, egui::Vec2> {
    let mut sizes = HashMap::new();
    let ctx = ui.ctx();
    ctx.request_discard("measuring node sizes");
    ui.horizontal(|ui| {
        for node in nodes {
            match node {
                Node::Regular(instance) => {
                    let r = draw_node(ui, &instance, InteractionHighlight::default());
                    sizes.insert((&instance).into(), r.rect.size());
                }
                Node::Unknown(instance) => {
                    let r = draw_dummy(ui, &instance);
                    sizes.insert((&instance).into(), r.rect.size());
                }
            };
        }
    });
    sizes
}

pub fn bounding_rect_from_iter<'a>(
    rectangles: impl Iterator<Item = &'a egui::Rect>,
) -> Option<egui::Rect> {
    // Start with `None` and gradually expand the bounding box.
    let mut bounding_rect: Option<egui::Rect> = None;

    for rect in rectangles {
        bounding_rect = match bounding_rect {
            Some(bounding) => Some(bounding.union(*rect)),
            None => Some(*rect),
        };
    }

    bounding_rect
}
