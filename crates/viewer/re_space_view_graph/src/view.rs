use std::collections::HashSet;

use egui::{self, Rect};

use re_log::ResultExt as _;
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
    graph::{Graph, NodeIndex},
    ui::{self, bounding_rect_from_iter, scene::ViewBuilder, GraphSpaceViewState},
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

    fn preferred_tile_aspect_ratio(&self, state: &dyn SpaceViewState) -> Option<f32> {
        state
            .downcast_ref::<GraphSpaceViewState>()
            .ok()
            .map(|state| {
                let (width, height) = state.world_bounds.map_or_else(
                    || {
                        let bbox = bounding_rect_from_iter(state.layout.values());
                        (
                            (bbox.max.x - bbox.min.x).abs(),
                            (bbox.max.y - bbox.min.y).abs(),
                        )
                    },
                    |bounds| {
                        (
                            bounds.x_range.abs_len() as f32,
                            bounds.y_range.abs_len() as f32,
                        )
                    },
                );

                width / height
            })
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

        let graph = Graph::from_nodes_edges(node_data, edge_data);

        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        // We keep track of the nodes in the data to clean up the layout.
        // TODO(grtlr): once we settle on a design, it might make sense to create a
        // `Layout` struct that keeps track of the layout and the nodes that
        // get added and removed and cleans up automatically (guard pattern).
        let mut seen: HashSet<NodeIndex> = HashSet::new();

        let layout_was_empty = state.layout.is_empty();

        let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );

        let bounds: blueprint::components::VisualBounds2D =
            bounds_property.component_or_fallback(ctx, self, state)?;

        state.world_bounds = Some(bounds);
        let bounds_rect: egui::Rect = bounds.into();

        let mut viewer = ViewBuilder::from_world_bounds(bounds_rect);

        // TODO(grtlr): Is there a blueprint archetype for debug information?
        if state.show_debug {
            viewer.show_debug();
        }

        let (new_world_bounds, response) = viewer.scene(ui, |mut scene| {
            for data in node_data {
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

            for data in edge_data {
                let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());

                for edge in data.edges() {
                    if let (Some(source_pos), Some(target_pos)) = (
                        state.layout.get(&edge.source_index()),
                        state.layout.get(&edge.target_index()),
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

        // Update blueprint if changed
        let updated_bounds: blueprint::components::VisualBounds2D = new_world_bounds.into();
        if response.double_clicked() || layout_was_empty {
            bounds_property.reset_blueprint_component::<blueprint::components::VisualBounds2D>(ctx);
        } else if bounds != updated_bounds {
            bounds_property.save_blueprint_component(ctx, &updated_bounds);
        }
        // Update stored bounds on the state, so visualizers see an up-to-date value.
        state.world_bounds = Some(bounds);

        // Clean up the layout for nodes that are no longer present.
        state.layout.retain(|k, _| seen.contains(k));

        Ok(())
    }
}
