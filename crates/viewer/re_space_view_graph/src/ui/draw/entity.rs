use egui::{Align2, Color32, FontId, Rect, Response, Sense, Stroke, Ui};

use re_log_types::EntityPath;
use re_viewer_context::SpaceViewHighlights;

pub fn draw_entity(
    ui: &mut Ui,
    rect: Rect,
    entity_path: &EntityPath,
    highlights: &SpaceViewHighlights,
) -> Response {
    let (rect, response) = ui.allocate_at_least(rect.size(), Sense::drag());

    let color = if highlights
        .entity_outline_mask(entity_path.hash())
        .overall
        .is_some()
    {
        ui.ctx().style().visuals.text_color()
    } else {
        ui.ctx()
            .style()
            .visuals
            .gray_out(ui.ctx().style().visuals.text_color())
    };

    let padded = rect.expand(10.0);

    ui.painter()
        .rect(padded, 0.0, Color32::TRANSPARENT, Stroke::new(1.0, color));

    ui.painter().text(
        padded.left_top(),
        Align2::LEFT_BOTTOM,
        entity_path.to_string(),
        FontId {
            size: 12.0,
            family: Default::default(),
        },
        color,
    );

    response
}
