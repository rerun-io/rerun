use std::collections::{BTreeSet, HashSet};

use ahash::HashMap;
use egui::{self, Vec2};

use fjadra::{Center, Link, ManyBody, PositionX, PositionY, SimulationBuilder};
use re_log::external::log;
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
    graph::NodeIndex,
    ui::{bounding_rect_from_iter, canvas::CanvasBuilder, GraphSpaceViewState},
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
        let state = state.downcast_ref::<GraphSpaceViewState>().ok()?;
        let layout = state.layout.as_ref()?;

        let (width, height) = state.world_bounds.map_or_else(
            || {
                let bbox = bounding_rect_from_iter(layout.1.values());
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

        Some(width / height)
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

        // We need to sort the entities to ensure that we are always drawing them in the right order.
        let entities = node_data
            .keys()
            .chain(edge_data.keys())
            .collect::<BTreeSet<_>>();

        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );

        let bounds: blueprint::components::VisualBounds2D =
            bounds_property.component_or_fallback(ctx, self, state)?;

        let layout_was_empty = state.layout.is_none();

        // For now, we reset the layout at every frame. Eventually, we want
        // to keep information between frames so that the nodes don't jump around.
        // let (layout_time, layout) = state
        //     .layout
        //     .insert(((query.timeline, query.latest_at), Default::default()));

        let layout = match state.layout {
            Some(ref mut layout)
                if (layout.0 .0, layout.0 .1) == (query.timeline, query.latest_at) =>
            {
                &mut layout.1
            }
            _ => {
                log::debug!("recomputing graph layout");

                let layout = state
                    .layout
                    .insert(((query.timeline, query.latest_at), Default::default()));

                let mut node_index: HashMap<NodeIndex, usize> = HashMap::default();
                let mut all_nodes = node_data
                    .values()
                    .flat_map(|data| data.nodes.iter().map(|n| n.index))
                    .enumerate()
                    .map(|(o, n)| {
                        node_index.insert(n, o);
                        n
                    })
                    .collect::<Vec<_>>();

                let mut all_edges: Vec<(usize, usize)> = Vec::new();
                for edge in edge_data.values().flat_map(|data| data.edges.iter()) {
                    let source = *node_index.entry(edge.source_index).or_insert_with(|| {
                        all_nodes.push(edge.source_index);
                        all_nodes.len() - 1
                    });

                    let target = *node_index.entry(edge.target_index).or_insert_with(|| {
                        all_nodes.push(edge.target_index);
                        all_nodes.len() - 1
                    });
                    all_edges.push((source, target));
                }

                let mut simulation = SimulationBuilder::default()
                    .build(all_nodes.iter().map(|_| Option::<[f64; 2]>::None))
                    .add_force(
                        "link",
                        Link::new(all_edges.into_iter()),
                    )
                    .add_force("charge", ManyBody::new())
                    .add_force("x", PositionX::new())
                    .add_force("y", PositionY::new());

                let positions = simulation.iter().last().expect("simulation should run");
                for (node, i) in node_index {
                    layout.1.entry(node).or_insert_with(|| {
                        let pos = positions[i];
                        let pos = egui::Pos2::new(pos[0] as f32, pos[1] as f32);
                        let size = egui::Vec2::ZERO;
                        egui::Rect::from_min_size(pos, size)
                    });
                }

                &mut layout.1
            }
        };

        state.world_bounds = Some(bounds);
        let bounds_rect: egui::Rect = bounds.into();

        let mut viewer = CanvasBuilder::from_world_bounds(bounds_rect);

        // TODO(grtlr): Is there a blueprint archetype for debug information?
        if state.show_debug {
            viewer.show_debug();
        }

        // We keep track of the nodes in the data to clean up the layout.
        // TODO(grtlr): once we settle on a design, it might make sense to create a
        // `Layout` struct that keeps track of the layout and the nodes that
        // get added and removed and cleans up automatically (guard pattern).
        let mut seen: HashSet<NodeIndex> = HashSet::new();

        let (new_world_bounds, response) = viewer.canvas(ui, |mut scene| {
            // We store the offset to draw entities next to each other.
            // This is a workaround and will probably be removed once we have auto-layout.
            let mut entity_offset = egui::Vec2::ZERO;

            for entity in entities {
                // We keep track of the size of the current entity.
                let mut entity_rect = egui::Rect::NOTHING;
                if let Some(data) = node_data.get(entity) {
                    for node in &data.nodes {
                        seen.insert(node.index);
                        let current = layout.entry(node.index).or_insert(scene.initial_rect(node));

                        let response = scene.explicit_node(current.min + entity_offset, node);

                        // TODO(grtlr): ⚠️ This is hacky:
                        // We need to undo the `entity_offset` otherwise the offset will increase each frame.
                        *current = response.rect.translate(-entity_offset);
                        entity_rect = entity_rect.union(response.rect);
                    }
                }

                if let Some(data) = edge_data.get(entity) {
                    // An implicit node is a node that is not explicitly specified in the `GraphNodes` archetype.
                    let implicit_nodes = data
                        .edges
                        .iter()
                        .flat_map(|e| e.nodes())
                        .filter(|n| !seen.contains(&NodeIndex::from_entity_node(entity, n)))
                        .collect::<Vec<_>>();

                    // TODO(grtlr): The following logic is quite hacky, because we have to place the implicit nodes somewhere.
                    // A lot of this logic will probably go away once we ship auto-layouts.
                    let mut current_implicit_offset =
                        Vec2::new(entity_rect.min.x, entity_rect.height() + 40.0);
                    for node in implicit_nodes {
                        let ix = NodeIndex::from_entity_node(entity, node);
                        seen.insert(ix);
                        let current = layout.entry(ix).or_insert(
                            egui::Rect::ZERO
                                .translate(entity_offset)
                                .translate(current_implicit_offset),
                        );
                        let response = scene.implicit_node(current.min, node);
                        *current = response.rect.translate(-entity_offset);
                        // entity_rect = entity_rect.union(response.rect);
                        current_implicit_offset.x += 10.0;
                    }

                    for edge in &data.edges {
                        if let (Some(source_pos), Some(target_pos)) = (
                            layout.get(&edge.source_index),
                            layout.get(&edge.target_index),
                        ) {
                            scene.edge(
                                source_pos.translate(entity_offset),
                                target_pos.translate(entity_offset),
                                edge,
                                data.graph_type == components::GraphType::Directed,
                            );
                        }
                    }
                }

                // if entity_rect.is_positive() {
                //     let response = scene.entity(entity, entity_rect, &query.highlights);

                //     let instance_path = InstancePath::entity_all(entity.clone());
                //     ctx.select_hovered_on_click(
                //         &response,
                //         vec![(Item::DataResult(query.space_view_id, instance_path), None)]
                //             .into_iter(),
                //     );

                //     // TODO(grtlr): Should take padding from `draw_entity` into account.
                //     // It's very likely that this part of the code is going to change once we introduce auto-layout.
                //     let between_entities = 80.0;
                //     // entity_offset.x += entity_rect.width() + between_entities;
                // }
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

        Ok(())
    }
}
