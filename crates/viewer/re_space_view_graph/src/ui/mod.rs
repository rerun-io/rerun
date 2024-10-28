use re_log_types::EntityPath;
use re_viewer_context::SpaceViewHighlights;

mod edge;
pub(crate) use edge::draw_edge;
mod node;
pub(crate) use node::draw_node;
mod state;
pub(crate) use state::GraphSpaceViewState;

pub(crate) mod scene;

use crate::types::UnknownNodeInstance;

pub fn draw_dummy(ui: &mut egui::Ui, instance: &UnknownNodeInstance<'_>) -> egui::Response {
    let text = egui::RichText::new(format!(
        "{} @ {}",
        instance.node_id.as_str(),
        instance.entity_path
    ))
    .color(ui.style().visuals.widgets.noninteractive.text_color());
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    ui.add(egui::Button::new(text))
}

pub fn draw_entity(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    entity_path: &EntityPath,
    highlights: &SpaceViewHighlights,
) -> egui::Response {
    let (rect, response) = ui.allocate_at_least(rect.size(), egui::Sense::hover());

    let padded = rect.expand(10.0);
    let tc = ui.ctx().style().visuals.text_color();
    ui.painter().rect(
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
        ui.painter().text(
            padded.left_top(),
            egui::Align2::LEFT_BOTTOM,
            entity_path.to_string(),
            egui::FontId::default(),
            ui.ctx().style().visuals.text_color(),
        );
    }

    response
}

pub fn bounding_rect_from_iter<'a>(
    rectangles: impl Iterator<Item = &'a egui::Rect>,
) -> egui::Rect {
    rectangles.fold(egui::Rect::NOTHING, |acc, rect| acc.union(*rect))
}
