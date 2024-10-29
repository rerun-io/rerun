use std::ops::RangeFrom;

use egui::{
    emath::TSTransform, Area, Color32, Id, LayerId, Order, Painter, Pos2, Rect, Response, Sense,
    Stroke, Ui,
};

fn fit_to_world_rect(clip_rect_window: Rect, world_rect: Rect) -> TSTransform {
    let available_size = clip_rect_window.size();

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

pub struct ViewBuilder {
    show_debug: bool,
    world_bounds: Rect,
    bounding_rect: Rect,
}

impl ViewBuilder {
    pub fn from_world_bounds(world_bounds: impl Into<Rect>) -> Self {
        Self {
            world_bounds: world_bounds.into(),
            show_debug: false,
            bounding_rect: Rect::NOTHING,
        }
    }

    pub fn show_debug(&mut self) {
        self.show_debug = true;
    }

    /// Return the clip rect of the scene in window coordinates.
    pub fn scene<F>(mut self, ui: &mut Ui, add_scene_contents: F) -> (Rect, Response)
    where
        F: for<'b> FnOnce(Scene<'b>),
    {
        re_tracing::profile_function!();

        let (id, clip_rect_window) = ui.allocate_space(ui.available_size());
        let response = ui.interact(clip_rect_window, id, Sense::click_and_drag());

        let mut world_to_view = fit_to_world_rect(clip_rect_window, self.world_bounds);

        if response.dragged() {
            world_to_view.translation += response.drag_delta();
        }

        let view_to_window = TSTransform::from_translation(ui.min_rect().left_top().to_vec2());

        let world_to_window = view_to_window * world_to_view;

        if let Some(pointer) = ui.ctx().input(|i| i.pointer.hover_pos()) {
            // Note: doesn't catch zooming / panning if a button in this PanZoom container is hovered.
            if response.hovered() {
                let pointer_in_world = world_to_window.inverse() * pointer;
                let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
                let pan_delta = ui.ctx().input(|i| i.smooth_scroll_delta);

                // Zoom in on pointer:
                world_to_view = world_to_view
                    * TSTransform::from_translation(pointer_in_world.to_vec2())
                    * TSTransform::from_scaling(zoom_delta)
                    * TSTransform::from_translation(-pointer_in_world.to_vec2());

                // Pan:
                world_to_view = TSTransform::from_translation(pan_delta) * world_to_view;
            }
        }

        let clip_rect_world = world_to_window.inverse() * clip_rect_window;

        let window_layer = ui.layer_id();

        add_scene_contents(Scene {
            ui,
            id,
            window_layer,
            clip_rect_world,
            world_to_window,
            counter: 0u64..,
            bounding_rect: &mut self.bounding_rect,
        });

        // We need to draw the debug information after the rest to ensure that we have the correct bounding box.
        if self.show_debug {
            let debug_id = LayerId::new(Order::Debug, id.with("debug_layer"));
            ui.ctx().set_transform_layer(debug_id, world_to_window);

            // Paint the coordinate system.
            let painter = Painter::new(ui.ctx().clone(), debug_id, clip_rect_world);

            // paint coordinate system at the world origin
            let origin = Pos2::new(0.0, 0.0);
            let x_axis = Pos2::new(100.0, 0.0);
            let y_axis = Pos2::new(0.0, 100.0);

            painter.line_segment([origin, x_axis], Stroke::new(1.0, Color32::RED));
            painter.line_segment([origin, y_axis], Stroke::new(2.0, Color32::GREEN));

            if self.bounding_rect.is_positive() {
                painter.rect(
                    self.bounding_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(255, 0, 255, 8),
                    Stroke::new(1.0, Color32::from_rgb(255, 0, 255)),
                );
            }
        }

        (
            (view_to_window * world_to_view).inverse() * clip_rect_window,
            response,
        )
    }
}

pub struct Scene<'a> {
    ui: &'a mut Ui,
    id: Id,
    window_layer: LayerId,
    clip_rect_world: Rect,
    world_to_window: TSTransform,
    counter: RangeFrom<u64>,
    bounding_rect: &'a mut Rect,
}

impl<'a> Scene<'a> {
    /// `pos` is the top-left position of the node in world coordinates.
    pub fn node<F>(&mut self, pos: Pos2, add_node_contents: F) -> Response
    where
        F: for<'b> FnOnce(&'b mut Ui) -> Response,
    {
        let response = Area::new(
            self.id.with((
                "__node",
                self.counter
                    .next()
                    .expect("The counter should never run out."),
            )),
        )
        .fixed_pos(pos)
        .order(Order::Foreground)
        .constrain(false)
        .show(self.ui.ctx(), |ui| {
            ui.set_clip_rect(self.clip_rect_world);
            add_node_contents(ui)
        })
        .response;

        let id = response.layer_id;
        self.ui.ctx().set_transform_layer(id, self.world_to_window);
        self.ui.ctx().set_sublayer(self.window_layer, id);

        *self.bounding_rect = self.bounding_rect.union(response.rect);

        response
    }

    pub fn entity<F>(&mut self, pos: Pos2, add_entity_contents: F) -> Response
    where
        F: for<'b> FnOnce(&'b mut Ui) -> Response,
    {
        let response = Area::new(
            self.id.with((
                "__entity",
                self.counter
                    .next()
                    .expect("The counter should never run out."),
            )),
        )
        .fixed_pos(pos)
        .order(Order::Background)
        .constrain(false)
        .show(self.ui.ctx(), |ui| {
            ui.set_clip_rect(self.clip_rect_world);
            add_entity_contents(ui)
        })
        .response;

        let id = response.layer_id;
        self.ui.ctx().set_transform_layer(id, self.world_to_window);
        self.ui.ctx().set_sublayer(self.window_layer, id);

        response
    }

    pub fn edge<F>(&mut self, add_edge_contents: F) -> Response
    where
        F: for<'b> FnOnce(&'b mut Ui) -> Response,
    {
        let response = Area::new(
            self.id.with((
                "edge",
                self.counter
                    .next()
                    .expect("The counter should never run out."),
            )),
        )
        .order(Order::Middle)
        .constrain(false)
        .show(self.ui.ctx(), |ui| {
            ui.set_clip_rect(self.clip_rect_world);
            add_edge_contents(ui)
        })
        .response;

        let id = response.layer_id;
        self.ui.ctx().set_transform_layer(id, self.world_to_window);
        self.ui.ctx().set_sublayer(self.window_layer, id);

        response
    }
}
