use re_arrow_store::LatestAtQuery;
use re_data_store::{EntityPath, EntityProperties, InstancePath};
use re_log_types::{
    component_types::{InstanceKey, Tensor},
    DecodedTensor,
};
use re_viewer_context::{SceneQuery, TensorDecodeCache, ViewerContext};

/// A tensor scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneTensor {
    pub tensors: std::collections::BTreeMap<InstancePath, DecodedTensor>,
}

impl SceneTensor {
    /// Loads all tensors into the scene according to the given query.
    pub(crate) fn load(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let store = &ctx.log_db.entity_db.data_store;
        for (ent_path, props) in query.iter_entities() {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            if let Some(tensor) = store.query_latest_component::<Tensor>(ent_path, &timeline_query)
            {
                self.load_tensor_entity(ctx, ent_path, &props, tensor);
            }
        }
    }

    fn load_tensor_entity(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ent_path: &EntityPath,
        _props: &EntityProperties,
        tensor: Tensor,
    ) {
        if !tensor.is_shaped_like_an_image() {
            match ctx.cache.entry::<TensorDecodeCache>().entry(tensor) {
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
