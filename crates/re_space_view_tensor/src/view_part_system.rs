use re_arrow_store::LatestAtQuery;
use re_components::{DecodedTensor, Tensor};
use re_data_store::{EntityPath, EntityProperties, InstancePath};
use re_log_types::{Component as _, InstanceKey};
use re_viewer_context::{
    ArchetypeDefinition, SpaceViewSystemExecutionError, TensorDecodeCache, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
};

/// A bar chart scene, with everything needed to render it.
#[derive(Default)]
pub struct TensorSystem {
    pub tensors: std::collections::BTreeMap<InstancePath, DecodedTensor>,
}

impl ViewPartSystem for TensorSystem {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![Tensor::name()]
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let store = &ctx.store_db.entity_db.data_store;
        for (ent_path, props) in query.iter_entities() {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            if let Some(tensor) = store.query_latest_component::<Tensor>(ent_path, &timeline_query)
            {
                self.load_tensor_entity(ctx, ent_path, &props, tensor);
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
        _props: &EntityProperties,
        tensor: Tensor,
    ) {
        if !tensor.is_shaped_like_an_image() {
            match ctx.cache.entry(|c: &mut TensorDecodeCache| c.entry(tensor)) {
                Ok(tensor) => {
                    let instance_path = InstancePath::instance(ent_path.clone(), InstanceKey(0));
                    self.tensors.insert(instance_path, tensor);
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
