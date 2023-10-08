use re_arrow_store::LatestAtQuery;
use re_data_store::{EntityPath, EntityProperties, InstancePath, InstancePathHash};
use re_log_types::RowId;
use re_types::{
    archetypes::Tensor,
    components::{InstanceKey, TensorData},
    tensor_data::DecodedTensor,
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    NamedViewSystem, SpaceViewSystemExecutionError, TensorDecodeCache, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
};

#[derive(Default)]
pub struct TensorSystem {
    pub tensors: std::collections::BTreeMap<InstancePath, (RowId, DecodedTensor)>,
}

impl NamedViewSystem for TensorSystem {
    fn name() -> re_viewer_context::ViewSystemName {
        "Tensor".into()
    }
}

impl ViewPartSystem for TensorSystem {
    fn required_components(&self) -> ComponentNameSet {
        Tensor::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(Tensor::indicator().name()).collect()
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let store = &ctx.store_db.entity_db.data_store;
        for (ent_path, props) in query.iter_entities_for_system(Self::name()) {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            if let Some(tensor) =
                store.query_latest_component::<TensorData>(ent_path, &timeline_query)
            {
                self.load_tensor_entity(ctx, ent_path, tensor.row_id, &props, tensor.value);
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
        row_id: RowId,
        _props: &EntityProperties,
        tensor: TensorData,
    ) {
        // NOTE: Tensors don't support batches at the moment so always splat.
        let tensor_path_hash = InstancePathHash::entity_splat(ent_path).versioned(row_id);
        match ctx
            .cache
            .entry(|c: &mut TensorDecodeCache| c.entry(tensor_path_hash, tensor.0))
        {
            Ok(tensor) => {
                let instance_path = InstancePath::instance(ent_path.clone(), InstanceKey(0));
                self.tensors.insert(instance_path, (row_id, tensor));
            }
            Err(err) => {
                re_log::warn_once!("Failed to decode decoding tensor at path {ent_path}: {err}");
            }
        }
    }
}
