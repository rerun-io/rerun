use re_arrow_store::LatestAtQuery;
use re_data_store::{EntityPath, EntityProperties, InstancePath};
use re_log_types::component_types::{InstanceKey, Tensor};
use re_query::{query_entity_with_primary, EntityView, QueryError};

use crate::{misc::ViewerContext, ui::SceneQuery, DecodedTensor};

// ---

/// A tensor scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneTensor {
    pub tensors: std::collections::BTreeMap<InstancePath, DecodedTensor>,
}

impl SceneTensor {
    /// Loads all tensors into the scene according to the given query.
    pub(crate) fn load(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        for (ent_path, props) in query.iter_entities() {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            match query_entity_with_primary::<Tensor>(
                &ctx.log_db.entity_db.data_store,
                &timeline_query,
                ent_path,
                &[],
            )
            .and_then(|entity_view| self.load_tensor_entity(ctx, ent_path, &props, &entity_view))
            {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
                }
            }
        }
    }

    fn load_tensor_entity(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ent_path: &EntityPath,
        _props: &EntityProperties,
        entity_view: &EntityView<Tensor>,
    ) -> Result<(), QueryError> {
        entity_view.visit1(|instance_key: InstanceKey, tensor: Tensor| {
            if !tensor.is_shaped_like_an_image() {
                match ctx.cache.decode.try_decode_tensor_if_necessary(tensor) {
                    Ok(tensor) => {
                        let instance_path = InstancePath::instance(ent_path.clone(), instance_key);
                        self.tensors.insert(instance_path, tensor);
                    }
                    Err(err) => {
                        re_log::warn_once!(
                            "Failed to decode decoding tensor at path {ent_path}: {err}"
                        );
                    }
                }
            }
        })
    }
}
