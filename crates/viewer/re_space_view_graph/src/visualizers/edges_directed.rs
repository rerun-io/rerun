use re_chunk::{ChunkComponentIterItem, LatestAtQuery};
use re_query::{clamped_zip_2x1, range_zip_1x1};
use re_log_types::Instance;
use re_space_view::{DataResultQuery, RangeResultsExt};
use re_types::{
    self, archetypes,
    components::{self},
    Loggable as _,
};
use re_viewer_context::{
    self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo, VisualizerSystem,
};

use crate::types::EdgeInstance;

#[derive(Default)]
pub struct EdgesDirectedVisualizer {
    pub data: Vec<EdgesDirectedData>,
}

pub struct EdgesDirectedData {
    pub entity_path: re_log_types::EntityPath,
    edges: ChunkComponentIterItem<components::GraphEdgeDirected>,
    colors: ChunkComponentIterItem<components::Color>,
}

impl EdgesDirectedData {
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
            _entity_path: &self.entity_path,
            instance,
            color: color.map(|c| egui::Color32::from(c.0)),
        })
    }
}

impl IdentifiedViewSystem for EdgesDirectedVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphEdgesDirected".into()
    }
}

impl VisualizerSystem for EdgesDirectedVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<archetypes::GraphEdgesDirected>()
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
                .latest_at_with_blueprint_resolved_data::<archetypes::GraphEdgesDirected>(
                    ctx,
                    &timeline_query,
                );

            let all_indexed_edges =
                results.iter_as(query.timeline, components::GraphEdgeDirected::name());
            let all_colors = results.iter_as(query.timeline, components::Color::name());

            let data = range_zip_1x1(
                all_indexed_edges.component::<components::GraphEdgeDirected>(),
                all_colors.component::<components::Color>(),
            );

            for (_index, edges, colors) in data {
                self.data.push(EdgesDirectedData {
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

re_viewer_context::impl_component_fallback_provider!(EdgesDirectedVisualizer => []);
