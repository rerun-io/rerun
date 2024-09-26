use re_viewer::external::{
    re_log_types::EntityPath,
    re_renderer,
    re_types::{
        self,
        components::{GraphEdge, GraphNodeId},
        Loggable as _,
    },
    re_viewer_context::{
        self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext,
        ViewContextCollection, ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo,
        VisualizerSystem,
    },
};

#[derive(Default)]
pub struct GraphEdgeSystem {
    pub edges: Vec<(EntityPath, Vec<EdgeWithInstance>)>,
    pub globals: Vec<(EntityPath, Vec<EdgeWithInstance>)>,
}

pub struct EdgeWithInstance {
    pub edge: GraphEdge,
    pub label: Option<String>,
}

impl IdentifiedViewSystem for GraphEdgeSystem {
    fn identifier() -> ViewSystemIdentifier {
        "GraphEdges".into()
    }
}

impl VisualizerSystem for GraphEdgeSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<re_types::archetypes::GraphEdges>()
    }

    /// Populates the scene part with data from the store.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = ctx.recording().query_caches().latest_at(
                ctx.recording_store(),
                &ctx.current_query(),
                &data_result.entity_path,
                [GraphEdge::name()],
            );

            if let Some(edges) = results.component_batch::<GraphEdge>() {
                // log::debug!("Edges: {:?}", edges);

                self.edges.push((
                    data_result.entity_path.clone(),
                    edges
                        .iter()
                        .map(|edge| EdgeWithInstance {
                            edge: edge.to_owned(),
                            label: None,
                        })
                        .collect(),
                ));
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

re_viewer_context::impl_component_fallback_provider!(GraphEdgeSystem => []);
