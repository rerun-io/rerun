use std::collections::HashMap;

use re_format::format_f32;
use re_log_types::EntityPath;
use re_viewer::external::{
    egui::{self, emath, TextWrapMode},
    re_ui::UiExt,
    re_viewer_context::{
        HoverHighlight, InteractionHighlight, SelectionHighlight, SpaceViewHighlights,
        SpaceViewState,
    },
};

mod edge;
pub(crate) use edge::draw_edge;

use crate::{graph::Node, types::{NodeIndex, NodeInstance, UnknownNodeInstance}};

pub fn draw_node(
    ui: &mut egui::Ui,
    instance: &NodeInstance,
    highlight: InteractionHighlight,
) -> egui::Response {
    let hcolor = match (
        highlight.hover,
        highlight.selection != SelectionHighlight::None,
    ) {
        (HoverHighlight::None, false) => ui.style().visuals.text_color(),
        (HoverHighlight::None, true) => ui.style().visuals.selection.bg_fill,
        (HoverHighlight::Hovered, ..) => ui.style().visuals.widgets.hovered.bg_fill,
    };

    let bg = match highlight.hover {
        HoverHighlight::None => ui.style().visuals.widgets.noninteractive.bg_fill,
        HoverHighlight::Hovered => ui.style().visuals.widgets.hovered.bg_fill,
    };
    // ui.style().visuals.faint_bg_color

    let text = instance
        .label
        .map_or(egui::RichText::new(instance.node_id.to_string()), |label| {
            egui::RichText::new(label.to_string())
        });

    egui::Frame::default()
        .rounding(egui::Rounding::same(4.0))
        .stroke(egui::Stroke::new(1.0, ui.style().visuals.text_color()))
        .inner_margin(egui::Vec2::new(6.0, 4.0))
        .fill(bg)
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
            if let Some(color) = instance.color {
                ui.add(egui::Button::new(text.color(color)));
            } else {
                ui.add(egui::Button::new(text));
            }
        })
        .response
}

pub fn draw_dummy(ui: &mut egui::Ui, instance: &UnknownNodeInstance) -> egui::Response {
    let text = egui::RichText::new(format!(
        "{} @ {}",
        instance.node_id,
        instance.entity_path.to_string()
    ))
    .color(ui.style().visuals.widgets.noninteractive.text_color());
    ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
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

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
#[derive(Default)]
pub(crate) struct GraphSpaceViewState {
    pub world_to_view: emath::TSTransform,

    // Debug information
    pub show_debug: bool,

    /// Positions of the nodes in world space.
    pub layout: Option<HashMap<NodeIndex, egui::Rect>>,
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

impl GraphSpaceViewState {
    pub fn fit_to_screen(&mut self, bounding_rect: egui::Rect, available_size: egui::Vec2) {
        // Compute the scale factor to fit the bounding rectangle into the available screen size.
        let scale_x = available_size.x / bounding_rect.width();
        let scale_y = available_size.y / bounding_rect.height();

        // Use the smaller of the two scales to ensure the whole rectangle fits on the screen.
        let scale = scale_x.min(scale_y).min(1.0);

        // Compute the translation to center the bounding rect in the screen.
        let center_screen = egui::Pos2::new(available_size.x / 2.0, available_size.y / 2.0);
        let center_world = bounding_rect.center().to_vec2();

        // Set the transformation to scale and then translate to center.
        self.world_to_view =
            emath::TSTransform::from_translation(center_screen.to_vec2() - center_world * scale)
                * emath::TSTransform::from_scaling(scale);
    }

    pub fn bounding_box_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(layout) = &self.layout {
            ui.grid_left_hand_label("Bounding box")
                .on_hover_text("The bounding box encompassing all Entities in the view right now");
            ui.vertical(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                if let Some(egui::Rect { min, max }) = bounding_rect_from_iter(layout.values()) {
                    ui.label(format!("x [{} - {}]", format_f32(min.x), format_f32(max.x),));
                    ui.label(format!("y [{} - {}]", format_f32(min.y), format_f32(max.y),));
                }
            });
            ui.end_row();
        }
    }

    pub fn debug_ui(&mut self, ui: &mut egui::Ui) {
        ui.re_checkbox(&mut self.show_debug, "Show debug information")
            .on_hover_text("Shows debug information for the current graph");
        ui.end_row();
    }
}

impl SpaceViewState for GraphSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
