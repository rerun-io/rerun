use egui::{emath::TSTransform, Area, Id, Order, Pos2, Rect, Response, Ui, UiKind, Vec2};

/// Helper function to handle pan and zoom interactions on a response.
fn register_pan_and_zoom(ui: &Ui, resp: &Response, ui_from_world: &mut TSTransform) {
    if resp.dragged() {
        ui_from_world.translation += ui_from_world.scaling * resp.drag_delta();
    }

    if let Some(mouse_pos) = ui.input(|i| i.pointer.latest_pos()) {
        if resp.contains_pointer() {
            let pointer_in_world = ui_from_world.inverse() * mouse_pos;
            let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
            let pan_delta = ui.ctx().input(|i| i.smooth_scroll_delta);

            // Zoom in on pointer, but only if we are not zoomed out too far.
            if zoom_delta < 1.0 || ui_from_world.scaling < 1.0 {
                *ui_from_world = *ui_from_world
                    * TSTransform::from_translation(pointer_in_world.to_vec2())
                    * TSTransform::from_scaling(zoom_delta)
                    * TSTransform::from_translation(-pointer_in_world.to_vec2());
            }

            // Pan:
            *ui_from_world = TSTransform::from_translation(pan_delta) * *ui_from_world;
        }
    }
}

/// Creates a transformation that fits a given world rectangle into the available screen size.
pub fn fit_to_world_rect(available_size: Vec2, world_rect: Rect) -> TSTransform {
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

/// Provides a zoom-pan area for a given view.
pub fn zoom_pan_area(
    ui: &Ui,
    view_bounds_ui: Rect,
    world_bounds: Rect,
    // TODO(grtlr): Can we get rid of the `Id` here?
    id: Id,
    draw_contens: impl FnOnce(&mut Ui),
) -> (Response, Rect) {
    // ui space = global egui space
    // world-space = the space where we put graph nodes
    let mut ui_from_world = fit_to_world_rect(view_bounds_ui.size(), world_bounds);

    let area_resp = Area::new(id.with("zoom_pan_area"))
        .constrain_to(view_bounds_ui)
        .order(Order::Middle)
        .kind(UiKind::GenericArea)
        .show(ui.ctx(), |ui| {
            // Transform to the world space:
            let visible_rect_in_world = ui_from_world.inverse() * view_bounds_ui;

            ui.set_clip_rect(visible_rect_in_world); // set proper clip-rect so we can interact with the background. TODO: why is this needed?

            // A Ui for sensing drag-to-pan, scroll-to-zoom, etc
            let mut drag_sense_ui = ui.new_child(
                egui::UiBuilder::new()
                    .sense(egui::Sense::drag())
                    .max_rect(visible_rect_in_world),
            );
            drag_sense_ui.set_min_size(visible_rect_in_world.size());
            let pan_response = drag_sense_ui.response();

            // Update the transform based on the interactions:
            register_pan_and_zoom(ui, &pan_response, &mut ui_from_world);

            // Update the clip-rect with the new transform, to avoid frame-delays
            ui.set_clip_rect(ui_from_world.inverse() * view_bounds_ui);

            // Add the actul contents to the area:
            draw_contens(ui);

            pan_response
        });

    ui.ctx()
        .set_transform_layer(area_resp.response.layer_id, ui_from_world);

    let view_size = Rect::from_min_size(Pos2::ZERO, view_bounds_ui.size());
    (area_resp.inner, ui_from_world.inverse() * view_size)
}
