use egui::{Color32, Frame, Painter, Pos2, Rect, Response, Shape, Stroke, Ui, Vec2};

/// Draws an edge with an optional arrow mark at the target end.
pub fn draw_edge(ui: &mut Ui, source: Rect, target: Rect, show_arrow: bool) -> Response {
    let fg = ui.style().visuals.text_color();

    Frame::default()
        .show(ui, |ui| {
            let source_center = source.center();
            let target_center = target.center();

            // Calculate direction vector from source to target
            let direction = (target_center - source_center).normalized();

            // Find the border points on both rectangles
            let source_point = find_border_point(source, -direction); // Reverse direction for target
            let target_point = find_border_point(target, direction);

            let painter = ui.painter();

            painter.line_segment([source_point, target_point], Stroke::new(1.0, fg));

            // Conditionally draw an arrow at the target point
            if show_arrow {
                draw_arrow(painter, target_point, direction, fg);
            }
        })
        .response
}

/// Helper function to find the point where the line intersects the border of a rectangle
fn find_border_point(rect: Rect, direction: Vec2) -> Pos2 {
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;

    for i in 0..2 {
        let inv_d = 1.0 / direction[i];
        let mut t0 = (rect.min[i] - rect.center()[i]) * inv_d;
        let mut t1 = (rect.max[i] - rect.center()[i]) * inv_d;

        if inv_d < 0.0 {
            std::mem::swap(&mut t0, &mut t1);
        }

        t_min = t_min.max(t0);
        t_max = t_max.min(t1);
    }

    let t = t_max.min(t_min); // Pick the first intersection
    rect.center() + t * direction
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
