use re_log_types::Instance;
use re_viewer::external::{
    egui,
    re_chunk::{ChunkComponentIterItem, LatestAtQuery},
    re_query::{clamped_zip_2x1, range_zip_1x1},
    re_renderer,
    re_space_view::{DataResultQuery, RangeResultsExt},
    re_types::{
        self, archetypes,
        components::{self},
        Loggable as _,
    },
    re_viewer_context::{
        self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext,
        ViewContextCollection, ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo,
        VisualizerSystem,
    },
};

use crate::types::EdgeInstance;

#[derive(Default)]
pub struct EdgesUndirectedVisualizer {
    pub data: Vec<EdgesUndirectedData>,
}

pub struct EdgesUndirectedData {
    pub entity_path: re_log_types::EntityPath,
    edges: ChunkComponentIterItem<components::GraphEdgeUndirected>,
    colors: ChunkComponentIterItem<components::Color>,
}

impl EdgesUndirectedData {
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
            color: color.map(|c| egui::Color32::from(c.0)),
        })
    }
}

impl IdentifiedViewSystem for EdgesUndirectedVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphEdgesUndirected".into()
    }
}

impl VisualizerSystem for EdgesUndirectedVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<archetypes::GraphEdgesUndirected>()
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
                .latest_at_with_blueprint_resolved_data::<archetypes::GraphEdgesUndirected>(
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
                self.data.push(EdgesUndirectedData {
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

re_viewer_context::impl_component_fallback_provider!(EdgesUndirectedVisualizer => []);
