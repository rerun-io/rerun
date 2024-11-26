use egui::emath::TSTransform;
use re_log_types::EntityPath;
use re_space_view::{
    controls::{DRAG_PAN2D_BUTTON, ZOOM_SCROLL_MODIFIER},
    view_property_ui,
};
use re_types::{
    blueprint::{self, archetypes::VisualBounds2D},
    components, SpaceViewClassIdentifier,
};
use re_ui::{self, ModifiersMarkdown, MouseButtonMarkdown, UiExt as _};
use re_viewer_context::{
    external::re_entity_db::InstancePath, IdentifiedViewSystem as _, Item, RecommendedSpaceView,
    SpaceViewClass, SpaceViewClassLayoutPriority, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt as _,
    SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput, ViewQuery,
    ViewerContext,
};
use re_viewport_blueprint::ViewProperty;
use std::hash::{Hash as _, Hasher as _};

use crate::{
    graph::Graph,
    ui::{draw::DrawableNode, Discriminator, GraphSpaceViewState},
    visualizers::{merge, EdgesVisualizer, NodeVisualizer},
};

fn register_pan_and_zoom(
    ui: &egui::Ui,
    resp: egui::Response,
    transform: &mut TSTransform,
) -> egui::Response {
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

fn fit_to_world_rect(clip_rect_window: egui::Rect, world_rect: egui::Rect) -> TSTransform {
    let available_size = clip_rect_window.size();

    // Compute the scale factor to fit the bounding rectangle into the available screen size.
    let scale_x = available_size.x / world_rect.width();
    let scale_y = available_size.y / world_rect.height();

    // Use the smaller of the two scales to ensure the whole rectangle fits on the screen.
    let scale = scale_x.min(scale_y).min(1.0);

    // Compute the translation to center the bounding rect in the screen.
    let center_screen = egui::Pos2::new(available_size.x / 2.0, available_size.y / 2.0);
    let center_world = world_rect.center().to_vec2();

    // Set the transformation to scale and then translate to center.

    TSTransform::from_translation(center_screen.to_vec2() - center_world * scale)
        * TSTransform::from_scaling(scale)
}

#[derive(Default)]
pub struct GraphSpaceView;

impl SpaceViewClass for GraphSpaceView {
    // State type as described above.

    fn identifier() -> SpaceViewClassIdentifier {
        "Graph".into()
    }

    fn display_name(&self) -> &'static str {
        "Graph"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_GRAPH
    }

    fn help_markdown(&self, egui_ctx: &egui::Context) -> String {
        format!(
            r"# Graph View

Display a graph of nodes and edges.

## Navigation controls
- Pinch gesture or {zoom_scroll_modifier} + scroll to zoom.
- Click and drag with the {drag_pan2d_button} to pan.
- Double-click to reset the view.",
            zoom_scroll_modifier = ModifiersMarkdown(ZOOM_SCROLL_MODIFIER, egui_ctx),
            drag_pan2d_button = MouseButtonMarkdown(DRAG_PAN2D_BUTTON),
        )
    }

    /// Register all systems (contexts & parts) that the space view needs.
    fn on_register(
        &self,
        system_registry: &mut SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<NodeVisualizer>()?;
        system_registry.register_visualizer::<EdgesVisualizer>()
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<GraphSpaceViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, state: &dyn SpaceViewState) -> Option<f32> {
        let state = state.downcast_ref::<GraphSpaceViewState>().ok()?;

        if let Some(bounds) = state.world_bounds {
            let width = bounds.x_range.abs_len() as f32;
            let height = bounds.y_range.abs_len() as f32;
            return Some(width / height);
        }

        if let Some(rect) = state.layout_state.bounding_rect() {
            let width = rect.width().abs();
            let height = rect.height().abs();
            return Some(width / height);
        }
        None
    }

    fn layout_priority(&self) -> SpaceViewClassLayoutPriority {
        Default::default()
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics {
        if let Some(applicable) = ctx
            .applicable_entities_per_visualizer
            .get(&NodeVisualizer::identifier())
        {
            SpaceViewSpawnHeuristics::new(
                applicable
                    .iter()
                    .cloned()
                    .map(RecommendedSpaceView::new_single_entity),
            )
        } else {
            SpaceViewSpawnHeuristics::empty()
        }
    }

    /// Additional UI displayed when the space view is selected.
    ///
    /// In this sample we show a combo box to select the color coordinates mode.
    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        ui.selection_grid("graph_view_settings_ui").show(ui, |ui| {
            state.layout_ui(ui);
            state.simulation_ui(ui);
            state.debug_ui(ui);
        });

        view_property_ui::<VisualBounds2D>(ctx, ui, space_view_id, self, state);

        Ok(())
    }

    /// The contents of the Space View window and all interaction within it.
    ///
    /// This is called with freshly created & executed context & part systems.
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );
        let bounds: blueprint::components::VisualBounds2D =
            bounds_property.component_or_fallback(ctx, self, state)?;

        let world_bounds = bounds.into();

        let mut world_to_view = fit_to_world_rect(ui.max_rect(), world_bounds);

        //let view_rect = ui.max_rect();

        //
        // A: closure
        //

        // draggable_and_zoomable_area(
        //     ui,
        //     view_rect,
        //     &mut state.transform,
        //     |ui, apply_pan_and_zoom| {
        //         for node in nodes {
        //             let resp = node_ui(ui, node);
        //             apply_pan_and_zoom(resp);
        //         }
        //     },
        // );
        //
        // draggable_and_zoomable_area(ui, view_rect, &mut state.transform, |scene: Scene| {
        //     for node in nodes {
        //         scene.draw_something(|ui| {
        //             node_ui(ui, node) // must return a resposne
        //         });
        //     }
        // });

        let text = "hello world";

        let node = DrawableNode::text(ui, text, None, Default::default());
        let circle_node = DrawableNode::circle(ui, None, None);

        let view_rect = ui.max_rect();
        let clip_rect_world = world_to_view.inverse() * view_rect;

        let mut new_world_bounds = egui::Rect::NOTHING;

        let base_id = egui::Id::new(query.space_view_id);
        let inner_resp = egui::Area::new(base_id.with("view"))
            .constrain_to(view_rect)
            .order(egui::Order::Middle)
            .kind(egui::UiKind::GenericArea)
            .show(ui.ctx(), |ui| {
                // let resp =
                //     ui.interact(ui.max_rect(), base_id.with("sub_view"), egui::Sense::drag());

                //ui.allocate_space(view_rect.size());
                //ui.allocate_rect(view_rect, egui::Sense::hover());

                ui.set_clip_rect(clip_rect_world);

                //for node in nodes {}
                //for edge in edges {}

                let resp = {
                    let mut node_ui = ui.new_child(egui::UiBuilder::new().max_rect(
                        egui::Rect::from_center_size(egui::pos2(400., 400.), node.size()),
                    ));

                    node.draw(&mut node_ui)
                };
                new_world_bounds = new_world_bounds.union(resp.rect);
                register_pan_and_zoom(ui, resp, &mut world_to_view);

                let resp = {
                    let mut node_ui = ui.new_child(egui::UiBuilder::new().max_rect(
                        egui::Rect::from_center_size(egui::pos2(600., 400.), circle_node.size()),
                    ));

                    circle_node.draw(&mut node_ui)
                };
                new_world_bounds = new_world_bounds.union(resp.rect);
                register_pan_and_zoom(ui, resp, &mut world_to_view);

                // // We need to draw the debug information after the rest to ensure that we have the correct bounding box.
                // if state.show_debug {
                //     // Paint the coordinate system.
                //     let painter = egui::Painter::new(ui.ctx().clone(), "___graph_view_debug"), clip_rect_world);

                //     // paint coordinate system at the world origin
                //     let origin = Pos2::new(0.0, 0.0);
                //     let x_axis = Pos2::new(100.0, 0.0);
                //     let y_axis = Pos2::new(0.0, 100.0);

                //     painter.line_segment([origin, x_axis], Stroke::new(1.0, Color32::RED));
                //     painter.line_segment([origin, y_axis], Stroke::new(1.0, Color32::GREEN));

                //     if self.bounding_rect.is_positive() {
                //         painter.rect(
                //             self.bounding_rect,
                //             0.0,
                //             Color32::from_rgba_unmultiplied(255, 0, 255, 8),
                //             Stroke::new(1.0, Color32::from_rgb(255, 0, 255)),
                //         );
                //     }
                // }
            });

        let resp = ui.allocate_rect(view_rect, egui::Sense::drag());
        let resp = register_pan_and_zoom(ui, resp, &mut world_to_view);

        ui.ctx()
            .set_transform_layer(inner_resp.response.layer_id, world_to_view);

        // Update blueprint if changed
        let updated_bounds: blueprint::components::VisualBounds2D = new_world_bounds.into();
        if resp.double_clicked() {
            bounds_property.reset_blueprint_component::<blueprint::components::VisualBounds2D>(ctx);
        } else if bounds != updated_bounds {
            bounds_property.save_blueprint_component(ctx, &updated_bounds);
        }
        // Update stored bounds on the state, so visualizers see an up-to-date value.
        state.world_bounds = Some(bounds);

        return Ok(());

        // let node_data = &system_output.view_systems.get::<NodeVisualizer>()?.data;
        // let edge_data = &system_output.view_systems.get::<EdgesVisualizer>()?.data;
        //
        // let graphs = merge(node_data, edge_data)
        //     .map(|(ent, nodes, edges)| (ent, Graph::new(nodes, edges)))
        //     .collect::<Vec<_>>();
        //
        // // We could move this computation to the visualizers to improve
        // // performance if needed.
        // let discriminator = {
        //     let mut hasher = ahash::AHasher::default();
        //     graphs.hash(&mut hasher);
        //     Discriminator::new(hasher.finish())
        // };
        //
        // let state = state.downcast_mut::<GraphSpaceViewState>()?;
        //
        // let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(
        //     ctx.blueprint_db(),
        //     ctx.blueprint_query,
        //     query.space_view_id,
        // );
        //
        // let bounds: blueprint::components::VisualBounds2D =
        //     bounds_property.component_or_fallback(ctx, self, state)?;
        //
        // let layout_was_empty = state.layout_state.is_none();
        // let layout = state
        //     .layout_state
        //     .get(discriminator, graphs.iter().map(|(_, graph)| graph));
        //
        // let mut needs_remeasure = false;
        //
        // state.world_bounds = Some(bounds);
        // let bounds_rect: egui::Rect = bounds.into();
        //
        // let mut scene_builder = SceneBuilder::from_world_bounds(bounds_rect);
        //
        // if state.show_debug {
        //     scene_builder.show_debug();
        // }
        //
        // let (new_world_bounds, response) = scene_builder.add(ui, |mut scene| {
        //     for (entity, graph) in &graphs {
        //         // We use the following to keep track of the bounding box over nodes in an entity.
        //         let mut entity_rect = egui::Rect::NOTHING;
        //
        //         let ent_highlights = query.highlights.entity_highlight((*entity).hash());
        //
        //         // Draw explicit nodes.
        //         for node in graph.nodes_explicit() {
        //             let pos = layout.get(&node.index).unwrap_or(egui::Rect::ZERO);
        //
        //             let response = scene.explicit_node(
        //                 pos.min,
        //                 node,
        //                 ent_highlights.index_highlight(node.instance),
        //             );
        //
        //             if response.clicked() {
        //                 let instance_path =
        //                     InstancePath::instance((*entity).clone(), node.instance);
        //                 ctx.select_hovered_on_click(
        //                     &response,
        //                     vec![(Item::DataResult(query.space_view_id, instance_path), None)]
        //                         .into_iter(),
        //                 );
        //             }
        //
        //             entity_rect = entity_rect.union(response.rect);
        //             needs_remeasure |= layout.update(&node.index, response.rect);
        //         }
        //
        //         // Draw implicit nodes.
        //         for node in graph.nodes_implicit() {
        //             let current = layout.get(&node.index).unwrap_or(egui::Rect::ZERO);
        //             let response = scene.implicit_node(current.min, node);
        //             entity_rect = entity_rect.union(response.rect);
        //             needs_remeasure |= layout.update(&node.index, response.rect);
        //         }
        //
        //         // Draw edges.
        //         for edge in graph.edges() {
        //             if let (Some(from), Some(to)) = (
        //                 layout.get(&edge.source_index),
        //                 layout.get(&edge.target_index),
        //             ) {
        //                 let show_arrow = graph.kind() == components::GraphType::Directed;
        //                 scene.edge(from, to, edge, show_arrow);
        //             }
        //         }
        //
        //         // Draw entity rect.
        //         if graphs.len() > 1 && entity_rect.is_positive() {
        //             let response = scene.entity(entity, entity_rect, &query.highlights);
        //
        //             let instance_path = InstancePath::entity_all((*entity).clone());
        //             ctx.select_hovered_on_click(
        //                 &response,
        //                 vec![(Item::DataResult(query.space_view_id, instance_path), None)]
        //                     .into_iter(),
        //             );
        //         }
        //     }
        // });
        //

        //
        // if needs_remeasure {
        //     ui.ctx().request_discard("layout needed a remeasure");
        // }
        //
        // if state.layout_state.is_in_progress() {
        //     ui.ctx().request_repaint();
        // }
        //
        // Ok(())
    }
}
