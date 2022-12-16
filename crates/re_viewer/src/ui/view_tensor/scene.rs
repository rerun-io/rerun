use re_data_store::{query::visit_type_data, FieldName, InstanceId};
use re_log_types::{IndexHash, MsgId, ObjectType, Tensor};

use crate::{misc::ViewerContext, ui::SceneQuery};

// ---

/// A tensor scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneTensor {
    pub tensors: std::collections::BTreeMap<InstanceId, Tensor>,
}

impl SceneTensor {
    /// Loads all tensor objects into the scene according to the given query.
    pub(crate) fn load_objects(&mut self, ctx: &ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        self.load_tensors(ctx, query);
    }

    fn load_tensors(&mut self, ctx: &ViewerContext<'_>, query: &SceneQuery<'_>) {
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
                 tensor: &re_log_types::Tensor| {
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
}

impl SceneTensor {
    pub fn is_empty(&self) -> bool {
        let Self { tensors } = self;

        tensors.is_empty()
    }
}
