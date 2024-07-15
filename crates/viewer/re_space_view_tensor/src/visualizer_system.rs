use re_chunk_store::{LatestAtQuery, RowId};
use re_types::{archetypes::Tensor, components::TensorData};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext, ViewContextCollection,
    ViewQuery, VisualizerQueryInfo, VisualizerSystem,
};

#[derive(Default)]
pub struct TensorSystem {
    pub tensors: Vec<(RowId, TensorData)>,
}

impl IdentifiedViewSystem for TensorSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Tensor".into()
    }
}

impl VisualizerSystem for TensorSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Tensor>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            // TODO(#5607): what should happen if the promise is still pending?
            if let Some(tensor) = ctx
                .recording()
                .latest_at_component::<TensorData>(&data_result.entity_path, &timeline_query)
            {
                self.tensors.push((tensor.row_id(), tensor.value));
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

re_viewer_context::impl_component_fallback_provider!(TensorSystem => []);
