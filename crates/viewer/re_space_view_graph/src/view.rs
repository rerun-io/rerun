use re_log_types::EntityPath;
use re_space_view::view_property_ui;
use re_types::{
    blueprint::{self, archetypes::VisualBounds2D},
    components, SpaceViewClassIdentifier,
};
use re_ui::{self, UiExt as _};
use re_viewer_context::{
    external::re_entity_db::InstancePath, IdentifiedViewSystem as _, Item, SpaceViewClass,
    SpaceViewClassLayoutPriority, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt as _,
    SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput, ViewQuery,
    ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use crate::{
    graph::Graph,
    ui::{canvas::CanvasBuilder, GraphSpaceViewState},
    visualizers::{merge, EdgesVisualizer, NodeVisualizer},
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

    fn help_markdown(&self, _egui_ctx: &egui::Context) -> String {
        "A space view that shows a graph as a node link diagram.".to_owned()
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

        if let Some(rect) = state.layout.bounding_rect() {
            let width = rect.width().abs();
            let height = rect.height().abs();
            return Some(width / height);
        }
        None
    }

    // TODO(grtlr): implement `recommended_root_for_entities`

    fn layout_priority(&self) -> SpaceViewClassLayoutPriority {
        Default::default()
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics {
        // By default spawn a single view at the root if there's anything the visualizer is applicable to.
        if ctx
            .applicable_entities_per_visualizer
            .get(&NodeVisualizer::identifier())
            .map_or(true, |entities| entities.is_empty())
        {
            SpaceViewSpawnHeuristics::default()
        } else {
            SpaceViewSpawnHeuristics::root()
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
            .map(|(ent, nodes, edges)| (ent, Graph::new(nodes, edges)))
            .collect::<Vec<_>>();

        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );

        let bounds: blueprint::components::VisualBounds2D =
            bounds_property.component_or_fallback(ctx, self, state)?;

        let layout_was_empty = state.layout.is_none();
        let layout = state.layout.get(
            query.timeline,
            query.latest_at,
            graphs.iter().map(|(_, graph)| graph),
        );

        state.world_bounds = Some(bounds);
        let bounds_rect: egui::Rect = bounds.into();

        let mut viewer = CanvasBuilder::from_world_bounds(bounds_rect);

        // TODO(grtlr): Is there a blueprint archetype for debug information?
        if state.show_debug {
            viewer.show_debug();
        }

        let (new_world_bounds, response) = viewer.canvas(ui, |mut scene| {
            for (entity, graph) in &graphs {
                // We use the following to keep track of the bounding box over nodes in an entity.
                let mut entity_rect = egui::Rect::NOTHING;

                // Draw explicit nodes.
                for node in graph.nodes_explicit() {
                    let pos = layout.get(&node.index).unwrap_or(egui::Rect::ZERO); // TODO(grtlr): sometimes there just isn't any data.
                                                                                   // .expect("explicit node should be in layout");
                    let response = scene.explicit_node(pos.min, node);
                    entity_rect = entity_rect.union(response.rect);
                    layout.update(&node.index, response.rect);
                }

                // Draw implicit nodes.
                for node in graph.nodes_implicit() {
                    let current = layout.get(&node.index).unwrap_or(egui::Rect::ZERO); // TODO(grtlr): sometimes there just isn't any data.
                                                                                       // .expect("implicit node should be in layout");
                    let response = scene.implicit_node(current.min, node);
                    entity_rect = entity_rect.union(response.rect);
                    layout.update(&node.index, response.rect);
                }

                // Draw edges.
                for edge in graph.edges() {
                    if let (Some(from), Some(to)) = (
                        layout.get(&edge.source_index),
                        layout.get(&edge.target_index),
                    ) {
                        let show_arrow = graph.kind() == components::GraphType::Directed;
                        scene.edge(from, to, edge, show_arrow);
                    }
                }

                // Draw entity rect.
                if entity_rect.is_positive() {
                    let response = scene.entity(entity, entity_rect, &query.highlights);

                    let instance_path = InstancePath::entity_all((*entity).clone());
                    ctx.select_hovered_on_click(
                        &response,
                        vec![(Item::DataResult(query.space_view_id, instance_path), None)]
                            .into_iter(),
                    );
                }
            }
        });

        // Update blueprint if changed
        let updated_bounds: blueprint::components::VisualBounds2D = new_world_bounds.into();
        if response.double_clicked() || layout_was_empty {
            bounds_property.reset_blueprint_component::<blueprint::components::VisualBounds2D>(ctx);
        } else if bounds != updated_bounds {
            bounds_property.save_blueprint_component(ctx, &updated_bounds);
        }
        // Update stored bounds on the state, so visualizers see an up-to-date value.
        state.world_bounds = Some(bounds);

        if state.layout.is_in_progress() {
            ui.ctx().request_repaint();
        }

        Ok(())
    }
}
