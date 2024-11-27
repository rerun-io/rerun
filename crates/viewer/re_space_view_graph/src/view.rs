use egui::{emath::TSTransform, Color32, Layout, Pos2, Stroke};
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
    canvas::{draw_debug, draw_node, zoom_pan_area}, graph::Graph, layout::LayoutRequest, ui::{draw::DrawableLabel, Discriminator, GraphSpaceViewState}, visualizers::{merge, EdgesVisualizer, NodeVisualizer}
};

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
        let node_data = &system_output.view_systems.get::<NodeVisualizer>()?.data;
        let edge_data = &system_output.view_systems.get::<EdgesVisualizer>()?.data;

        let graphs = merge(node_data, edge_data)
            .map(|(ent, nodes, edges)| Graph::new(ui, ent.clone(), nodes, edges))
            .collect::<Vec<_>>();

        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );
        let bounds: blueprint::components::VisualBounds2D =
            bounds_property.component_or_fallback(ctx, self, state)?;

        let view_rect = ui.max_rect();

        let request = LayoutRequest::from_graphs(graphs.iter());

        // The descriminator is used to determine if the layout needs to be recomputed.
        let discriminator = {
            let mut hasher = ahash::AHasher::default();
            for graph in &graphs {
                graph.size_hash().hash(&mut hasher);
            }
            hasher.finish()
        };

        let layout = state.layout_state.get(discriminator, &graphs);

        let (resp, new_bounds) = zoom_pan_area(
            ui,
            view_rect,
            bounds.into(),
            egui::Id::new(query.space_view_id),
            |ui, world_to_view| {
                let mut world_bounding_rect = egui::Rect::NOTHING;

                for graph in graphs {
                    for node in graph.nodes() {
                        // TODO(grtlr): provide debug assertions here.
                        let center = layout.get_node(&node.id()).unwrap_or(egui::Rect::ZERO).center();

                        // TODO(grtlr): Add proper highlights here:
                        let resp = draw_node(
                            ui,
                            center,
                            world_to_view,
                            node.label(),
                            Default::default(),
                        );
                        world_bounding_rect = world_bounding_rect.union(resp.rect);
                    }
                }

                // We need to draw the debug information after the rest to ensure that we have the correct bounding box.
                if state.show_debug {
                    draw_debug(ui, world_bounding_rect);
                }
            },
        );

        // Update blueprint if changed
        let updated_bounds: blueprint::components::VisualBounds2D = new_bounds.into();
        if resp.double_clicked() {
            bounds_property.reset_blueprint_component::<blueprint::components::VisualBounds2D>(ctx);
        } else if bounds != updated_bounds {
            bounds_property.save_blueprint_component(ctx, &updated_bounds);
        }
        // Update stored bounds on the state, so visualizers see an up-to-date value.
        state.world_bounds = Some(bounds);

        if state.layout_state.is_in_progress() {
            ui.ctx().request_repaint();
        }

        Ok(())

        //

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

        //
        // Ok(())
    }
}
