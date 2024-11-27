use egui::{
    emath::TSTransform, Area, Color32, Id, Order, Pos2, Rect, Response, Sense, Stroke, Ui, UiBuilder, UiKind, Vec2
};

use crate::ui::draw::DrawableNode;

fn register_pan_and_zoom(ui: &Ui, resp: Response, transform: &mut TSTransform) -> Response {
    if resp.dragged() {
        transform.translation += resp.drag_delta();
    }

    if let Some(mouse_pos) = resp.hover_pos() {
        let pointer_in_world = transform.inverse() * mouse_pos;
        let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
        let pan_delta = ui.ctx().input(|i| i.smooth_scroll_delta);

        // Zoom in on pointer, but only if we are not zoomed out too far.
        if zoom_delta < 1.0 || transform.scaling < 1.0 {
            *transform = *transform
                * TSTransform::from_translation(pointer_in_world.to_vec2())
                * TSTransform::from_scaling(zoom_delta)
                * TSTransform::from_translation(-pointer_in_world.to_vec2());
        }

        // Pan:
        *transform = TSTransform::from_translation(pan_delta) * *transform;
    }

    resp
}

fn fit_to_world_rect(available_size: Vec2, world_rect: Rect) -> TSTransform {
    // Compute the scale factor to fit the bounding rectangle into the available screen size.
    let scale_x = available_size.x / world_rect.width();
    let scale_y = available_size.y / world_rect.height();

    // Use the smaller of the two scales to ensure the whole rectangle fits on the screen.
    let scale = scale_x.min(scale_y).min(1.0);

    // Compute the translation to center the bounding rect in the screen.
    let center_screen = Pos2::new(available_size.x / 2.0, available_size.y / 2.0);
    let center_world = world_rect.center().to_vec2();

    // Set the transformation to scale and then translate to center.

    TSTransform::from_translation(center_screen.to_vec2() - center_world * scale)
        * TSTransform::from_scaling(scale)
}

pub fn draw_node(
    ui: &mut Ui,
    center: Pos2,
    world_to_view: &mut TSTransform,
    node: DrawableNode,
) -> Response {
    let resp = {
        let builder = UiBuilder::new().max_rect(Rect::from_center_size(center, node.size()));
        let mut node_ui = ui.new_child(builder);
        node.draw(&mut node_ui)
    };
    register_pan_and_zoom(ui, resp, world_to_view)
}

pub fn draw_debug(ui: &mut Ui, world_bounding_rect: Rect) {
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

pub fn zoom_pan_area(
    ui: &mut Ui,
    view_rect: Rect,
    world_bounds: Rect,
    id: Id,
    draw_contens: impl FnOnce(&mut Ui, &mut TSTransform),
) -> (Response, Rect) {
    let mut world_to_view = fit_to_world_rect(view_rect.size(), world_bounds);
    let clip_rect_world = world_to_view.inverse() * view_rect;

    let area_resp = Area::new(id.with("view"))
        .constrain_to(view_rect)
        .order(Order::Middle)
        .kind(UiKind::GenericArea)
        .show(ui.ctx(), |ui| {
            ui.set_clip_rect(clip_rect_world);

            draw_contens(ui, &mut world_to_view);
        });

    // TODO(grtlr): Do we even need an `Area`, or could we just spawn a new `child_ui`?
    let resp = ui.allocate_rect(view_rect, Sense::drag());
    let resp = register_pan_and_zoom(ui, resp, &mut world_to_view);

    ui.ctx()
        .set_transform_layer(area_resp.response.layer_id, world_to_view);

    let view_size = Rect::from_min_size(Pos2::ZERO, view_rect.size());
    (resp, world_to_view.inverse() * view_size)
}
