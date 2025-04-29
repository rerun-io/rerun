use crate::functions::utils::{
    columnar_value_to_array_of_array, concatenate_list_of_component_arrays, create_indicator_array,
    create_rerun_metadata,
};
use arrow::datatypes::{DataType, Field};
use arrow_array::StructArray;
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
use std::sync::Arc;

#[derive(Debug)]
pub struct DepthImageToPointCloudUdf {
    signature: Signature,
}

impl Default for DepthImageToPointCloudUdf {
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
        // Image Buffer
        DataType::new_list(ImageBuffer::arrow_datatype(), true),
        // Image Format
        DataType::new_list(ImageFormat::arrow_datatype(), true),
        // Depth Meter
        DataType::new_list(DepthMeter::arrow_datatype(), true),
        // Pinhole Projection
        DataType::new_list(PinholeProjection::arrow_datatype(), true),
        // Resolution
        DataType::new_list(Resolution::arrow_datatype(), true),
    ]
}

fn create_output_fields() -> Vec<Field> {
    let points3d_indicator_field = Field::new(
        "Points3DIndicator",
        DataType::new_list(DataType::Null, true),
        true,
    )
    .with_metadata(create_rerun_metadata(
        None,
        "Points3DIndicator",
        None,
        None,
        "data",
        true,
    ));

    let position_field = Field::new(
        "Position3D",
        DataType::new_list(Position3D::arrow_datatype(), true),
        true,
    )
    .with_metadata(create_rerun_metadata(
        None,
        "Position3D",
        Some("Points3D"),
        Some("position"),
        "data",
        false,
    ));

    vec![points3d_indicator_field, position_field]
}

fn compute_points(
    image_buffer: &ImageBuffer,
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
                &image_buffer,
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
            "depth_image_to_point_cloud",
            DataType::Struct(create_output_fields().into()),
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

        let validity = result.iter().map(|v| v.is_some()).collect::<Vec<_>>();
        let indicator_array = create_indicator_array(&validity);
        let result_array = concatenate_list_of_component_arrays::<Position3D>(&result)?;

        let fields = create_output_fields().into();
        StructArray::try_new(fields, vec![indicator_array, result_array], None)
            .map(|arr| ColumnarValue::Array(Arc::new(arr)))
            .map_err(DataFusionError::from)
    }
}
