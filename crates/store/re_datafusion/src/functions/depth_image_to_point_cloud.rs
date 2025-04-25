use arrow::array::ListArray;
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field, Fields};
use arrow_array::builder::{GenericListBuilder, NullBuilder};
use arrow_array::{Array, ArrayRef, StructArray};
use datafusion::common::{
    exec_datafusion_err, exec_err, DataFusionError, Result as DataFusionResult,
};
use datafusion::logical_expr::{
    ColumnarValue, ReturnFieldArgs, ScalarFunctionArgs, ScalarUDFImpl, Signature, TypeSignature,
    Volatility,
};
use itertools::multizip;
use re_types::components::ImageFormat;
use re_types::components::{DepthMeter, ImageBuffer, PinholeProjection, Position3D, Resolution};
use re_types::datatypes::{ChannelDatatype, ColorModel};
use re_types_core::{ComponentBatch, Loggable as _};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct DepthImageToPointCloudUdf {
    output_entity_path: String,
    signature: Signature,
}

impl DepthImageToPointCloudUdf {
    pub fn new(output_entity_path: impl Into<String>) -> Self {
        Self {
            output_entity_path: output_entity_path.into(),
            signature: Signature::new(
                TypeSignature::Exact(create_input_datatypes()),
                Volatility::Immutable,
            ),
        }
    }
}

fn create_input_datatypes() -> Vec<DataType> {
    vec![
        // Image Buffer
        DataType::new_list(DataType::new_list(DataType::UInt8, false), true),
        // Image Format
        DataType::new_list(format_field_data_type(), true),
        // Depth Meter
        DataType::new_list(DataType::Float32, true),
        // Pinhole Projection
        DataType::new_list(
            DataType::new_fixed_size_list(DataType::Float32, 9, false),
            true,
        ),
        // Resolution
        DataType::new_list(
            DataType::new_fixed_size_list(DataType::Float32, 2, false),
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
    let points3d_indicator_field = Field::new(
        format!("{output_entity_path}:Points3DIndicator"),
        DataType::new_list(DataType::Null, true),
        true,
    )
    .with_metadata(create_rerun_metadata(
        output_entity_path,
        "Points3DIndicator",
        None,
        None,
        "data",
        true,
    ));

    let position_field = Field::new(
        format!("{output_entity_path}:Position3D"),
        DataType::new_list(Position3D::arrow_datatype(), true),
        true,
    )
    .with_metadata(create_rerun_metadata(
        output_entity_path,
        "Position3D",
        Some("Points3D"),
        Some("position"),
        "data",
        false,
    ));

    vec![points3d_indicator_field, position_field]
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

fn columnar_value_to_array_of_array<'a>(
    columnar: &'a ColumnarValue,
    name: &str,
) -> DataFusionResult<&'a ListArray> {
    let ColumnarValue::Array(array_ref) = columnar else {
        exec_err!("Unexpect scalar columnar value for {name}")?
    };
    array_ref
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or(exec_datafusion_err!("Incorrect array type for {name}"))
}

fn compute_points(
    image_buffer: ImageBuffer,
    image_format: ImageFormat,
    depth_meter: DepthMeter,
    pinhole_projection: PinholeProjection,
    _resolution: Resolution,
) -> DataFusionResult<Vec<Position3D>> {
    if image_format.color_model() != ColorModel::L {
        exec_err!("Unsupported color model for depth image")?;
    }

    let image_f32: Vec<f32> = if image_format.datatype() == ChannelDatatype::U8 {
        image_buffer.iter().map(|f| *f as f32).collect()
    } else if image_format.datatype() == ChannelDatatype::U16 {
        let element_size = size_of::<u16>();
        let num_elements = image_buffer.len() / element_size;
        let num_bytes = num_elements * element_size;
        let bytes = &image_buffer[..num_bytes];
        let u16: &[u16] =
            bytemuck::try_cast_slice(bytes).map_err(|err| exec_datafusion_err!("{err}"))?;

        u16.iter().map(|f| *f as f32).collect()
    } else {
        exec_err!("Unsupported data type for depth image")?
    };

    let depth = depth_meter.0 .0;
    let points: Vec<Position3D> = (0..image_format.width)
        .flat_map(|x| (0..image_format.height).map(move |y| (x, y)))
        .map(|(x, y)| {
            let idx = y * image_format.width + x;

            let image_space = glam::Vec3::new(x as f32, y as f32, image_f32[idx as usize] / depth);
            pinhole_projection.unproject(image_space).into()
        })
        .collect::<Vec<_>>();

    Ok(points)
}

fn compute_points_per_entry(
    image_buffer: Vec<Option<ImageBuffer>>,
    image_format: Vec<Option<ImageFormat>>,
    depth_meter: Vec<Option<DepthMeter>>,
    pinhole_projection: Vec<Option<PinholeProjection>>,
    resolution: Vec<Option<Resolution>>,
) -> DataFusionResult<Vec<Position3D>> {
    let points_per_entry = multizip((
        image_buffer.into_iter(),
        image_format.into_iter(),
        depth_meter.into_iter(),
        pinhole_projection.into_iter(),
        resolution.into_iter(),
    ))
    .map(|entry| {
        if let (
            Some(image_buffer),
            Some(image_format),
            Some(depth_meter),
            Some(pinhole_projection),
            Some(resolution),
        ) = entry
        {
            compute_points(
                image_buffer,
                image_format,
                depth_meter,
                pinhole_projection,
                resolution,
            )
        } else {
            Ok(Vec::default())
        }
    })
    .collect::<DataFusionResult<Vec<_>>>()?;

    // Since we had one image per entry and it can create X number of points,
    // we need to flatten these
    let points = points_per_entry.into_iter().flatten().collect::<Vec<_>>();

    Ok(points)
}

impl ScalarUDFImpl for DepthImageToPointCloudUdf {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        "depth_image_to_point_cloud"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _arg_types: &[DataType]) -> DataFusionResult<DataType> {
        exec_err!("use return_field_from_args instead")
    }

    fn return_field_from_args(&self, _args: ReturnFieldArgs<'_>) -> DataFusionResult<Field> {
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

        let image_buffer_arr = columnar_value_to_array_of_array(&args.args[0], "ImageBuffer")?;
        let image_format_arr = columnar_value_to_array_of_array(&args.args[1], "ImageFormat")?;
        let depth_meter_arr = columnar_value_to_array_of_array(&args.args[2], "DepthMeter")?;
        let pinhole_arr = columnar_value_to_array_of_array(&args.args[3], "PinholeProjection")?;
        let resolution_arr = columnar_value_to_array_of_array(&args.args[4], "Resolution")?;

        let result = multizip((
            image_buffer_arr.iter(),
            image_format_arr.iter(),
            depth_meter_arr.iter(),
            pinhole_arr.iter(),
            resolution_arr.iter(),
        ))
        .map(|entry| {
            let (
                Some(image_buffer),
                Some(image_format),
                Some(depth_meter),
                Some(pinhole_projection),
                Some(resolution),
            ) = entry
            else {
                return Ok(None);
            };

            let image_buffer = ImageBuffer::from_arrow_opt(&image_buffer)
                .map_err(|err| exec_datafusion_err!("{err}"))?;
            let image_format = ImageFormat::from_arrow_opt(&image_format)
                .map_err(|err| exec_datafusion_err!("{err}"))?;
            let depth_meter = DepthMeter::from_arrow_opt(&depth_meter)
                .map_err(|err| exec_datafusion_err!("{err}"))?;
            let pinhole_projection = PinholeProjection::from_arrow_opt(&pinhole_projection)
                .map_err(|err| exec_datafusion_err!("{err}"))?;
            let resolution = Resolution::from_arrow_opt(&resolution)
                .map_err(|err| exec_datafusion_err!("{err}"))?;

            let num_entries = image_buffer.len();
            if image_format.len() != num_entries
                || depth_meter.len() != num_entries
                || pinhole_projection.len() != num_entries
                || resolution.len() != num_entries
            {
                exec_err!("Expected all components to have identical length arrays")?;
            }

            let points_per_entry = compute_points_per_entry(
                image_buffer,
                image_format,
                depth_meter,
                pinhole_projection,
                resolution,
            )?;

            let points_batch = &points_per_entry as &dyn ComponentBatch;

            let points_arr = points_batch
                .to_arrow()
                .map_err(|err| exec_datafusion_err!("{err}"))?;
            Ok(Some(points_arr))

            // Ok(Some(points_per_entry))
        })
        .collect::<DataFusionResult<Vec<_>>>()?;

        // let result_arr = ListArray::try_from(result.into_iter()).map_err(|err| exec_datafusion_err!("{err}"))?;
        // to_arrow_list_array

        // let array = self.to_arrow()?;
        // let offsets =
        //     arrow::buffer::OffsetBuffer::from_lengths(std::iter::repeat(1).take(array.len()));
        // let nullable = true;
        // let field = arrow::datatypes::Field::new("item", array.data_type().clone(), nullable);
        // ArrowListArray::try_new(field.into(), offsets, array, None).map_err(|err| err.into())

        // let results_arr = Position3D::to_arrow_list_array(result);

        let fields = create_output_fields(self.output_entity_path.as_str());

        let mut offsets = Vec::with_capacity(result.len() + 1);
        let mut valid_arrays: Vec<&dyn Array> = Vec::new();
        let mut validity = Vec::with_capacity(result.len());

        offsets.push(0);
        let mut cumulative_length = 0;

        for opt_array in &result {
            match opt_array {
                Some(array) => {
                    // This element is valid
                    validity.push(true);
                    valid_arrays.push(array);
                    cumulative_length += array.len() as i32;
                }
                None => {
                    // This element is null
                    validity.push(false);
                }
            }
            offsets.push(cumulative_length);
        }

        // Create offset buffer
        let offset_buffer = OffsetBuffer::new(offsets.into());

        // Concatenate all the valid arrays
        let values = if valid_arrays.is_empty() {
            arrow::array::new_empty_array(&Position3D::arrow_datatype())
        } else {
            re_arrow_util::concat_arrays(&valid_arrays)?
        };

        // let lengths = result
        //     .iter()
        //     .map(|maybe_row| maybe_row.as_ref().map(|row| row.len()))
        //     .collect::<Vec<_>>();

        // let offsets = OffsetBuffer::from_lengths(lengths.iter().map(|v| v.unwrap_or(0)));
        // let values = result
        //     .into_iter()
        //     .flatten()
        //     .flatten()
        //     .collect::<Vec<_>>()
        //     .to_arrow()
        //     .map_err(|err| exec_datafusion_err!("{err}"))?;

        // let validity: Vec<bool> = lengths.iter().map(|v| v.is_some()).collect();

        // Points3D Indicator
        let mut indicator_array_builder: GenericListBuilder<i32, NullBuilder> =
            GenericListBuilder::with_capacity(NullBuilder::new(), 0);
        for is_valid in &validity {
            indicator_array_builder.append(*is_valid);
        }
        let indicator_array = Arc::new(indicator_array_builder.finish()) as ArrayRef;

        let list_field = Arc::new(Field::new("item", values.data_type().clone(), true));
        let result_array = Arc::new(ListArray::try_new(
            list_field,
            offset_buffer,
            values,
            Some(validity.into()),
        )?);

        StructArray::try_new(fields.into(), vec![indicator_array, result_array], None)
            .map(|arr| ColumnarValue::Array(Arc::new(arr)))
            .map_err(DataFusionError::from)
    }
}

#[cfg(test)]
mod tests {
    use crate::functions::depth_image_to_point_cloud::DepthImageToPointCloudUdf;
    use datafusion::error::DataFusionError;
    use datafusion::logical_expr::ScalarUDF;
    use datafusion::prelude::{col, ParquetReadOptions, SessionContext};

    #[tokio::test]
    async fn test_convert_to_point_cloud() -> Result<(), DataFusionError> {
        let ctx = SessionContext::default();

        let to_point_cloud_udf =
            ScalarUDF::new_from_impl(DepthImageToPointCloudUdf::new("/output_point_cloud"));

        let input_entity_path = "/world/camera_lowres";

        let mut df = ctx
            .read_parquet(
                "/Users/tsaucer/working/arkit_demo_data_depth_image.parquet",
                ParquetReadOptions::default(),
            )
            .await?;

        df = df.select(vec![to_point_cloud_udf
            .call(vec![
                col(format!("{input_entity_path}/depth:ImageBuffer")),
                col(format!("{input_entity_path}/depth:ImageFormat")),
                col(format!("{input_entity_path}/depth:DepthMeter")),
                col(format!("{input_entity_path}:PinholeProjection")),
                col(format!("{input_entity_path}:Resolution")),
            ])
            .alias("point_cloud_archetype")])?;

        let df_cached = df.cache().await?;

        println!(
            "Number of point cloud entries: {}",
            df_cached
                .filter(col("point_cloud_archetype").is_not_null())?
                .count()
                .await?
        );

        Ok(())
    }
}
