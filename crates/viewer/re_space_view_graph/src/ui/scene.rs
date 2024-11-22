use egui::{
    emath::TSTransform, Area, Color32, Id, LayerId, Order, Painter, Pos2, Rect, Response, Sense,
    Stroke, Ui, Vec2,
};
use re_chunk::EntityPath;
use re_types::{components::Radius, datatypes::Float32};
use re_viewer_context::{InteractionHighlight, SpaceViewHighlights};
use std::hash::Hash;

use crate::{
    graph::NodeInstanceImplicit,
    visualizers::{EdgeInstance, NodeInstance},
};

use super::draw::{draw_edge, draw_entity, draw_explicit, draw_implicit};

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

pub struct SceneBuilder {
    show_debug: bool,
    world_bounds: Rect,
    bounding_rect: Rect,
    children_drag_delta: Vec2,
    children_hovered: bool,
}

impl SceneBuilder {
    pub fn from_world_bounds(world_bounds: impl Into<Rect>) -> Self {
        Self {
            world_bounds: world_bounds.into(),
            show_debug: false,
            bounding_rect: Rect::NOTHING,
            children_drag_delta: Vec2::ZERO,
            children_hovered: false,
        }
    }

    pub fn show_debug(&mut self) {
        self.show_debug = true;
    }

    /// Return the clip rect of the scene in window coordinates.
    pub fn add<F>(mut self, ui: &mut Ui, add_scene_contents: F) -> (Rect, Response)
    where
        F: for<'b> FnOnce(Scene<'b>),
    {
        re_tracing::profile_function!();

        let (id, clip_rect_window) = ui.allocate_space(ui.available_size());
        let response = ui.interact(clip_rect_window, id, Sense::click_and_drag());

        let mut world_to_view = fit_to_world_rect(clip_rect_window, self.world_bounds);

        let view_to_window = TSTransform::from_translation(ui.min_rect().left_top().to_vec2());
        let world_to_window = view_to_window * world_to_view;
        let clip_rect_world = world_to_window.inverse() * clip_rect_window;

        let window_layer = ui.layer_id();

        add_scene_contents(Scene {
            ui,
            id,
            window_layer,
            context: SceneContext {
                clip_rect_world,
                world_to_window,
            },
            bounding_rect: &mut self.bounding_rect,
            children_drag_delta: &mut self.children_drag_delta,
            children_hovered: &mut self.children_hovered,
        });

        // :warning: Currently, `children_drag_delta` and `children_hovered` only report events from the entity rectangles.
        // TODO(grtlr): Would it makes sense to move the creation of the `Response` here and let `Canvas` only be a thin wrapper?
        //              That way we would avoid the duplication between those objects and we would have single-responsibility.

        world_to_view.translation += self.children_drag_delta;
        if response.dragged() {
            world_to_view.translation += response.drag_delta();
        }

        if let Some(pointer) = ui.ctx().input(|i| i.pointer.hover_pos()) {
            // Note: Catch zooming / panning either in the container, or on the entitys.
            if response.hovered() || self.children_hovered {
                let pointer_in_world = world_to_window.inverse() * pointer;
                let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
                let pan_delta = ui.ctx().input(|i| i.smooth_scroll_delta);

                // Zoom in on pointer, but only if we are not zoomed out too far.
                if zoom_delta < 1.0 || world_to_view.scaling < 1.0 {
                    world_to_view = world_to_view
                    * TSTransform::from_translation(pointer_in_world.to_vec2())
                    * TSTransform::from_scaling(zoom_delta)
                    * TSTransform::from_translation(-pointer_in_world.to_vec2());
                }

                // Pan:
                world_to_view = TSTransform::from_translation(pan_delta) * world_to_view;
            }
        }

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

pub struct SceneContext {
    clip_rect_world: Rect,
    world_to_window: TSTransform,
}

impl SceneContext {
    pub fn distance_to_world(&self, distance: f32) -> f32 {
        distance / self.world_to_window.scaling
    }

    /// If the radius is negative, we need to convert it from ui to world coordinates.
    pub fn radius_to_world(&self, radius: Radius) -> f32 {
        match radius {
            Radius(Float32(r)) if r.is_sign_positive() => r,
            Radius(Float32(r)) => self.distance_to_world(r.abs()),
        }
    }
}

pub struct Scene<'a> {
    ui: &'a mut Ui,
    id: Id,
    window_layer: LayerId,
    context: SceneContext,
    bounding_rect: &'a mut Rect,
    children_drag_delta: &'a mut Vec2,
    children_hovered: &'a mut bool,
}

impl<'a> Scene<'a> {
    /// Draws a regular node, i.e. an explicit node instance.
    pub fn explicit_node(&mut self, pos: Pos2, node: &NodeInstance, highlights: InteractionHighlight) -> Response {
        self.node_wrapper(node.index, pos, |ui, world_to_ui| {
            draw_explicit(ui, world_to_ui, node, highlights)
        })
    }

    pub fn implicit_node(&mut self, pos: Pos2, node: &NodeInstanceImplicit) -> Response {
        self.node_wrapper(node.index, pos, |ui, _| draw_implicit(ui, node))
    }

    /// `pos` is the top-left position of the node in world coordinates.
    fn node_wrapper<F>(&mut self, id: impl Hash, pos: Pos2, add_node_contents: F) -> Response
    where
        F: for<'b> FnOnce(&'b mut Ui, &'b SceneContext) -> Response,
    {
        let response = Area::new(self.id.with(id))
            .fixed_pos(pos)
            .order(Order::Foreground)
            .constrain(false)
            .show(self.ui.ctx(), |ui| {
                ui.set_clip_rect(self.context.clip_rect_world);
                add_node_contents(ui, &self.context)
            })
            .response;

        let id = response.layer_id;
        self.ui
            .ctx()
            .set_transform_layer(id, self.context.world_to_window);
        self.ui.ctx().set_sublayer(self.window_layer, id);

        *self.bounding_rect = self.bounding_rect.union(response.rect);

        response
    }

    pub fn entity(
        &mut self,
        entity: &EntityPath,
        rect: Rect,
        highlights: &SpaceViewHighlights,
    ) -> Response {
        let response = Area::new(self.id.with(entity))
            .fixed_pos(rect.min)
            .order(Order::Background)
            .constrain(false)
            .show(self.ui.ctx(), |ui| {
                ui.set_clip_rect(self.context.clip_rect_world);
                draw_entity(ui, rect, entity, highlights)
            })
            .inner;

        if response.dragged() {
            *self.children_drag_delta += response.drag_delta();
        }

        if response.hovered() {
            *self.children_hovered = true;
        }

        let id = response.layer_id;
        self.ui
            .ctx()
            .set_transform_layer(id, self.context.world_to_window);
        self.ui.ctx().set_sublayer(self.window_layer, id);

        response
    }

    pub fn edge(
        &mut self,
        from: Rect,
        to: Rect,
        edge: &EdgeInstance,
        show_arrow: bool,
    ) -> Response {
        let response = Area::new(self.id.with(((edge.source_index, edge.target_index),)))
            .order(Order::Middle)
            .constrain(false)
            .show(self.ui.ctx(), |ui| {
                ui.set_clip_rect(self.context.clip_rect_world);
                draw_edge(ui, from, to, show_arrow)
            })
            .response;

        let id = response.layer_id;
        self.ui
            .ctx()
            .set_transform_layer(id, self.context.world_to_window);
        self.ui.ctx().set_sublayer(self.window_layer, id);

        response
    }
}
