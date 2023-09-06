use re_arrow_store::LatestAtQuery;
use re_data_store::{EntityPath, EntityProperties, InstancePath, InstancePathHash};
use re_log_types::{RowId, TimeInt, Timeline};
use re_types::components::TensorData;
use re_types::tensor_data::DecodedTensor;
use re_types::{components::InstanceKey, ComponentName, Loggable as _};
use re_viewer_context::{
    ArchetypeDefinition, NamedViewSystem, SpaceViewSystemExecutionError, TensorDecodeCache,
    ViewContextCollection, ViewPartSystem, ViewQuery, ViewerContext,
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
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![TensorData::name()]
    }

    /// Tensor view doesn't handle 2D images, see [`TensorSystem::load_tensor_entity`]
    fn queries_any_components_of(
        &self,
        store: &re_arrow_store::DataStore,
        ent_path: &EntityPath,
        components: &[ComponentName],
    ) -> bool {
        if !components.contains(&TensorData::name()) {
            return false;
        }

        // TODO(jleibs): Use indicator components

        if let Some(tensor) = store.query_latest_component::<TensorData>(
            ent_path,
            &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
        ) {
            !tensor.0.is_shaped_like_an_image() && !tensor.0.is_vector()
        } else {
            false
        }
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
        ctx: &mut ViewerContext<'_>,
        ent_path: &EntityPath,
        row_id: RowId,
        _props: &EntityProperties,
        tensor: TensorData,
    ) {
        if !tensor.0.is_shaped_like_an_image() {
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
                    re_log::warn_once!(
                        "Failed to decode decoding tensor at path {ent_path}: {err}"
                    );
                }
            }
        }
    }
}
