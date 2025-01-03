//! A small, self-container pan-and-zoom area for [`egui`].
//!
//! Throughout this module, we use the following conventions or naming the different spaces:
//! * `ui`-space: The _global_ `egui` space.
//! * `view`-space: The space where the pan-and-zoom area is drawn.
//! * `scene`-space: The space where the actual content is drawn.

use egui::{emath::TSTransform, Rect, Response, Ui, UiBuilder, Vec2};

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

            // Most of the time we can return early. This is also important to
            // avoid `ui_from_scene` to change slightly due to floating point errors.
            if zoom_delta == 1.0 && pan_delta == Vec2::ZERO {
                return;
            }

            // Zoom in on pointer, but only if we are not zoomed out too far.
            if zoom_delta < 1.0 || ui_from_scene.scaling < 1.0 {
                *ui_from_scene = *ui_from_scene
                    * TSTransform::from_translation(pointer_in_scene.to_vec2())
                    * TSTransform::from_scaling(zoom_delta)
                    * TSTransform::from_translation(-pointer_in_scene.to_vec2());

                // We clamp the resulting scaling to avoid zooming out too far.
                ui_from_scene.scaling = ui_from_scene.scaling.min(1.0);
            }

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
///
/// Will fill the entire `max_rect` of the `parent_ui`.
pub fn zoom_pan_area(
    parent_ui: &mut Ui,
    to_global: &mut TSTransform,
    draw_contents: impl FnOnce(&mut Ui),
) -> Response {
    // Create a new egui paint layer, where we can draw our contents:
    let zoom_pan_layer_id = egui::LayerId::new(
        parent_ui.layer_id().order,
        parent_ui.id().with("zoom_pan_area"),
    );

    // Put the layer directly on-top of the main layer of the ui:
    parent_ui
        .ctx()
        .set_sublayer(parent_ui.layer_id(), zoom_pan_layer_id);

    let global_view_bounds = parent_ui.max_rect();

    let mut local_ui = parent_ui.new_child(
        UiBuilder::new()
            .layer_id(zoom_pan_layer_id)
            .max_rect(to_global.inverse() * global_view_bounds)
            .sense(egui::Sense::click_and_drag()),
    );
    local_ui.set_min_size(local_ui.max_rect().size()); // Allocate all available space

    // Set proper clip-rect so we can interact with the background:
    local_ui.set_clip_rect(local_ui.max_rect());

    let pan_response = local_ui.response();

    // Update the `to_global` transform based on use interaction:
    register_pan_and_zoom(&local_ui, &pan_response, to_global);

    // Update the clip-rect with the new transform, to avoid frame-delays
    local_ui.set_clip_rect(to_global.inverse() * global_view_bounds);

    // Add the actual contents to the area:
    draw_contents(&mut local_ui);

    // Tell egui to apply the transform on the layer:
    local_ui
        .ctx()
        .set_transform_layer(zoom_pan_layer_id, *to_global);

    pan_response
}
