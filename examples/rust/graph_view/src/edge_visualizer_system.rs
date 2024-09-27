use re_log_types::Instance;
use re_viewer::external::{
    re_chunk::{ChunkComponentIterItem, LatestAtQuery},
    re_query::{clamped_zip_2x1, range_zip_1x1},
    re_renderer,
    re_space_view::{DataResultQuery, RangeResultsExt},
    re_types::{self, archetypes, components, Loggable as _},
    re_viewer_context::{
        self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext,
        ViewContextCollection, ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo,
        VisualizerSystem,
    },
};

use crate::common::{QualifiedEdge, QualifiedNode};

#[derive(Default)]
pub struct GraphEdgeVisualizer {
    pub(crate) data: Vec<GraphEdgeVisualizerData>,
}

pub(crate) struct GraphEdgeVisualizerData {
    pub entity_path: re_log_types::EntityPath,
    pub edges: ChunkComponentIterItem<components::GraphEdge>,
    pub colors: ChunkComponentIterItem<components::Color>,
}

impl GraphEdgeVisualizerData {
    pub(crate) fn edges(
        &self,
    ) -> impl Iterator<Item = (QualifiedEdge, Instance, Option<&components::Color>)> {
        clamped_zip_2x1(
            // TODO(grtlr): Avoid all this cloning!
            self.edges.iter().map(|e| QualifiedEdge {
                source: QualifiedNode {
                    entity_path: e
                        .source_entity
                        .clone()
                        .map_or(self.entity_path.clone(), From::from),
                    node_id: e.source.clone(),
                },
                target: QualifiedNode {
                    entity_path: e
                        .target_entity
                        .clone()
                        .map_or(self.entity_path.clone(), From::from),
                    node_id: e.target.clone(),
                },
            }),
            (0..).map(Instance::from),
            self.colors.iter().map(Option::Some),
            Option::<&components::Color>::default,
        )
    }
}

impl IdentifiedViewSystem for GraphEdgeVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphEdges".into()
    }
}

impl VisualizerSystem for GraphEdgeVisualizer {
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
            let all_colors = results.iter_as(query.timeline, components::Color::name());

            let data = range_zip_1x1(
                all_indexed_edges.component::<components::GraphEdge>(),
                all_colors.component::<components::Color>(),
            );

            for (_index, edges, colors) in data {
                self.data.push(GraphEdgeVisualizerData {
                    entity_path: data_result.entity_path.clone(),
                    edges,
                    colors: colors.unwrap_or_default(),
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

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

re_viewer_context::impl_component_fallback_provider!(GraphEdgeVisualizer => []);
