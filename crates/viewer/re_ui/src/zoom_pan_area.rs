//! A small, self-container pan-and-zoom area for [`egui`].
//!
//! Throughout this module, we use the following conventions or naming the different spaces:
//! * `ui`-space: The _global_ `egui` space.
//! * `view`-space: The space where the pan-and-zoom area is drawn.
//! * `scene`-space: The space where the actual content is drawn.

use egui::{emath::TSTransform, Area, Order, Rect, Response, Ui, UiKind};

/// Helper function to handle pan and zoom interactions on a response.
fn register_pan_and_zoom(ui: &Ui, resp: &Response, ui_from_scene: &mut TSTransform) {
    if resp.dragged() {
        ui_from_scene.translation += ui_from_scene.scaling * resp.drag_delta();
    }

    if let Some(mouse_pos) = ui.input(|i| i.pointer.latest_pos()) {
        if resp.contains_pointer() {
            let pointer_in_scene = ui_from_scene.inverse() * mouse_pos;
            let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
            let pan_delta = ui.ctx().input(|i| i.smooth_scroll_delta);

            // Zoom in on pointer, but only if we are not zoomed out too far.
            if zoom_delta < 1.0 || ui_from_scene.scaling < 1.0 {
                *ui_from_scene = *ui_from_scene
                    * TSTransform::from_translation(pointer_in_scene.to_vec2())
                    * TSTransform::from_scaling(zoom_delta)
                    * TSTransform::from_translation(-pointer_in_scene.to_vec2());
            }

            // We clamp the resulting scaling to avoid zooming out too far.
            ui_from_scene.scaling = ui_from_scene.scaling.min(1.0);

            // Pan:
            *ui_from_scene = TSTransform::from_translation(pan_delta) * *ui_from_scene;
        }
    }
}

/// Creates a transformation that fits a given scene rectangle into the available screen size.
pub fn fit_to_rect_in_scene(rect_in_ui: Rect, rect_in_scene: Rect) -> TSTransform {
    let available_size_in_ui = rect_in_ui.size();

    // Compute the scale factor to fit the bounding rectangle into the available screen size.
    let scale_x = available_size_in_ui.x / rect_in_scene.width();
    let scale_y = available_size_in_ui.y / rect_in_scene.height();

    // Use the smaller of the two scales to ensure the whole rectangle fits on the screen.
    let scale = scale_x.min(scale_y).min(1.0);

    // Compute the translation to center the bounding rect in the screen.
    let center_screen = rect_in_ui.center();
    let center_scene = rect_in_scene.center().to_vec2();

    // Set the transformation to scale and then translate to center.
    TSTransform::from_translation(center_screen.to_vec2() - center_scene * scale)
        * TSTransform::from_scaling(scale)
}

/// Provides a zoom-pan area for a given view.
pub fn zoom_pan_area(
    ui: &Ui,
    view_bounds_in_ui: Rect,
    ui_from_scene: &mut TSTransform,
    draw_contents: impl FnOnce(&mut Ui),
) -> Response {
    let area_resp = Area::new(ui.id().with("zoom_pan_area"))
        .constrain_to(view_bounds_in_ui)
        .order(Order::Middle)
        .kind(UiKind::GenericArea)
        .show(ui.ctx(), |ui| {
            // Transform to the scene space:
            let visible_rect_in_scene = ui_from_scene.inverse() * view_bounds_in_ui;

            // set proper clip-rect so we can interact with the background.
            ui.set_clip_rect(visible_rect_in_scene);

            // A Ui for sensing drag-to-pan, scroll-to-zoom, etc
            let mut drag_sense_ui = ui.new_child(
                egui::UiBuilder::new()
                    .sense(egui::Sense::click_and_drag())
                    .max_rect(visible_rect_in_scene),
            );
            drag_sense_ui.set_min_size(visible_rect_in_scene.size());
            let pan_response = drag_sense_ui.response();

            // Update the transform based on the interactions:
            register_pan_and_zoom(ui, &pan_response, ui_from_scene);

            // Update the clip-rect with the new transform, to avoid frame-delays
            ui.set_clip_rect(ui_from_scene.inverse() * view_bounds_in_ui);

            // Add the actual contents to the area:
            draw_contents(ui);

            pan_response
        });

    ui.ctx()
        .set_transform_layer(area_resp.response.layer_id, *ui_from_scene);

    area_resp.inner
}
