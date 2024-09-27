use re_viewer::external::{
    re_chunk::{ChunkComponentIterItem, LatestAtQuery},
    re_log_types::EntityPath,
    re_query::{clamped_zip_1x1, range_zip_1x1},
    re_renderer,
    re_space_view::{DataResultQuery, RangeResultsExt},
    re_types::{self, archetypes, components, Loggable as _},
    re_viewer_context::{
        self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext,
        ViewContextCollection, ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo,
        VisualizerSystem,
    },
};

/// Our space view consist of single part which holds a list of egui colors for each entity path.
#[derive(Default)]
pub struct GraphNodeVisualizer {
    pub(crate) data: Vec<GraphNodeVisualizerData>,
}

pub struct GraphNodeVisualizerData {
    pub(crate) entity_path: EntityPath,
    pub(crate) node_ids: ChunkComponentIterItem<components::GraphNodeId>,
    pub(crate) colors: ChunkComponentIterItem<components::Color>,
}

impl GraphNodeVisualizerData {
    pub(crate) fn nodes(
        &self,
    ) -> impl Iterator<Item = (&components::GraphNodeId, Option<&components::Color>)> {
        clamped_zip_1x1(
            self.node_ids.iter(),
            self.colors.iter().map(Option::Some),
            Option::<&components::Color>::default,
        )
    }
}

impl IdentifiedViewSystem for GraphNodeVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphNodes".into()
    }
}

impl VisualizerSystem for GraphNodeVisualizer {
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

            let all_indexed_nodes = results.iter_as(query.timeline, components::GraphNodeId::name());
            let all_colors = results.iter_as(query.timeline, components::Color::name());

            let data = range_zip_1x1(
                all_indexed_nodes.component::<components::GraphNodeId>(),
                all_colors.component::<components::Color>(),
            );

            for (_index, node_ids, colors) in data {
                self.data.push(GraphNodeVisualizerData {
                    entity_path: data_result.entity_path.clone(),
                    node_ids,
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

re_viewer_context::impl_component_fallback_provider!(GraphNodeVisualizer => []);
