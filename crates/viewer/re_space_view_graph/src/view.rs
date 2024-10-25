use std::collections::HashSet;

use egui::{self, Rect};

use re_log_types::EntityPath;
use re_types::{components, SpaceViewClassIdentifier};
use re_ui::{self, UiExt};
use re_viewer_context::{
    external::re_entity_db::InstancePath, IdentifiedViewSystem as _, Item, SpaceViewClass,
    SpaceViewClassLayoutPriority, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt as _,
    SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput, ViewQuery,
    ViewerContext,
};

use crate::{
    graph::{Graph, NodeIndex},
    ui::{self, GraphSpaceViewState},
    visualizers::{EdgesVisualizer, NodeVisualizer},
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

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        // Prefer a square tile if possible.
        Some(1.0)
    }

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
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        ui.selection_grid("graph_settings_ui").show(ui, |ui| {
            state.bounding_box_ui(ui);
            state.debug_ui(ui);
            state.simulation_ui(ui);
        });

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
        let node_system = system_output.view_systems.get::<NodeVisualizer>()?;
        let edge_system = system_output.view_systems.get::<EdgesVisualizer>()?;

        let graph = Graph::from_nodes_edges(&node_system.data, &edge_system.data);

        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        // We keep track of the nodes in the data to clean up the layout.
        // TODO(grtlr): once we settle on a design, it might make sense to create a
        // `Layout` struct that keeps track of the layout and the nodes that
        // get added and removed and cleans up automatically (guard pattern).
        let mut seen: HashSet<NodeIndex> = HashSet::new();

        let layout_was_empty = state.layout.is_empty();

        state.viewer.scene(ui, |mut scene| {
            for data in &node_system.data {
                let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());

                // We keep track of the size of the current entity.
                let mut entity_rect: Option<egui::Rect> = None;

                for node in data.nodes() {
                    let ix = NodeIndex::from(&node);
                    seen.insert(ix);
                    let current = state.layout.entry(ix).or_insert(
                        node.position.map_or(egui::Rect::ZERO, |p| {
                            Rect::from_center_size(p.into(), egui::Vec2::ZERO)
                        }),
                    );

                    let response = scene.node(current.min, |ui| {
                        ui::draw_node(ui, &node, ent_highlight.index_highlight(node.instance))
                    });

                    let instance = InstancePath::instance(data.entity_path.clone(), node.instance);
                    ctx.select_hovered_on_click(
                        &response,
                        Item::DataResult(query.space_view_id, instance),
                    );

                    *current = response.rect;
                    entity_rect =
                        entity_rect.map_or(Some(response.rect), |e| Some(e.union(response.rect)));
                }

                // TODO(grtlr): handle interactions
                let _response = entity_rect.map(|rect| {
                    scene.entity(rect.min, |ui| {
                        ui::draw_entity(ui, rect, &data.entity_path, &query.highlights)
                    })
                });
            }

            for dummy in graph.unknown_nodes() {
                let ix = NodeIndex::from(&dummy);
                seen.insert(ix);
                let current = state.layout.entry(ix).or_insert(Rect::ZERO);
                let response = scene.node(current.min, |ui| ui::draw_dummy(ui, &dummy));
                *current = response.rect;
            }

            for data in &edge_system.data {
                let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());

                for edge in data.edges() {
                    if let (Some(source_pos), Some(target_pos)) = (
                        state.layout.get(&edge.source_ix()),
                        state.layout.get(&edge.target_ix()),
                    ) {
                        scene.edge(|ui| {
                            ui::draw_edge(
                                ui,
                                None, // TODO(grtlr): change this back once we have edge colors
                                source_pos,
                                target_pos,
                                ent_highlight.index_highlight(edge.instance),
                                edge.edge_type == components::GraphType::Directed,
                            )
                        });
                    }
                }
            }
        });

        // Clean up the layout for nodes that are no longer present.
        state.layout.retain(|k, _| seen.contains(k));

        if layout_was_empty {
            state.viewer.fit_to_screen();
        }

        Ok(())
    }
}
