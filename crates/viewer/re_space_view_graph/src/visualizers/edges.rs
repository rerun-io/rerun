use re_chunk::{ChunkComponentIterItem, LatestAtQuery};
use re_log_types::Instance;
use re_query::{clamped_zip_2x1, range_zip_1x1};
use re_space_view::{DataResultQuery, RangeResultsExt};
use re_types::{self, archetypes, components, Loggable as _};
use re_viewer_context::{
    self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo, VisualizerSystem,
};

use crate::types::EdgeInstance;

#[derive(Default)]
pub struct EdgesVisualizer {
    pub data: Vec<EdgeData>,
}

pub struct EdgeData {
    pub entity_path: re_log_types::EntityPath,
    pub graph_type: components::GraphType,
    edges: ChunkComponentIterItem<components::GraphEdge>,
}

impl EdgeData {
    pub fn edges(&self) -> impl Iterator<Item = EdgeInstance<'_>> {
        clamped_zip_2x1(
            self.edges.iter(),
            (0..).map(Instance::from),
            // A placeholder for components that we will add in the future.
            std::iter::repeat(None),
            Option::<()>::default,
        )
        .map(|(edge, instance, _placeholder)| EdgeInstance {
            source: edge.first.clone().into(),
            target: edge.second.clone().into(),
            entity_path: &self.entity_path,
            instance,
            edge_type: self.graph_type,
        })
    }
}

impl IdentifiedViewSystem for EdgesVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphEdgesDirected".into()
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
            let all_graph_type = results.iter_as(query.timeline, components::GraphType::name());

            let data = range_zip_1x1(
                all_indexed_edges.component::<components::GraphEdge>(),
                all_graph_type.component::<components::GraphType>(),
            );

            for (_index, edges, graph_type) in data {
                self.data.push(EdgeData {
                    entity_path: data_result.entity_path.clone(),
                    edges,
                    graph_type: graph_type
                        .unwrap_or_default()
                        .first()
                        .copied()
                        .unwrap_or_default(),
                });
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
