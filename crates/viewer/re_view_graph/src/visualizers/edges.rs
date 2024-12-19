use re_chunk::LatestAtQuery;
use re_log_types::{EntityPath, Instance};
use re_types::{
    self, archetypes,
    components::{self, GraphEdge, GraphNode},
    Component as _,
};
use re_view::{DataResultQuery, RangeResultsExt};
use re_viewer_context::{
    self, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, ViewSystemIdentifier, VisualizerQueryInfo, VisualizerSystem,
};

use crate::graph::NodeId;

#[derive(Default)]
pub struct EdgesVisualizer {
    pub data: ahash::HashMap<EntityPath, EdgeData>,
}

pub struct EdgeInstance {
    // We will need this in the future, when we want to select individual edges.
    pub _instance: Instance,
    pub source: GraphNode,
    pub target: GraphNode,
    pub source_index: NodeId,
    pub target_index: NodeId,
}

pub struct EdgeData {
    pub graph_type: components::GraphType,
    pub edges: Vec<EdgeInstance>,
}

impl IdentifiedViewSystem for EdgesVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphEdges".into()
    }
}

impl VisualizerSystem for EdgesVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<archetypes::GraphEdges>()
    }

    /// Populates the scene part with data from the store.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = data_result
                .latest_at_with_blueprint_resolved_data::<archetypes::GraphEdges>(
                    ctx,
                    &timeline_query,
                );

            let all_indexed_edges = results.iter_as(query.timeline, components::GraphEdge::name());
            let graph_type = results.get_mono_with_fallback::<components::GraphType>();

            // TODO(cmc): Provide a `iter_struct`.
            for (_index, edges) in all_indexed_edges.component_slow::<GraphEdge>() {
                let edges = edges
                    .iter()
                    .enumerate()
                    .map(|(i, edge)| {
                        let source = GraphNode::from(edge.first.clone());
                        let target = GraphNode::from(edge.second.clone());

                        let entity_path = &data_result.entity_path;
                        let source_index = NodeId::from_entity_node(entity_path, &source);
                        let target_index = NodeId::from_entity_node(entity_path, &target);

                        EdgeInstance {
                            _instance: Instance::from(i as u64),
                            source,
                            target,
                            source_index,
                            target_index,
                        }
                    })
                    .collect();

                self.data.insert(
                    data_result.entity_path.clone(),
                    EdgeData { edges, graph_type },
                );
            }
        }

        // We're not using `re_renderer` here, so return an empty vector.
        // If you want to draw additional primitives here, you can emit re_renderer draw data here directly.
        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

re_viewer_context::impl_component_fallback_provider!(EdgesVisualizer => []);
