use re_chunk::LatestAtQuery;
use re_log_types::EntityPath;
use re_space_view::{DataResultQuery, RangeResultsExt};
use re_types::{
    self, archetypes,
    components::{self, GraphEdge, GraphNode},
    Component as _,
};
use re_viewer_context::{
    self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo, VisualizerSystem,
};

use crate::graph::NodeIndex;

#[derive(Default)]
pub struct EdgesVisualizer {
    pub data: ahash::HashMap<EntityPath, EdgeData>,
}

pub struct EdgeInstance {
    pub source: GraphNode,
    pub target: GraphNode,
    pub source_index: NodeIndex,
    pub target_index: NodeIndex,
}

impl std::hash::Hash for EdgeInstance {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // We use the more verbose destructring here, to make sure that we
        // exhaustively consider all fields when hashing (we get a compiler
        // warning when we forget a field).
        let Self {
            // The index fields already uniquely identify `source` and `target`.
            source: _,
            target: _,
            source_index,
            target_index,
        } = self;
        source_index.hash(state);
        target_index.hash(state);
    }
}

#[derive(Hash)]
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = data_result
                .latest_at_with_blueprint_resolved_data::<archetypes::GraphEdges>(
                    ctx,
                    &timeline_query,
                );

            let all_indexed_edges = results.iter_as(query.timeline, components::GraphEdge::name());
            let graph_type = results.get_mono_with_fallback::<components::GraphType>();

            for (_index, edges) in all_indexed_edges.component::<GraphEdge>() {
                let edges = edges
                    .iter()
                    .map(|edge| {
                        let source = GraphNode::from(edge.first.clone());
                        let target = GraphNode::from(edge.second.clone());

                        let entity_path = &data_result.entity_path;
                        let source_index = NodeIndex::from_entity_node(entity_path, &source);
                        let target_index = NodeIndex::from_entity_node(entity_path, &target);

                        EdgeInstance {
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
