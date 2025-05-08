use crate::functions::utils::{
    columnar_value_to_array_of_array, concatenate_list_of_component_arrays, create_rerun_metadata,
};
use arrow::array::{
    ArrayRef, FixedSizeListArray, Float64Array, Int64Array, ListArray, StringArray, StructArray,
    UInt8Array,
};
use arrow::datatypes::{DataType, Field};
use datafusion::common::{exec_datafusion_err, exec_err, Result as DataFusionResult};
use datafusion::error::DataFusionError;
use datafusion::logical_expr::{
    ColumnarValue, ReturnFieldArgs, ScalarFunctionArgs, ScalarUDFImpl, Signature, TypeSignature,
    Volatility,
};
use itertools::multizip;
use log::warn;
use re_types::components::ImageBuffer;
use re_types::components::ImageFormat;
use re_types_core::Loggable as _;
use re_video::decode::FrameContent;
use re_video::Frame;
use std::any::Any;
use std::fmt::Debug;
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

pub struct BoundedImageExtractionUdf {
    signature: Signature,
    decoded_blob: Arc<Mutex<Option<DecodedBlob>>>,
}

impl Debug for BoundedImageExtractionUdf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BoundedImageExtractionUdf()")
    }
}

struct DecodedBlob {
    hash: u64,
    frames: Vec<Frame>,
}

impl Default for BoundedImageExtractionUdf {
    fn default() -> Self {
        Self {
            signature: Signature::new(
                TypeSignature::Exact(create_input_datatypes()),
                Volatility::Immutable,
            ),
            decoded_blob: Arc::new(Mutex::new(None)),
        }
    }
}

fn create_input_datatypes() -> Vec<DataType> {
    vec![
        // Frame ID
        DataType::Int64,
        // Position2D
        DataType::new_list(
            DataType::new_fixed_size_list(DataType::Float64, 2, false),
            true,
        ),
        // HalfSize2D
        DataType::new_list(
            DataType::new_fixed_size_list(DataType::Float64, 2, false),
            true,
        ),
        // Video Blob
        DataType::new_list(DataType::new_list(DataType::UInt8, false), true),
        // MediaType
        DataType::new_list(DataType::Utf8, true),
    ]
}

fn create_output_fields() -> Vec<Field> {
    let image_buffer_field = Field::new(
        "ImageBuffer",
        DataType::new_list(ImageBuffer::arrow_datatype(), true),
        true,
    )
    .with_metadata(create_rerun_metadata(
        None,
        "ImageBuffer",
        Some("Image"),
        Some("buffer"),
        "data",
        false,
    ));

    let image_format_field = Field::new(
        "ImageFormat",
        DataType::new_list(ImageFormat::arrow_datatype(), true),
        true,
    )
    .with_metadata(create_rerun_metadata(
        None,
        "ImageFormat",
        Some("Image"),
        Some("format"),
        "data",
        false,
    ));

    vec![image_buffer_field, image_format_field]
}

impl BoundedImageExtractionUdf {
    fn blob_to_frames(&self, blob: &UInt8Array) -> DataFusionResult<()> {
        let buffer = blob.values().inner().as_ref();

        let mut decoded_blob = self
            .decoded_blob
            .lock()
            .map_err(|err| exec_datafusion_err!("{err}"))?;

        let mut blob_hasher = std::hash::DefaultHasher::new();
        blob_hasher.write(&buffer);
        let blob_hash = blob_hasher.finish();

        if let Some(decoded_blob) = decoded_blob.as_ref() {
            if decoded_blob.hash == blob_hash {
                return Ok(());
            }
        }

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
        let result_frames = frames.drain(..).filter_map(|f| f.ok()).collect::<Vec<_>>();

        *decoded_blob = Some(DecodedBlob {
            hash: blob_hash,
            frames: result_frames,
        });

        Ok(())
    }
}

fn create_image_chips(
    centers: &FixedSizeListArray,
    half_sizes: &FixedSizeListArray,
    frame: &Frame,
) -> Option<Vec<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>>> {
    let chips = centers
        .iter()
        .zip(half_sizes.iter())
        .filter_map(|tuple| match tuple {
            (Some(center), Some(half_size)) => Some((center, half_size)),
            _ => None,
        })
        .filter_map(|(center, half_size)| {
            let center = center.as_any().downcast_ref::<Float64Array>()?;
            let half_size = half_size.as_any().downcast_ref::<Float64Array>()?;

            let min_x = center.value(0) - half_size.value(0);
            let min_y = center.value(1) - half_size.value(1);
            let max_x = center.value(0) + half_size.value(0);
            let max_y = center.value(1) + half_size.value(1);

            if min_x > frame.content.width as f64
                || min_y > frame.content.height as f64
                || max_x < 0.
                || max_y < 0.
            {
                return None;
            }

            let min_x = (min_x as usize).max(0);
            let min_y = (min_y as usize).max(0);
            let max_x = (max_x as usize).min(frame.content.width as usize);
            let max_y = (max_y as usize).min(frame.content.height as usize);

            Some(extract_rgb_image(
                [min_x, min_y, max_x, max_y],
                &frame.content,
            ))
        })
        .collect::<Vec<_>>();

    match chips.is_empty() {
        true => None,
        false => Some(chips),
    }
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

fn image_buffers_to_components(
    image_buffers: Vec<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>>,
) -> DataFusionResult<(ArrayRef, ArrayRef)> {
    let (buffers, formats): (Vec<Option<ImageBuffer>>, Vec<Option<ImageFormat>>) = image_buffers
        .into_iter()
        .map(ImageBuffer::from_image)
        .map(|maybe_buffer| match maybe_buffer {
            Ok(buffer) => Some(buffer),
            Err(err) => {
                // We don't want the whole UDF to fail, but we do need to let the user
                // know that their data did not process.
                warn!("image buffer conversion error: {}", err);
                None
            }
        })
        .map(|maybe_tuple| {
            maybe_tuple
                .map(|(b, f)| (Some(b), Some(f)))
                .unwrap_or((None, None))
        })
        .collect();

    let buffer_array =
        ImageBuffer::to_arrow_opt(buffers).map_err(|err| exec_datafusion_err!("{err}"))?;
    let format_array =
        ImageFormat::to_arrow_opt(formats).map_err(|err| exec_datafusion_err!("{err}"))?;

    Ok((buffer_array, format_array))
}

fn image_buffer_rows_to_components(
    image_buffers: Vec<Option<Vec<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>>>>,
) -> DataFusionResult<(ArrayRef, ArrayRef)> {
    let (buffers, formats): (Vec<Option<ArrayRef>>, Vec<Option<ArrayRef>>) = image_buffers
        .into_iter()
        .map(|maybe_buffer| maybe_buffer.map(image_buffers_to_components).transpose())
        .collect::<DataFusionResult<Vec<_>>>()?
        .into_iter()
        .map(|maybe_arrays| {
            maybe_arrays
                .map(|(b, v)| (Some(b), Some(v)))
                .unwrap_or((None, None))
        })
        .unzip();

    let buffers = concatenate_list_of_component_arrays::<ImageBuffer>(&buffers)?;
    let formats = concatenate_list_of_component_arrays::<ImageFormat>(&formats)?;

    Ok((buffers, formats))
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
        if args.arg_fields.len() != 5 {
            exec_err!(
                "UDF expects 5 arguments for Frame ID, Position2D, HalfSize2D, Video Blob, and Video MediaType"
            )?;
        }

        Ok(Field::new(
            "Image",
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
        let ColumnarValue::Array(frame_id_arr) = &args.args[0] else {
            exec_err!("Unexpected scalar columnar value for frame ID")?
        };
        let frame_id_arr =
            frame_id_arr
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or(exec_datafusion_err!(
                    "Incorrect data type for Frame ID. Expected Int64, found {}",
                    frame_id_arr.data_type()
                ))?;

        let centers_arr = columnar_value_to_array_of_array(&args.args[1], "Position2D")?;
        let half_sizes_arr = columnar_value_to_array_of_array(&args.args[2], "HalfSize2D")?;
        let blob_arr = columnar_value_to_array_of_array(&args.args[3], "Blob")?.value(0);
        let media_type_arr = columnar_value_to_array_of_array(&args.args[4], "MediaType")?.value(0);

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

        self.blob_to_frames(&blob)?;
        let decoded_blob_lock = self
            .decoded_blob
            .lock()
            .map_err(|err| exec_datafusion_err!("{err}"))?;
        let video_frames = &decoded_blob_lock
            .as_ref()
            .ok_or(exec_datafusion_err!("Unable to retrieve video frames"))?
            .frames;

        let result = multizip((
            frame_id_arr.iter(),
            centers_arr.iter(),
            half_sizes_arr.iter(),
        ))
        .map(|entry| {
            let (Some(frame_id), Some(centers), Some(half_sizes)) = entry else {
                return Ok(None);
            };

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
                    create_image_chips(centers, half_sizes, frame)
                } else {
                    None
                }
            } else {
                None
            })
        })
        .collect::<DataFusionResult<Vec<_>>>()?;

        let (buffer_array, format_array) = image_buffer_rows_to_components(result)?;

        let fields = create_output_fields().into();

        StructArray::try_new(fields, vec![buffer_array, format_array], None)
            .map(|arr| ColumnarValue::Array(Arc::new(arr)))
            .map_err(DataFusionError::from)
    }
}
