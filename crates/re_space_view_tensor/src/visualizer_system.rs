use re_data_store::LatestAtQuery;
use re_entity_db::{external::re_query2::CachedLatestAtMonoResult, EntityPath};
use re_log_types::RowId;
use re_types::{archetypes::Tensor, components::TensorData, tensor_data::DecodedTensor};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, TensorDecodeCache, ViewContextCollection,
    ViewQuery, ViewerContext, VisualizerQueryInfo, VisualizerSystem,
};

#[derive(Default)]
pub struct TensorSystem {
    pub tensors: std::collections::BTreeMap<EntityPath, (RowId, DecodedTensor)>,
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
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            // TODO(#5607): what should happen if the promise is still pending?
            if let Some(tensor) = ctx
                .recording()
                .latest_at_component::<TensorData>(&data_result.entity_path, &timeline_query)
            {
                self.load_tensor_entity(ctx, &data_result.entity_path, tensor);
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TensorSystem {
    fn load_tensor_entity(
        &mut self,
        ctx: &ViewerContext<'_>,
        ent_path: &EntityPath,
        tensor: CachedLatestAtMonoResult<TensorData>,
    ) {
        let row_id = tensor.row_id();
        match ctx
            .cache
            .entry(|c: &mut TensorDecodeCache| c.entry(row_id, tensor.value.0))
        {
            Ok(decoded_tensor) => {
                self.tensors
                    .insert(ent_path.clone(), (row_id, decoded_tensor));
            }
            Err(err) => {
                re_log::warn_once!("Failed to decode decoding tensor at path {ent_path}: {err}");
            }
        }
    }
}
