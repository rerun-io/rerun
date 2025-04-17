use arrow::array::{
    ArrayRef, FixedSizeListArray, Float64Array, GenericListBuilder, Int64Array, ListArray,
    NullBuilder, StringArray, StructArray, UInt16Array, UInt8Array,
};
use arrow::buffer::{NullBuffer, OffsetBuffer};
use arrow::datatypes::{DataType, Field, Fields};
use datafusion::common::{exec_datafusion_err, exec_err, Result as DataFusionResult};
use datafusion::error::DataFusionError;
use datafusion::logical_expr::{ColumnarValue, ReturnFieldArgs, ScalarFunctionArgs, ScalarUDFImpl, Signature, TypeSignature, Volatility};
use itertools::multizip;
use re_dataframe::external::re_chunk::ArrowArray as _;
use re_types::components::ImageBuffer;
use re_types::components::ImageFormat;
use re_types_core::Loggable as _;
use re_video::decode::FrameContent;
use re_video::Frame;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

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
        // Frame ID
        DataType::Int64,
        // ClassId
        DataType::new_list(DataType::UInt16, true),
        // Position2D
        DataType::new_list(
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float64, false)), 2),
            true,
        ),
        // HalfSize2D
        DataType::new_list(
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float64, false)), 2),
            true,
        ),
        // Video Blob
        DataType::new_list(DataType::new_list(DataType::UInt8, false), true),
        // MediaType
        DataType::new_list(DataType::Utf8, true),
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
        Some("format"),
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
) -> Option<[usize; 4]> {
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
                println!("wrong type {}", half_sizes.data_type());
                return 0;
            };
            // Since we can't find max of a float, this is good enough for what we're doing here
            (area_array.value(0) * area_array.value(1) * 100.0) as u64
        })?;

    let best_center = result.0.as_any().downcast_ref::<Float64Array>()?;
    let best_half_size = result.1.as_any().downcast_ref::<Float64Array>()?;
    Some([
        (best_center.value(0) - best_half_size.value(0)) as usize,
        (best_center.value(1) - best_half_size.value(1)) as usize,
        (best_center.value(0) + best_half_size.value(0)) as usize,
        (best_center.value(1) + best_half_size.value(1)) as usize,
    ])
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

fn blob_to_frames(blob: &UInt8Array) -> DataFusionResult<Vec<Frame>> {
    let buffer = blob.values().inner().as_ref();

    let video = re_video::VideoData::load_mp4(buffer)
        .map_err(|err| exec_datafusion_err!("Error loading mp4 from blob: {err}"))?;

    let frames = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let on_output = {
        let frames = frames.clone();
        move |frame| {
            frames.lock().push(frame);
        }
    };

    let mut decoder = re_video::decode::new_decoder(
        "bounded_image_extraction_udf",
        &video,
        &re_video::decode::DecodeSettings::default(),
        on_output,
    )
    .map_err(|err| exec_datafusion_err!("Error when creating decoder: {err}"))?;

    for sample in &video.samples {
        let chunk = sample
            .get(buffer)
            .ok_or(exec_datafusion_err!("Unable to get chunk to decode"))?;
        decoder
            .submit_chunk(chunk)
            .map_err(|err| exec_datafusion_err!("Failed to submit chunk: {err}"))?;
    }
    decoder
        .end_of_video()
        .map_err(|err| exec_datafusion_err!("Failed to finish video: {err}"))?;

    let mut has_all_samples = false;
    while !has_all_samples {
        let frames = frames.lock();
        has_all_samples = frames.len() >= video.samples.len();
        if !has_all_samples {
            sleep(Duration::from_millis(100));
        }
    }

    let mut frames = frames.lock();

    // This will silently discard any frames that ffmpeg could not decode, so maybe not ideal
    // but also probably shouldn't stop the whole UDF
    Ok(frames.drain(..).filter_map(|f| f.ok()).collect())
}

fn create_image_chip(
    class_of_interest: u16,
    class_ids: &UInt16Array,
    centers: &FixedSizeListArray,
    half_sizes: &FixedSizeListArray,
    frame: &Frame,
) -> Option<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
    let detection = find_largest_detection(class_of_interest, class_ids, centers, half_sizes)?;

    if detection[2] > frame.content.width as usize || detection[3] > frame.content.height as usize {
        return None;
    }

    let width = detection[2] - detection[0];
    let height = detection[3] - detection[1];
    let mut data = Vec::with_capacity(width * height);
    for i in detection[0]..detection[2] {
        for j in detection[1]..detection[3] {
            let val = frame.content.data.get(i + (j * width))?;
            data.push(*val);
        }
    }

    Some(extract_rgb_image(detection, &frame.content))
}

fn extract_rgb_image(
    detection: [usize; 4],
    frame_content: &FrameContent,
) -> image::ImageBuffer<image::Rgb<u8>, Vec<u8>> {
    let width = detection[2] - detection[0];
    let height = detection[3] - detection[1];

    // Create a new RGB image buffer
    let mut img = image::ImageBuffer::new(width as u32, height as u32);

    let image_area = frame_content.width as usize * frame_content.height as usize;
    let u_start = image_area;
    let v_start = u_start + image_area / 4;

    let stride = frame_content.width as usize;
    let y_data = &frame_content.data[0..u_start];
    let u_data = &frame_content.data[u_start..v_start];
    let v_data = &frame_content.data[v_start..];

    for y in 0..height {
        for x in 0..width {
            let image_x = x + detection[0];
            let image_y = y + detection[1];
            let y_index = image_y * stride + image_x;

            // For YUV420, U and V have half the resolution of Y
            // Adjust indices according to your specific YUV format
            let uv_index = (image_y / 2) * (stride / 2) + (image_x / 2);

            let y_value = y_data[y_index] as f32;
            let u_value = u_data[uv_index] as f32 - 128.0;
            let v_value = v_data[uv_index] as f32 - 128.0;

            // YUV to RGB conversion
            let r = y_value + 1.5748 * v_value;
            let g = y_value - 0.1873 * u_value - 0.4681 * v_value;
            let b = y_value + 1.8556 * u_value;

            // Clamp values to 0-255 range and convert to u8
            let r = r.clamp(0.0, 255.0) as u8;
            let g = g.clamp(0.0, 255.0) as u8;
            let b = b.clamp(0.0, 255.0) as u8;

            img.put_pixel(x as u32, y as u32, image::Rgb([r, g, b]));
        }
    }

    img
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
        exec_err!("use return_field_from_args instead")
    }
    fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> DataFusionResult<Field> {
        if args.arg_fields.len() != 6 {
            exec_err!(
                "UDF expects 6 arguments for Frame ID, ClassID, Position2D, HalfSize2D, Video Blob, and Video MediaType"
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
        let ColumnarValue::Array(frame_id_arr) = &args.args[0] else {
            exec_err!("Unexpect scalar columnar value for frame ID")?
        };
        let frame_id_arr =
            frame_id_arr
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or(exec_datafusion_err!(
                    "Incorrect data type for Frame ID. Expected Int64, found {}",
                    frame_id_arr.data_type()
                ))?;

        let class_ids_arr = columnar_value_to_array_of_array(&args.args[1], "ClassID")?;
        let centers_arr = columnar_value_to_array_of_array(&args.args[2], "Position2D")?;
        let half_sizes_arr = columnar_value_to_array_of_array(&args.args[3], "HalfSize2D")?;
        let blob_arr = columnar_value_to_array_of_array(&args.args[4], "Blob")?.value(0);
        let media_type_arr = columnar_value_to_array_of_array(&args.args[5], "MediaType")?.value(0);

        let media_type = media_type_arr
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(exec_datafusion_err!(
                "Unable to cast string array {}",
                media_type_arr.data_type()
            ))?
            .value(0);

        if media_type != "video/mp4" {
            exec_err!("Unsupported media type: {media_type}")?;
        }

        let video_blob_arr = blob_arr
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or(exec_datafusion_err!(
                "Unable to cast blob array from {}",
                blob_arr.data_type()
            ))?
            .value(0);

        let blob = video_blob_arr
            .as_any()
            .downcast_ref::<UInt8Array>()
            .ok_or(exec_datafusion_err!("Trying to get lower level blob"))?;

        let video_frames = blob_to_frames(blob)?;

        let result = multizip((
            frame_id_arr.iter(),
            class_ids_arr.iter(),
            centers_arr.iter(),
            half_sizes_arr.iter(),
        ))
        .map(|entry| {
            let (Some(frame_id), Some(class_ids), Some(centers), Some(half_sizes)) = entry else {
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
            Ok(if frame_id > 0 {
                if let Some(frame) = video_frames
                    .iter()
                    .find(|frame| frame.info.frame_nr == Some(frame_id as usize))
                {
                    create_image_chip(
                        self.class_of_interest,
                        class_ids,
                        centers,
                        half_sizes,
                        frame,
                    )
                } else {
                    None
                }
            } else {
                None
            })

            // Ok(find_largest_detection(
            //     self.class_of_interest,
            //     class_ids,
            //     centers,
            //     half_sizes,
            // ))
        })
        .collect::<DataFusionResult<Vec<_>>>()?;

        let fields = create_output_fields(self.output_entity_path.as_str());

        let (buffer, format): (Vec<Option<ImageBuffer>>, Vec<Option<ImageFormat>>) = result
            .into_iter()
            .map(|maybe_image| maybe_image.and_then(|image| ImageBuffer::from_image(image).ok()))
            .map(|buffer_and_format| match buffer_and_format {
                Some((buffer, format)) => (Some(buffer), Some(format)),
                None => (None, None),
            })
            .unzip();

        let validity = buffer.iter().map(|b| b.is_some()).collect::<Vec<_>>();

        let mut image_indicator_array_builder: GenericListBuilder<
            i32,
            NullBuilder,
        > = GenericListBuilder::with_capacity(NullBuilder::new(), 0);
        for is_valid in &validity {
            // image_indicator_array_builder.values().append(true);
            image_indicator_array_builder.append(*is_valid);
        }
        let image_indicator_array = Arc::new(image_indicator_array_builder.finish()) as ArrayRef;

        // let lengths = nulls
        //     .iter()
        //     .map(|b| match b {
        //         true => 1,
        //         false => 0,
        //     })
        //     .collect::<Vec<_>>();
        let lengths = vec![1; validity.len()];
        let null_buffer = NullBuffer::from(validity);
        let buffer_array_inner =
            ImageBuffer::to_arrow_opt(buffer).map_err(|err| exec_datafusion_err!("{err}"))?;

        let buffer_array = Arc::new(ListArray::try_new(
            Arc::new(Field::new_list_field(
                DataType::List(Arc::new(Field::new_list_field(DataType::UInt8, false))),
                true,
            )),
            OffsetBuffer::<i32>::from_lengths(lengths.clone()),
            buffer_array_inner,
            Some(null_buffer.clone()),
        )?) as ArrayRef;

        let format_array_inner =
            ImageFormat::to_arrow_opt(format).map_err(|err| exec_datafusion_err!("{err}"))?;
        // let lengths = vec![1; format_array_inner.len()];
        let format_array = Arc::new(ListArray::try_new(
            Arc::new(Field::new_list_field(format_field_data_type(), true)),
            OffsetBuffer::<i32>::from_lengths(lengths),
            format_array_inner,
            Some(null_buffer.clone()),
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
                col("frame"),
                col("/segmentation/detections/things:ClassId"),
                col("/segmentation/detections/things:Position2D"),
                col("/segmentation/detections/things:HalfSize2D"),
                col("/video:Blob"),
                col("/video:MediaType"),
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
