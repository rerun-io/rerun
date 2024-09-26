use re_viewer::external::{
    re_chunk::LatestAtQuery,
    re_log_types::EntityPath,
    re_query, re_renderer,
    re_space_view::{DataResultQuery, RangeResultsExt},
    re_types::{
        self,
        archetypes::GraphNodes,
        components::{self, Color, GraphNodeId},
        Loggable as _,
    },
    re_viewer_context::{
        self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext,
        ViewContextCollection, ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo,
        VisualizerSystem,
    },
};

/// Our space view consist of single part which holds a list of egui colors for each entity path.
#[derive(Default)]
pub struct GraphNodeVisualizer {
    pub data: Vec<GraphViewVisualizerData>,
}

pub struct GraphViewVisualizerData {
    pub entity_path: EntityPath,
    pub nodes: Vec<(GraphNodeId, Option<Color>)>,
}

impl IdentifiedViewSystem for GraphNodeVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphNodes".into()
    }
}

impl GraphNodeVisualizer {
    fn process_data(
        &mut self,
        entity_path: &EntityPath,
        data: impl Iterator<Item = Vec<(GraphNodeId, Option<Color>)>>,
    ) {
        for nodes in data {
            self.data.push(GraphViewVisualizerData {
                entity_path: entity_path.to_owned(),
                nodes,
            });
        }
    }
}

impl VisualizerSystem for GraphNodeVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<GraphNodes>()
    }

    /// Populates the scene part with data from the store.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);

        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = data_result
                .latest_at_with_blueprint_resolved_data::<GraphNodes>(ctx, &timeline_query);

            let Some(all_node_ids) = results.get_required_chunks(&components::GraphNodeId::name())
            else {
                continue;
            };

            let all_nodes_indexed = all_node_ids.iter().flat_map(move |chunk| {
                itertools::izip!(
                    chunk.iter_component_indices(
                        &view_query.timeline,
                        &components::GraphNodeId::name()
                    ),
                    chunk.iter_component::<components::GraphNodeId>()
                )
            });
            let all_colors = results.iter_as(view_query.timeline, components::Color::name());

            let data = re_query::range_zip_1x1(
                all_nodes_indexed,
                all_colors.component::<components::Color>(),
            )
            .map(|(_index, node_ids, colors)| {
                // TODO: Use an iterator here:
                re_query::clamped_zip_1x1(
                    node_ids.iter().cloned(),
                    colors.unwrap_or_default().iter().map(|&c| Some(c)),
                    Option::<Color>::default,
                )
                .collect::<Vec<_>>()
            });

            self.process_data(&data_result.entity_path, data);
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
