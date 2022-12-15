use re_data_store::{query::visit_type_data, FieldName};
use re_log_types::{IndexHash, MsgId, ObjectType, Tensor};

use crate::{misc::ViewerContext, ui::SceneQuery};

// ---

/// A tensor scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneTensor {
    pub tensors: Vec<Tensor>,
}

impl SceneTensor {
    /// Loads all tensor objects into the scene according to the given query.
    pub(crate) fn load_objects(&mut self, ctx: &ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        self.load_tensors(ctx, query);
    }

    fn load_tensors(&mut self, ctx: &ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let tensors = query
            .iter_object_stores(ctx.log_db, &[ObjectType::Image])
            .filter_map(|(_obj_type, _obj_path, time_query, obj_store)| {
                let mut tensors = Vec::new();
                visit_type_data(
                    obj_store,
                    &FieldName::from("tensor"),
                    &time_query,
                    |_instance_index: Option<&IndexHash>,
                     _time: i64,
                     _msg_id: &MsgId,
                     tensor: &re_log_types::Tensor| {
                        tensors.push(tensor.clone() /* shallow */);
                    },
                );

                // We have a special tensor viewer that (currently) only works
                // when we only have a single tensor (and no bounding boxes etc).
                // It is also not as great for images as the normal 2d view (at least not yet).
                // This is a hacky-way of detecting this special case.
                // TODO(emilk): integrate the tensor viewer into the 2D viewer instead,
                // so we can stack bounding boxes etc on top of it.
                if tensors.len() == 1 {
                    let tensor = tensors.pop().unwrap();

                    if !tensor.is_shaped_like_an_image() {
                        return Some(tensor);
                    }
                }

                None
            });

        self.tensors.extend(tensors);
    }
}

impl SceneTensor {
    pub fn is_empty(&self) -> bool {
        let Self { tensors } = self;

        tensors.is_empty()
    }
}
