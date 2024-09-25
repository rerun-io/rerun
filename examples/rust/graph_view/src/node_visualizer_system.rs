use re_viewer::external::{
    re_chunk::LatestAtQuery, re_log_types::{EntityPath, Instance}, re_renderer, re_space_view::{DataResultQuery, RangeResultsExt}, re_types::{
        self, archetypes::GraphNodes, components::{self, GraphEdge, GraphNodeId}, Loggable as _
    }, re_viewer_context::{
        self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext,
        ViewContextCollection, ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo,
        VisualizerSystem,
    }
};

/// Our space view consist of single part which holds a list of egui colors for each entity path.
#[derive(Default)]
pub struct GraphNodeSystem {
    pub nodes: Vec<GraphNodesEntry>,
}

pub struct GraphNodesEntry {
    pub entity_path: EntityPath,
    pub nodes_batch: Vec<components::GraphNodeId>,
}


impl IdentifiedViewSystem for GraphNodeSystem {
    fn identifier() -> ViewSystemIdentifier {
        "GraphNodes".into()
    }
}

impl VisualizerSystem for GraphNodeSystem {
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

                let Some(all_node_ids) = results.results.component_batch::<GraphNodeId>() else {
                    continue;
                };

                self.nodes.push(GraphNodesEntry{
                    nodes_batch: all_node_ids,
                    entity_path: data_result.entity_path.clone(),
                });



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

re_viewer_context::impl_component_fallback_provider!(GraphNodeSystem => []);
