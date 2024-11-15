use re_chunk::LatestAtQuery;
use re_log_types::EntityPath;
use re_query::{clamped_zip_1x4, range_zip_1x4};
use re_space_view::{DataResultQuery, RangeResultsExt};
use re_types::components::{Color, Radius, ShowLabels};
use re_types::datatypes::Float32;
use re_types::{
    self, archetypes,
    components::{self},
    ArrowString, Loggable as _,
};
use re_viewer_context::{
    self, IdentifiedViewSystem, QueryContext, SpaceViewSystemExecutionError,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemIdentifier, VisualizerQueryInfo, VisualizerSystem,
};

use crate::graph::NodeIndex;

#[derive(Default)]
pub struct NodeVisualizer {
    pub data: ahash::HashMap<EntityPath, NodeData>,
}

pub struct NodeInstance {
    pub node: components::GraphNode,
    pub index: NodeIndex,
    pub label: Option<ArrowString>,
    pub color: Option<egui::Color32>,
    pub position: Option<egui::Pos2>,
    pub radius: Option<components::Radius>,
}

pub struct NodeData {
    pub nodes: Vec<NodeInstance>,
}

impl IdentifiedViewSystem for NodeVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphNodes".into()
    }
}

impl VisualizerSystem for NodeVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<archetypes::GraphNodes>()
    }

    /// Populates the scene part with data from the store.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = data_result
                .latest_at_with_blueprint_resolved_data::<archetypes::GraphNodes>(
                    ctx,
                    &timeline_query,
                );

            let all_indexed_nodes = results.iter_as(query.timeline, components::GraphNode::name());
            let all_colors = results.iter_as(query.timeline, components::Color::name());
            let all_positions = results.iter_as(query.timeline, components::Position2D::name());
            let all_labels = results.iter_as(query.timeline, components::Text::name());
            let all_radii = results.iter_as(query.timeline, components::Radius::name());
            let show_labels = results
                .get_mono::<components::ShowLabels>()
                .map_or(true, bool::from);

            let data = range_zip_1x4(
                all_indexed_nodes.component::<components::GraphNode>(),
                all_colors.component::<components::Color>(),
                all_positions.primitive_array::<2, f32>(),
                all_labels.string(),
                all_radii.component::<components::Radius>(),
            );

            for (_index, nodes, colors, positions, labels, radii) in data {
                let nodes = clamped_zip_1x4(
                    nodes.iter(),
                    colors.unwrap_or_default().iter().map(Option::Some),
                    Option::<&Color>::default,
                    positions
                        .unwrap_or_default()
                        .iter()
                        .copied()
                        .map(Option::Some),
                    Option::<[f32; 2]>::default,
                    labels
                        .unwrap_or_default()
                        .iter()
                        .map(|l| show_labels.then_some(l)),
                    Option::<&ArrowString>::default,
                    radii.unwrap_or_default().iter().map(Option::Some),
                    Option::<&components::Radius>::default,
                )
                .map(|(node, color, position, label, radius)| NodeInstance {
                    node: node.clone(),
                    index: NodeIndex::from_entity_node(&data_result.entity_path, node),
                    color: color.map(|&Color(color)| color.into()),
                    position: position.map(|[x, y]| egui::Pos2::new(x, y)),
                    label: if show_labels { label.cloned() } else { None },
                    radius: radius.copied(),
                })
                .collect();

                self.data
                    .insert(data_result.entity_path.clone(), NodeData { nodes });
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<ShowLabels> for NodeVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> ShowLabels {
        true.into()
    }
}

impl TypedComponentFallbackProvider<Radius> for NodeVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> Radius {
        Radius(Float32(4.0f32))
    }
}

re_viewer_context::impl_component_fallback_provider!(NodeVisualizer => [ShowLabels, Radius]);
