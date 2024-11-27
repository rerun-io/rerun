use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, UiBuilder, Vec2};
use re_viewer_context::InteractionHighlight;

use crate::ui::draw::DrawableLabel;

// Sorry for the pun, could not resist 😎.
// On a serious note, is there no other way to create a `Sense` that does nothing?
const NON_SENSE: Sense = Sense {
    click: false,
    drag: false,
    focusable: false,
};

/// Draws a node at the given position.
pub fn draw_node(
    ui: &mut Ui,
    center: Pos2,
    node: &DrawableLabel,
    highlight: InteractionHighlight,
) -> Response {
    let builder = UiBuilder::new().max_rect(Rect::from_center_size(center, node.size()));
    let mut node_ui = ui.new_child(builder);
    node.draw(&mut node_ui, highlight)
}

/// Draws a bounding box, as well as a basic coordinate system.
pub fn draw_debug(ui: &Ui, world_bounding_rect: Rect) {
    let painter = ui.painter();

    // Paint coordinate system at the world origin
    let origin = Pos2::new(0.0, 0.0);
    let x_axis = Pos2::new(100.0, 0.0);
    let y_axis = Pos2::new(0.0, 100.0);

    painter.line_segment([origin, x_axis], Stroke::new(1.0, Color32::RED));
    painter.line_segment([origin, y_axis], Stroke::new(1.0, Color32::GREEN));

    if world_bounding_rect.is_positive() {
        painter.rect(
            world_bounding_rect,
            0.0,
            Color32::from_rgba_unmultiplied(255, 0, 255, 8),
            Stroke::new(1.0, Color32::from_rgb(255, 0, 255)),
        );
    }
}

/// Helper function to draw an arrow at the end of the edge
fn draw_arrow(painter: &Painter, tip: Pos2, direction: Vec2, color: Color32) {
    let arrow_size = 10.0; // Adjust size as needed
    let perpendicular = Vec2::new(-direction.y, direction.x) * 0.5 * arrow_size;

    let p1 = tip - direction * arrow_size + perpendicular;
    let p2 = tip - direction * arrow_size - perpendicular;

    // Draw a filled triangle for the arrow
    painter.add(Shape::convex_polygon(
        vec![tip, p1, p2],
        color,
        Stroke::NONE,
    ));
}

/// Draws an edge between two points, optionally with an arrow at the target point.
pub fn draw_edge(ui: &mut Ui, points: [Pos2; 2], show_arrow: bool) -> Response {
    let fg = ui.style().visuals.text_color();

    let rect = Rect::from_points(&points);
    let painter = ui.painter();
    painter.line_segment(points, Stroke::new(1.0, fg));

    // Calculate direction vector from source to target
    let direction = (points[1] - points[0]).normalized();

    // Conditionally draw an arrow at the target point
    if show_arrow {
        draw_arrow(painter, points[1], direction, fg);
    }

    // We can add interactions in the future, for now we simply allocate the
    // rect, so that bounding boxes are computed correctly.
    ui.allocate_rect(rect, NON_SENSE)
}
