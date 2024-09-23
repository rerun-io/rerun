use std::string;

use re_viewer::external::{
    egui,
    re_log::external::log,
    re_log_types::{EntityPath, Instance},
    re_renderer,
    re_types::{
        self,
        archetypes::GraphNodes,
        components::{Color, GraphEdge, GraphNodeId, Text},
        ComponentName, Loggable as _,
    },
    re_viewer_context::{
        self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext,
        ViewContextCollection, ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo,
        VisualizerSystem,
    },
};

/// Our space view consist of single part which holds a list of egui colors for each entity path.
#[derive(Default)]
pub struct GraphNodeSystem {
    pub nodes: Vec<(EntityPath, Vec<NodeIdWithInstance>)>,
    pub edges: Vec<(EntityPath, Vec<EdgeWithInstance>)>,
}

pub struct NodeIdWithInstance {
    pub node_id: GraphNodeId,
    // pub instance: Instance,
    pub label: Option<String>,
}

pub struct EdgeWithInstance {
    pub edge: GraphEdge,
    // pub instance: Instance,
    pub label: Option<String>,
}

impl IdentifiedViewSystem for GraphNodeSystem {
    fn identifier() -> ViewSystemIdentifier {
        "Graph Nodes".into()
    }
}

impl VisualizerSystem for GraphNodeSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<re_types::archetypes::GraphNodes>()
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
                [GraphNodeId::name(), GraphEdge::name()],
            );

            if let Some(node_ids) = results.component_batch::<GraphNodeId>() {
                log::debug!("Node ids: {:?}", node_ids);

                self.nodes.push((
                    data_result.entity_path.clone(),
                    node_ids
                        .iter()
                        .map(|&node_id| NodeIdWithInstance {
                            node_id,
                            label: None,
                        })
                        .collect(),
                ));
            }

            if let Some(edges) = results.component_batch::<GraphEdge>() {
                log::debug!("Edges: {:?}", edges);

                self.edges.push((
                    data_result.entity_path.clone(),
                    edges
                        .iter()
                        .map(|&edge | EdgeWithInstance {
                            edge,
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

re_viewer_context::impl_component_fallback_provider!(GraphNodeSystem => []);
