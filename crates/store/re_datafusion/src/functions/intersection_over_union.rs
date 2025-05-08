use crate::functions::utils::{
    columnar_value_to_array_of_array, concatenate_list_of_component_arrays, create_rerun_metadata,
};
use arrow::array::{ArrayRef, FixedSizeListArray, Float64Array};
use arrow::datatypes::{DataType, Field};
use arrow_array::Array as _;
use datafusion::common::{exec_datafusion_err, exec_err, Result as DataFusionResult};
use datafusion::logical_expr::{
    ColumnarValue, ReturnFieldArgs, ScalarFunctionArgs, ScalarUDFImpl, Signature, TypeSignature,
    Volatility,
};
use itertools::multizip;
use re_types::components::Scalar;
use re_types_core::Loggable as _;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug)]
pub struct IntersectionOverUnionUdf {
    signature: Signature,
}

impl Default for IntersectionOverUnionUdf {
    fn default() -> Self {
        Self {
            signature: Signature::new(
                TypeSignature::Exact(create_input_datatypes()),
                Volatility::Immutable,
            ),
        }
    }
}

fn create_input_datatypes() -> Vec<DataType> {
    vec![
        // Position2D - Entity 1
        DataType::new_list(
            DataType::new_fixed_size_list(DataType::Float64, 2, false),
            true,
        ),
        // HalfSize2D - Entity 1
        DataType::new_list(
            DataType::new_fixed_size_list(DataType::Float64, 2, false),
            true,
        ),
        // Position2D - Entity 2
        DataType::new_list(
            DataType::new_fixed_size_list(DataType::Float64, 2, false),
            true,
        ),
        // HalfSize2D - Entity 2
        DataType::new_list(
            DataType::new_fixed_size_list(DataType::Float64, 2, false),
            true,
        ),
    ]
}

fn create_output_field() -> Field {
    Field::new(
        "Scalar",
        DataType::new_list(Scalar::arrow_datatype(), true),
        true,
    )
    .with_metadata(create_rerun_metadata(
        None,
        "Scalar",
        Some("Scalars"),
        Some("scalars"),
        "data",
        false,
    ))
}

fn box_min(pos: &Float64Array, half_size: &Float64Array) -> (f64, f64) {
    (
        pos.value(0) - half_size.value(0),
        pos.value(1) - half_size.value(1),
    )
}

fn box_max(pos: &Float64Array, half_size: &Float64Array) -> (f64, f64) {
    (
        pos.value(0) + half_size.value(0),
        pos.value(1) + half_size.value(1),
    )
}

fn box_area(half_size: &Float64Array) -> f64 {
    4.0 * half_size.value(0) * half_size.value(1)
}

fn intersects(
    ent1_pos: &Float64Array,
    ent1_half_size: &Float64Array,
    ent2_pos: &Float64Array,
    ent2_half_size: &Float64Array,
) -> bool {
    let ent1_min = box_min(ent1_pos, ent1_half_size);
    let ent1_max = box_max(ent1_pos, ent1_half_size);
    let ent2_min = box_min(ent2_pos, ent2_half_size);
    let ent2_max = box_max(ent2_pos, ent2_half_size);

    // Check if boxes overlap in both x and y directions
    ent1_min.0 <= ent2_max.0
        && ent1_max.0 >= ent2_min.0
        && ent1_min.1 <= ent2_max.1
        && ent1_max.1 >= ent2_min.1
}

fn intersection_area(
    ent1_pos: &Float64Array,
    ent1_half_size: &Float64Array,
    ent2_pos: &Float64Array,
    ent2_half_size: &Float64Array,
) -> Option<f64> {
    if !intersects(ent1_pos, ent1_half_size, ent2_pos, ent2_half_size) {
        return None;
    }

    let ent1_min = box_min(ent1_pos, ent1_half_size);
    let ent1_max = box_max(ent1_pos, ent1_half_size);
    let ent2_min = box_min(ent2_pos, ent2_half_size);
    let ent2_max = box_max(ent2_pos, ent2_half_size);

    let intersection_min_x = ent1_min.0.max(ent2_min.0);
    let intersection_min_y = ent1_min.1.max(ent2_min.1);

    let intersection_max_x = ent1_max.0.min(ent2_max.0);
    let intersection_max_y = ent1_max.1.min(ent2_max.1);

    let intersection_width = intersection_max_x - intersection_min_x;
    let intersection_height = intersection_max_y - intersection_min_y;

    Some(intersection_width * intersection_height)
}

fn compute_intersection_over_union(
    ent1_pos_arr: &FixedSizeListArray,
    ent1_half_size_arr: &FixedSizeListArray,
    ent2_pos_arr: &FixedSizeListArray,
    ent2_half_size_arr: &FixedSizeListArray,
) -> DataFusionResult<ArrayRef> {
    let result = multizip((
        ent1_pos_arr.iter(),
        ent1_half_size_arr.iter(),
        ent2_pos_arr.iter(),
        ent2_half_size_arr.iter(),
    ))
    .map(|entry| {
        let (Some(ent1_pos), Some(ent1_half_size), Some(ent2_pos), Some(ent2_half_size)) = entry
        else {
            return Ok(None);
        };

        let ent1_pos = ent1_pos
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or(exec_datafusion_err!("Incorrect array type"))?;
        let ent1_half_size = ent1_half_size
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or(exec_datafusion_err!("Incorrect array type"))?;
        let ent2_pos = ent2_pos
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or(exec_datafusion_err!("Incorrect array type"))?;
        let ent2_half_size = ent2_half_size
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or(exec_datafusion_err!("Incorrect array type"))?;

        let intersection_area =
            intersection_area(ent1_pos, ent1_half_size, ent2_pos, ent2_half_size)
                .map(|intersection| {
                    let union = box_area(ent1_half_size) + box_area(ent2_half_size) - intersection;
                    intersection / union
                })
                .unwrap_or(0.0);

        Ok(Some(Scalar::from(intersection_area)))
    })
    .collect::<DataFusionResult<Vec<_>>>()?;

    let result_arr = Scalar::to_arrow_opt(result).map_err(|err| exec_datafusion_err!("{err}"))?;
    Ok(result_arr)
}

impl ScalarUDFImpl for IntersectionOverUnionUdf {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        "intersection_over_union"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _arg_types: &[DataType]) -> DataFusionResult<DataType> {
        exec_err!("use return_field_from_args instead")
    }

    fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> DataFusionResult<Field> {
        if args.arg_fields.len() != 4 {
            exec_err!("UDF expects 4 arguments.")?;
        }

        Ok(create_output_field())
    }

    fn invoke_with_args(
        &self,
        args: ScalarFunctionArgs<'_, '_>,
    ) -> DataFusionResult<ColumnarValue> {
        let ent1_pos_arr = columnar_value_to_array_of_array(&args.args[0], "Position2D")?;
        let ent1_half_size_arr = columnar_value_to_array_of_array(&args.args[1], "HalfSize2D")?;
        let ent2_pos_arr = columnar_value_to_array_of_array(&args.args[2], "Position2D")?;
        let ent2_half_size_arr = columnar_value_to_array_of_array(&args.args[3], "HalfSize2D")?;

        let result = multizip((
            ent1_pos_arr.iter(),
            ent1_half_size_arr.iter(),
            ent2_pos_arr.iter(),
            ent2_half_size_arr.iter(),
        ))
        .map(|entry| {
            let (Some(ent1_pos), Some(ent1_half_size), Some(ent2_pos), Some(ent2_half_size)) =
                entry
            else {
                return Ok(None);
            };

            let ent1_pos = ent1_pos
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or(exec_datafusion_err!("Incorrect data type for Position2D"))?;
            let ent1_half_size = ent1_half_size
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or(exec_datafusion_err!("Incorrect data type for Position2D"))?;
            let ent2_pos = ent2_pos
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or(exec_datafusion_err!("Incorrect data type for Position2D"))?;
            let ent2_half_size = ent2_half_size
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or(exec_datafusion_err!("Incorrect data type for Position2D"))?;

            let intersection_over_union = compute_intersection_over_union(
                ent1_pos,
                ent1_half_size,
                ent2_pos,
                ent2_half_size,
            )?;

            Ok(Some(intersection_over_union))
        })
        .collect::<DataFusionResult<Vec<_>>>()?;

        let results = concatenate_list_of_component_arrays::<Scalar>(&result)?;

        Ok(ColumnarValue::Array(results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datafusion::logical_expr::{col, ScalarUDF};
    use datafusion::prelude::{ParquetReadOptions, SessionContext};

    #[tokio::test]
    async fn test_intersection_over_union() -> DataFusionResult<()> {
        let ctx = SessionContext::default();
        let udf = ScalarUDF::new_from_impl(IntersectionOverUnionUdf::default());
        let df = ctx
            .read_parquet(
                "/Users/tsaucer/working/intersection_over_union_example.parquet",
                ParquetReadOptions::default(),
            )
            .await?
            .select(vec![
                col("frame"),
                col("log_tick"),
                col("log_time"),
                udf.call(vec![
                    col("/video/tracked/16:Position2D"),
                    col("/video/tracked/16:HalfSize2D"),
                    col("/video/tracked/21:Position2D"),
                    col("/video/tracked/21:HalfSize2D"),
                ]),
            ])?;

        df.show().await?;

        Ok(())
    }
}
