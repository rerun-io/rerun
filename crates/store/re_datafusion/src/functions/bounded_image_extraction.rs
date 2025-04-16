use arrow::array::{
    ArrayRef, FixedSizeListArray, Float64Array, GenericListBuilder, ListArray,
    NullBuilder, StructArray, UInt16Array,
};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field, Fields};
use datafusion::common::{exec_datafusion_err, exec_err, Result as DataFusionResult};
use datafusion::error::DataFusionError;
use datafusion::logical_expr::{
    ColumnarValue, ReturnFieldArgs, ScalarFunctionArgs, ScalarUDFImpl, Signature, TypeSignature,
    Volatility,
};
use itertools::multizip;
use re_dataframe::external::re_chunk::ArrowArray as _;
use re_types::components::ImageBuffer;
use re_types::components::ImageFormat;
use re_types_core::Loggable as _;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct BoundedImageExtractionUdf {
    output_entity_path: String,
    class_of_interest: u16,
    signature: Signature,
}

impl BoundedImageExtractionUdf {
    pub fn new(output_entity_path: impl Into<String>, class_of_interest: u16) -> Self {
        Self {
            output_entity_path: output_entity_path.into(),
            class_of_interest,
            signature: Signature::new(
                TypeSignature::Exact(create_input_datatypes()),
                Volatility::Immutable,
            ),
        }
    }
}

fn create_input_datatypes() -> Vec<DataType> {
    vec![
        DataType::new_list(DataType::UInt16, true),
        DataType::new_list(
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float64, false)), 2),
            true,
        ),
        DataType::new_list(
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float64, false)), 2),
            true,
        ),
    ]
}

fn format_field_data_type() -> DataType {
    DataType::Struct(Fields::from(vec![
        Field::new("width", DataType::UInt32, false),
        Field::new("height", DataType::UInt32, false),
        Field::new("pixel_format", DataType::UInt8, true),
        Field::new("color_model", DataType::UInt8, true),
        Field::new("channel_datatype", DataType::UInt8, true),
    ]))
}

fn create_output_fields(output_entity_path: &str) -> Vec<Field> {
    let image_indicator_field = Field::new(
        format!("{output_entity_path}:ImageIndicator"),
        DataType::new_list(DataType::Null, true),
        true,
    )
    .with_metadata(create_rerun_metadata(
        output_entity_path,
        "ImageIndicator",
        None,
        None,
        "data",
        true,
    ));

    let image_buffer_field = Field::new(
        format!("{output_entity_path}:ImageBuffer"),
        DataType::new_list(DataType::new_list(DataType::UInt8, false), true),
        true,
    )
    .with_metadata(create_rerun_metadata(
        output_entity_path,
        "ImageBuffer",
        Some("Image"),
        Some("buffer"),
        "data",
        false,
    ));

    let image_format_field = Field::new(
        format!("{output_entity_path}:ImageFormat"),
        DataType::new_list(format_field_data_type(), true),
        true,
    )
    .with_metadata(create_rerun_metadata(
        output_entity_path,
        "ImageFormat",
        Some("Image"),
        None,
        "data",
        false,
    ));

    vec![
        image_indicator_field,
        image_buffer_field,
        image_format_field,
    ]
}

fn create_rerun_metadata(
    entity_path: &str,
    component: &str,
    archetype: Option<&str>,
    archetype_field: Option<&str>,
    kind: &str,
    is_indicator: bool,
) -> HashMap<String, String> {
    let mut metadata: HashMap<String, String> = [
        ("rerun.entity_path".to_owned(), entity_path.into()),
        (
            "rerun.component".to_owned(),
            format!("rerun.components.{component}"),
        ),
        ("rerun.kind".to_owned(), kind.into()),
    ]
    .into_iter()
    .collect();

    if is_indicator {
        metadata.insert("rerun.is_indicator".to_owned(), "true".to_owned());
    }
    if let Some(archetype) = archetype {
        metadata.insert(
            "rerun.archetype".to_owned(),
            format!("rerun.archetypes.{archetype}"),
        );
    }
    if let Some(archetype_field) = archetype_field {
        metadata.insert("rerun.archetype_field".to_owned(), archetype_field.into());
    }

    metadata
}

fn find_largest_detection(
    class_of_interest: u16,
    class_ids: &UInt16Array,
    centers: &FixedSizeListArray,
    half_sizes: &FixedSizeListArray,
) -> Option<[u32; 4]> {
    let result = multizip((class_ids.iter(), centers.iter(), half_sizes.iter()))
        .filter_map(|combined_iter| match combined_iter {
            (Some(class_id), Some(center), Some(half_size)) => {
                if class_id == class_of_interest {
                    Some((center, half_size))
                } else {
                    None
                }
            }
            _ => None,
        })
        .max_by_key(|(_center, half_sizes)| {
            let Some(area_array) = half_sizes.as_any().downcast_ref::<Float64Array>() else {
                return 0;
            };
            // Since we can't find max of a float, this is good enough for what we're doing here
            (area_array.value(0) * area_array.value(1) * 100.0) as u64
        })?;

    let best_center = result.0.as_any().downcast_ref::<Float64Array>()?;
    let best_half_size = result.1.as_any().downcast_ref::<Float64Array>()?;
    Some([
        (best_center.value(0) - best_half_size.value(0)) as u32,
        (best_center.value(1) - best_half_size.value(1)) as u32,
        (best_center.value(0) + best_half_size.value(0)) as u32,
        (best_center.value(1) + best_half_size.value(1)) as u32,
    ])
}

impl ScalarUDFImpl for BoundedImageExtractionUdf {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        "bounded_image_extraction"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _arg_types: &[DataType]) -> DataFusionResult<DataType> {
        exec_err!("use return_field_from_args instead of return_type")
    }

    fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> DataFusionResult<Field> {
        if args.arg_fields.len() != 3 {
            exec_err!(
                "UDF expects 3 arguments for components ClassId, Position2D, and HalfSize2D"
            )?;
        }

        Ok(Field::new(
            &self.output_entity_path,
            DataType::Struct(create_output_fields(&self.output_entity_path).into()),
            true,
        ))
    }

    fn invoke_with_args(
        &self,
        args: ScalarFunctionArgs<'_, '_>,
    ) -> DataFusionResult<ColumnarValue> {
        // Rerun only contains these values as arrays, but if optimizations
        // could reduce the columns value to scalar we would need to expand
        // the logic here to account for that additional complexity.
        let ColumnarValue::Array(class_ids_arr) = &args.args[0] else {
            exec_err!("Unexpect scalar columnar value for ClassID")?
        };
        let ColumnarValue::Array(centers_arr) = &args.args[1] else {
            exec_err!("Unexpect scalar columnar value for Position2D")?
        };
        let ColumnarValue::Array(half_sizes_arr) = &args.args[2] else {
            exec_err!("Unexpect scalar columnar value for HalfSize2D")?
        };
        let class_ids_arr = class_ids_arr
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or(exec_datafusion_err!("Incorrect array type for ClassID"))?;
        let centers_arr = centers_arr
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or(exec_datafusion_err!("Incorrect array type for ClassID"))?;
        let half_sizes_arr = half_sizes_arr
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or(exec_datafusion_err!("Incorrect array type for ClassID"))?;

        let result = multizip((
            class_ids_arr.iter(),
            centers_arr.iter(),
            half_sizes_arr.iter(),
        ))
        .map(|entry| {
            let (Some(class_ids), Some(centers), Some(half_sizes)) = entry else {
                return Ok(None);
            };
            let class_ids = class_ids
                .as_any()
                .downcast_ref::<UInt16Array>()
                .ok_or(exec_datafusion_err!("Incorrect data type for ClassID"))?;
            let centers = centers
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or(exec_datafusion_err!("Incorrect data type for Position2D"))?;
            let half_sizes = half_sizes
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or(exec_datafusion_err!("Incorrect data type for Position2D"))?;

            Ok(find_largest_detection(
                self.class_of_interest,
                class_ids,
                centers,
                half_sizes,
            ))
        })
        .collect::<DataFusionResult<Vec<_>>>()?;

        let num_rows = result.len();
        let fields = create_output_fields(self.output_entity_path.as_str());

        // let image_indicator_value_builder: arrow::array::GenericListBuilder<i32, arrow::array::NullBuilder> = GenericListBuilder::with_capacity(NullBuilder::new(), 0);
        // let mut image_indicator_array_builder: arrow::array::GenericListBuilder<i32, arrow::array::GenericListBuilder<i32, arrow::array::NullBuilder>> = GenericListBuilder::with_capacity(image_indicator_value_builder, num_rows);
        let mut image_indicator_array_builder: arrow::array::GenericListBuilder<
            i32,
            arrow::array::NullBuilder,
        > = GenericListBuilder::with_capacity(NullBuilder::new(), 0);
        for _ in 0..num_rows {
            // image_indicator_array_builder.values().append(true);
            image_indicator_array_builder.append(true);
        }
        let image_indicator_array = Arc::new(image_indicator_array_builder.finish()) as ArrayRef;
        // let image_indicator_array =
        //     ListArray::new_null(Arc::new(Field::new("item", DataType::Null, true)), num_rows);
        // let image_indicator_array = Arc::new(image_indicator_array) as ArrayRef;

        let result = result
            .into_iter()
            .map(|maybe_tuple| {
                maybe_tuple.map(|bounds| {
                    // TODO grab these from the actual image not just create a pattern below

                    let width = bounds[2] - bounds[0];
                    let height = bounds[3] - bounds[1];
                    let mut img_buf = image::ImageBuffer::new(width, height);

                    // Iterate over the coordinates and pixels of the image
                    for (x, y, pixel) in img_buf.enumerate_pixels_mut() {
                        let r = (0.3 * x as f32) as u8;
                        let b = (0.3 * y as f32) as u8;
                        *pixel = image::Rgb([r, 0, b]);
                    }

                    let buffer_and_format = ImageBuffer::from_image(img_buf)
                        .map_err(|err| exec_datafusion_err!("{err}"))?;

                    Ok(buffer_and_format)
                })
            })
            .map(|v| v.transpose())
            .collect::<DataFusionResult<Vec<_>>>()?;

        let (buffer, format): (Vec<Option<ImageBuffer>>, Vec<Option<ImageFormat>>) = result
            .into_iter()
            .map(|opt_bv| match opt_bv {
                Some((buffer, format)) => (Some(buffer), Some(format)),
                None => (None, None),
            })
            .unzip();

        let buffer_array_inner =
            ImageBuffer::to_arrow_opt(buffer).map_err(|err| exec_datafusion_err!("{err}"))?;
        let lengths = vec![1; buffer_array_inner.len()];
        let buffer_array = Arc::new(ListArray::try_new(
            Arc::new(Field::new_list_field(
                DataType::List(Arc::new(Field::new_list_field(DataType::UInt8, false))),
                true,
            )),
            OffsetBuffer::<i32>::from_lengths(lengths),
            buffer_array_inner,
            None,
        )?) as ArrayRef;

        let format_array_inner =
            ImageFormat::to_arrow_opt(format).map_err(|err| exec_datafusion_err!("{err}"))?;
        let lengths = vec![1; format_array_inner.len()];
        let format_array = Arc::new(ListArray::try_new(
            Arc::new(Field::new_list_field(format_field_data_type(), true)),
            OffsetBuffer::<i32>::from_lengths(lengths),
            format_array_inner,
            None,
        )?) as ArrayRef;

        let fields = Fields::from(fields);
        // StructArray::try_from(vec![
        //     (format!("{}:ImageIndicator", self.output_entity_path).as_str(), image_indicator_array),
        //     (format!("{}:ImageBuffer", self.output_entity_path).as_str(), buffer_array),
        //     (format!("{}:ImageFormat", self.output_entity_path).as_str(), format_array),
        // ])
        StructArray::try_new(
            fields,
            vec![image_indicator_array, buffer_array, format_array],
            None,
        )
        .map(|arr| ColumnarValue::Array(Arc::new(arr)))
        .map_err(DataFusionError::from)
    }
}

#[cfg(test)]
mod tests {
    use crate::functions::bounded_image_extraction::BoundedImageExtractionUdf;
    use datafusion::error::DataFusionError;
    use datafusion::logical_expr::ScalarUDF;
    use datafusion::prelude::{col, ParquetReadOptions, SessionContext};

    #[tokio::test]
    async fn test_find_largest_detection() -> Result<(), DataFusionError> {
        let ctx = SessionContext::default();

        let extraction_udf =
            ScalarUDF::new_from_impl(BoundedImageExtractionUdf::new("/my_images", 19));

        let mut df = ctx
            .read_parquet(
                "/Users/tsaucer/working/minimal_data_for_image_chipping.parquet",
                ParquetReadOptions::default(),
            )
            .await?;

        df = df.select(vec![extraction_udf
            .call(vec![
                col("/segmentation/detections/things:ClassId"),
                col("/segmentation/detections/things:Position2D"),
                col("/segmentation/detections/things:HalfSize2D"),
            ])
            .alias("image_archetype")])?;

        println!("{}", df.schema());
        println!(
            "{}",
            df.filter(col("image_archetype").is_not_null())?
                .count()
                .await?
        );

        Ok(())
    }
}
// INPUTS

// /segmentation/detections/things:ClassId: list<item: uint16>
// child 0, item: uint16
// -- field metadata --
// rerun.component: 'rerun.components.ClassId'
// rerun.archetype_field: 'class_ids'
// rerun.archetype: 'rerun.archetypes.Boxes2D'
// rerun.kind: 'data'
// rerun.entity_path: '/segmentation/detections/things'

// /segmentation/detections/things:Position2D: list<item: fixed_size_list<item: float not null>[2]>
// child 0, item: fixed_size_list<item: float not null>[2]
// child 0, item: float not null
// -- field metadata --
// rerun.archetype_field: 'centers'
// rerun.entity_path: '/segmentation/detections/things'
// rerun.component: 'rerun.components.Position2D'
// rerun.kind: 'data'
// rerun.archetype: 'rerun.archetypes.Boxes2D'

// /segmentation/detections/things:HalfSize2D: list<item: fixed_size_list<item: float not null>[2]>
// child 0, item: fixed_size_list<item: float not null>[2]
// child 0, item: float not null
// -- field metadata --
// rerun.component: 'rerun.components.HalfSize2D'
// rerun.archetype_field: 'half_sizes'
// rerun.archetype: 'rerun.archetypes.Boxes2D'
// rerun.entity_path: '/segmentation/detections/things'
// rerun.kind: 'data'
//
//

// OUTPUTS

// /my_image:ImageIndicator: list<item: null>
// child 0, item: null
// -- field metadata --
// rerun.component: 'rerun.components.ImageIndicator'
// rerun.entity_path: '/my_image'
// rerun.kind: 'data'
// rerun.is_indicator: 'true'

// /my_image:ImageBuffer: list<item: list<item: uint8 not null>>
// child 0, item: list<item: uint8 not null>
// child 0, item: uint8 not null
// -- field metadata --
// rerun.component: 'rerun.components.ImageBuffer'
// rerun.archetype_field: 'buffer'
// rerun.kind: 'data'
// rerun.archetype: 'rerun.archetypes.Image'
// rerun.entity_path: '/my_image'

// /my_image:ImageFormat: list<item: struct<width: uint32 not null, height: uint32 not null, pixel_format: uint8, color_model: uint8, channel_datatype: uint8>>
// child 0, item: struct<width: uint32 not null, height: uint32 not null, pixel_format: uint8, color_model: uint8, channel_datatype: uint8>
// child 0, width: uint32 not null
// child 1, height: uint32 not null
// child 2, pixel_format: uint8
// child 3, color_model: uint8
// child 4, channel_datatype: uint8
// -- field metadata --
// rerun.archetype_field: 'format'
// rerun.entity_path: '/my_image'
// rerun.component: 'rerun.components.ImageFormat'
// rerun.kind: 'data'
// rerun.archetype: 'rerun.archetypes.Image'
