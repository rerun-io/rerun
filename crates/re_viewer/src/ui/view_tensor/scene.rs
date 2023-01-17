use re_arrow_store::LatestAtQuery;
use re_data_store::{query::visit_type_data, FieldName, Index, InstanceId, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{Instance, Tensor},
    ClassicTensor, IndexHash, MsgId, ObjectType,
};
use re_query::{query_entity_with_primary, EntityView, QueryError};

use crate::{misc::ViewerContext, ui::SceneQuery};

// ---

/// A tensor scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneTensor {
    pub tensors: std::collections::BTreeMap<InstanceId, ClassicTensor>,
}

impl SceneTensor {
    /// Loads all tensor objects into the scene according to the given query.
    pub(crate) fn load_objects(&mut self, ctx: &ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        self.load_tensors_classic(ctx, query);
        self.load_tensors_arrow(ctx, query);
    }

    fn load_tensors_classic(&mut self, ctx: &ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Image])
        {
            visit_type_data(
                obj_store,
                &FieldName::from("tensor"),
                &time_query,
                |instance_index_hash: Option<&IndexHash>,
                 _time: i64,
                 _msg_id: &MsgId,
                 tensor: &re_log_types::ClassicTensor| {
                    if !tensor.is_shaped_like_an_image() {
                        let instance_index = instance_index_hash.and_then(|instance_index_hash| {
                            ctx.log_db.obj_db.store.index_from_hash(instance_index_hash)
                        });
                        let instance_id =
                            InstanceId::new(obj_path.clone(), instance_index.cloned());
                        self.tensors.insert(instance_id, tensor.clone());
                    }
                },
            );
        }
    }

    fn load_tensors_arrow(&mut self, ctx: &ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        for (ent_path, props) in query.iter_entities() {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            match query_entity_with_primary::<Tensor>(
                &ctx.log_db.obj_db.arrow_store,
                &timeline_query,
                ent_path,
                &[],
            )
            .and_then(|entity_view| self.load_tensor_entity(ent_path, &props, &entity_view))
            {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying '{:?}': {:?}", ent_path, err);
                }
            }
        }
    }

    fn load_tensor_entity(
        &mut self,
        ent_path: &ObjPath,
        _props: &ObjectProps,
        entity_view: &EntityView<Tensor>,
    ) -> Result<(), QueryError> {
        entity_view.visit1(|instance: Instance, tensor: Tensor| {
            let tensor = ClassicTensor::from(&tensor);
            if !tensor.is_shaped_like_an_image() {
                let instance_id =
                    InstanceId::new(ent_path.clone(), Some(Index::ArrowInstance(instance)));
                self.tensors.insert(instance_id, tensor);
            }
        })
    }
}
