use re_log_types::{EntityPath, Instance};
use re_viewer::external::{
    egui::Color32,
    re_chunk::{ChunkComponentIterItem, LatestAtQuery},
    re_query::{clamped_zip_2x1, range_zip_1x1},
    re_renderer,
    re_space_view::{DataResultQuery, RangeResultsExt},
    re_types::{self, archetypes, datatypes, components::{self, GraphNodeId}, Loggable as _},
    re_viewer_context::{
        self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext,
        ViewContextCollection, ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo,
        VisualizerSystem,
    },
};

#[derive(Default)]
pub struct EdgeUndirectedVisualizer {
    pub data: Vec<EdgeUndirectedVisualizerData>,
}

pub struct EdgeUndirectedVisualizerData {
    pub entity_path: re_log_types::EntityPath,
    edges: ChunkComponentIterItem<components::GraphEdgeUndirected>,
    colors: ChunkComponentIterItem<components::Color>,
}

pub struct EdgeInstance<'a> {
    pub source: &'a datatypes::GraphLocation,
    pub target: &'a datatypes::GraphLocation,
    pub entity_path: &'a re_log_types::EntityPath,
    pub instance: Instance,
    pub color: Option<Color32>,
}

impl<'a> EdgeInstance<'a> {
    pub fn nodes(&'a self) -> impl Iterator<Item = datatypes::GraphLocation> {
        [self.source.clone(), self.target.clone()].into_iter()
    }
}

impl EdgeUndirectedVisualizerData {
    pub fn edges(&self) -> impl Iterator<Item = EdgeInstance> {
        clamped_zip_2x1(
            self.edges.iter(),
            (0..).map(Instance::from),
            self.colors.iter().map(Option::Some),
            Option::<&components::Color>::default,
        )
        .map(|(edge, instance, color)| EdgeInstance {
            source: &edge.source,
            target: &edge.target,
            entity_path: &self.entity_path,
            instance,
            color: color.map(|c| Color32::from(c.0)),
        })
    }
}

impl IdentifiedViewSystem for EdgeUndirectedVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphEdgesUndirected".into()
    }
}

impl VisualizerSystem for EdgeUndirectedVisualizer {
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

            let all_indexed_edges =
                results.iter_as(query.timeline, components::GraphEdgeUndirected::name());
            let all_colors = results.iter_as(query.timeline, components::Color::name());

            let data = range_zip_1x1(
                all_indexed_edges.component::<components::GraphEdgeUndirected>(),
                all_colors.component::<components::Color>(),
            );

            for (_index, edges, colors) in data {
                self.data.push(EdgeUndirectedVisualizerData {
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

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

re_viewer_context::impl_component_fallback_provider!(EdgeUndirectedVisualizer => []);
