use re_log_types::EntityPath;
use re_viewer_context::SpaceViewHighlights;

mod draw;
mod state;

pub mod scene;

pub use state::GraphSpaceViewState;

pub fn draw_entity(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    entity_path: &EntityPath,
    highlights: &SpaceViewHighlights,
) -> egui::Response {
    let (rect, response) = ui.allocate_at_least(rect.size(), egui::Sense::hover());

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

    ui.painter().rect(
        padded,
        0.0,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(1.0, color),
    );

    ui.painter().text(
        padded.left_top(),
        egui::Align2::LEFT_BOTTOM,
        entity_path.to_string(),
        egui::FontId {
            size: 12.0,
            family: Default::default(),
        },
        color,
    );
    // }

    response
}

pub fn bounding_rect_from_iter<'a>(rectangles: impl Iterator<Item = &'a egui::Rect>) -> egui::Rect {
    rectangles.fold(egui::Rect::NOTHING, |acc, rect| acc.union(*rect))
}
