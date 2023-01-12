use std::collections::BTreeMap;

use re_data_store::InstanceId;
use re_log_types::{ClassicTensor, TensorDataType};

use crate::{misc::ViewerContext, ui::scene::SceneQuery};

pub struct BarChartValues {
    pub values: Vec<f64>,
}

/// A bar chart scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneBarChart {
    pub charts: BTreeMap<InstanceId, BarChartValues>,
}

impl SceneBarChart {
    pub(crate) fn load_objects(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        self.load_tensors(ctx, query);
    }

    fn load_tensors(&mut self, ctx: &ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[re_log_types::ObjectType::Image])
        {
            re_data_store::query::visit_type_data(
                obj_store,
                &re_data_store::FieldName::from("tensor"),
                &time_query,
                |instance_index_hash: Option<&re_log_types::IndexHash>,
                 _time: i64,
                 _msg_id: &re_log_types::MsgId,
                 tensor: &re_log_types::ClassicTensor| {
                    if tensor.is_vector() {
                        if let Some(values) = tensor_to_values(tensor) {
                            let instance_index =
                                instance_index_hash.and_then(|instance_index_hash| {
                                    ctx.log_db.obj_db.store.index_from_hash(instance_index_hash)
                                });
                            let instance_id =
                                InstanceId::new(obj_path.clone(), instance_index.cloned());
                            let chart = BarChartValues { values };
                            self.charts.insert(instance_id, chart);
                        }
                    }
                },
            );
        }
    }
}

fn tensor_to_values(tensor: &ClassicTensor) -> Option<Vec<f64>> {
    match tensor.dtype() {
        TensorDataType::U8 => tensor
            .data
            .as_slice::<u8>()
            .map(|slice| slice.iter().map(|&value| value as f64).collect()),

        TensorDataType::U16 => tensor
            .data
            .as_slice::<u16>()
            .map(|slice| slice.iter().map(|&value| value as f64).collect()),

        TensorDataType::U32 => tensor
            .data
            .as_slice::<u32>()
            .map(|slice| slice.iter().map(|&value| value as f64).collect()),

        TensorDataType::U64 => tensor
            .data
            .as_slice::<u64>()
            .map(|slice| slice.iter().map(|&value| value as f64).collect()),

        TensorDataType::I8 => tensor
            .data
            .as_slice::<i8>()
            .map(|slice| slice.iter().map(|&value| value as f64).collect()),

        TensorDataType::I16 => tensor
            .data
            .as_slice::<i16>()
            .map(|slice| slice.iter().map(|&value| value as f64).collect()),

        TensorDataType::I32 => tensor
            .data
            .as_slice::<i32>()
            .map(|slice| slice.iter().map(|&value| value as f64).collect()),

        TensorDataType::I64 => tensor
            .data
            .as_slice::<i64>()
            .map(|slice| slice.iter().map(|&value| value as f64).collect()),

        TensorDataType::F16 => tensor
            .data
            .as_slice::<half::f16>()
            .map(|slice| slice.iter().map(|&value| value.to_f64()).collect()),

        TensorDataType::F32 => tensor
            .data
            .as_slice::<f32>()
            .map(|slice| slice.iter().map(|&value| value as f64).collect()),

        TensorDataType::F64 => tensor.data.as_slice::<f64>().map(|slice| slice.to_vec()),
    }
}
