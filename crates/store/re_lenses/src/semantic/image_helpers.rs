//! Helper functions for converting raw image data to Rerun image components.

use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, ListArray, StringArray, StructArray, UInt8Array, UInt32Array,
};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field};
use itertools::Itertools as _;
use re_lenses_core::combinators::{Error, try_downcast};
use re_sdk_types::ToArrow as _;
use re_sdk_types::encodings::{ChannelDatatype, ColorModel, ImageFormat, PixelFormat};
use strum::VariantNames as _;

use crate::semantic::helpers::{get_blob_field_as_binary, get_field_as};

const ENCODING_FIELD: &str = "encoding";

pub(crate) fn extract_blob_data(source: &StructArray) -> Result<Option<ArrayRef>, Error> {
    let data = source
        .column_by_name("data")
        .ok_or_else(|| Error::FieldNotFound {
            field_name: "data".to_owned(),
            available_fields: source.fields().iter().map(|f| f.name().clone()).collect(),
        })?
        .clone();
    crate::op::binary_to_list_uint8()(&data)
}

/// The supported raw-image encodings shared by the ROS and Foxglove conversion paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::EnumString, strum::VariantNames)]
#[strum(serialize_all = "lowercase")]
enum ImageEncoding {
    Rgb8,
    Rgba8,
    Rgb16,
    Rgba16,
    Bgr8,
    Bgra8,
    Bgr16,
    Bgra16,
    Mono8,
    Mono16,
    Yuyv,
    #[strum(serialize = "yuv422_yuy2")]
    Yuv422Yuy2,
    Nv12,
    #[strum(serialize = "8UC1")]
    Cv8UC1,
    #[strum(serialize = "8UC3")]
    Cv8UC3,
    #[strum(serialize = "8SC1")]
    Cv8SC1,
    #[strum(serialize = "16UC1")]
    Cv16UC1,
    #[strum(serialize = "16SC1")]
    Cv16SC1,
    #[strum(serialize = "32SC1")]
    Cv32SC1,
    #[strum(serialize = "32FC1")]
    Cv32FC1,
    #[strum(serialize = "64FC1")]
    Cv64FC1,
}

impl ImageEncoding {
    fn is_single_channel(self) -> bool {
        matches!(
            self,
            Self::Cv8UC1
                | Self::Cv8SC1
                | Self::Cv16UC1
                | Self::Cv16SC1
                | Self::Cv32SC1
                | Self::Cv32FC1
                | Self::Cv64FC1
                | Self::Mono8
                | Self::Mono16
        )
    }

    fn to_image_format(self, dimensions: [u32; 2]) -> ImageFormat {
        match self {
            Self::Rgb8 => ImageFormat::rgb8(dimensions),
            Self::Rgba8 => ImageFormat::rgba8(dimensions),
            Self::Rgb16 => {
                ImageFormat::from_color_model(dimensions, ColorModel::RGB, ChannelDatatype::U16)
            }
            Self::Rgba16 => {
                ImageFormat::from_color_model(dimensions, ColorModel::RGBA, ChannelDatatype::U16)
            }
            // OpenCV uses BGR instead of RGB, so we can assume a 3-channel OpenCV image to be BGR.
            // https://opencv.org/color-spaces-in-opencv/#h-rgb-red-green-blue-color-space
            Self::Bgr8 | Self::Cv8UC3 => {
                ImageFormat::from_color_model(dimensions, ColorModel::BGR, ChannelDatatype::U8)
            }
            Self::Bgra8 => {
                ImageFormat::from_color_model(dimensions, ColorModel::BGRA, ChannelDatatype::U8)
            }
            Self::Bgr16 => {
                ImageFormat::from_color_model(dimensions, ColorModel::BGR, ChannelDatatype::U16)
            }
            Self::Bgra16 => {
                ImageFormat::from_color_model(dimensions, ColorModel::BGRA, ChannelDatatype::U16)
            }
            Self::Mono8 => {
                ImageFormat::from_color_model(dimensions, ColorModel::L, ChannelDatatype::U8)
            }
            Self::Mono16 => {
                ImageFormat::from_color_model(dimensions, ColorModel::L, ChannelDatatype::U16)
            }
            // ROS & Foxglove support both `yuyv` and `yuv422_yuy2` as format strings for [`PixelFormat::YUY2`].
            // We keep two enum variants for easier (de-)serialization.
            // https://github.com/ros2/common_interfaces/blob/rolling/sensor_msgs/include/sensor_msgs/image_encodings.hpp#L101
            // https://docs.foxglove.dev/docs/sdk/schemas/raw-image#data
            Self::Yuyv | Self::Yuv422Yuy2 => {
                ImageFormat::from_pixel_format(dimensions, PixelFormat::YUY2)
            }
            Self::Nv12 => ImageFormat::from_pixel_format(dimensions, PixelFormat::NV12),
            Self::Cv8UC1 => ImageFormat::depth(dimensions, ChannelDatatype::U8),
            Self::Cv8SC1 => ImageFormat::depth(dimensions, ChannelDatatype::I8),
            Self::Cv16UC1 => ImageFormat::depth(dimensions, ChannelDatatype::U16),
            Self::Cv16SC1 => ImageFormat::depth(dimensions, ChannelDatatype::I16),
            Self::Cv32SC1 => ImageFormat::depth(dimensions, ChannelDatatype::I32),
            Self::Cv32FC1 => ImageFormat::depth(dimensions, ChannelDatatype::F32),
            Self::Cv64FC1 => ImageFormat::depth(dimensions, ChannelDatatype::F64),
        }
    }
}

/// Returns a pipe-compatible function that converts a struct with `width`, `height`, and
/// `encoding` fields into a Rerun [`ImageFormat`] struct array.
pub(crate) fn encoding_to_image_format()
-> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let source = try_downcast::<StructArray>(source, "encoding_to_image_format input")?;

        let width_array = get_field_as::<UInt32Array>(source, "width")?;
        let height_array = get_field_as::<UInt32Array>(source, "height")?;
        let encoding_array = get_field_as::<StringArray>(source, ENCODING_FIELD)?;

        // The MCAP decoders declare `encoding` non-nullable, so this should never fire.
        // We also have defensive coding below to handle the case where it does, just in case.
        re_log::debug_assert!(
            !encoding_array.is_nullable(),
            "`{ENCODING_FIELD}` must be declared non-nullable"
        );

        let formats: Vec<ImageFormat> = (0..source.len())
            .map(|i| -> Result<_, Error> {
                if encoding_array.is_null(i) {
                    // Defensive coding (see above debug_assert).
                    return Err(Error::UnexpectedNull {
                        field_name: ENCODING_FIELD,
                        context: "encoding_to_image_format",
                    });
                }
                let encoding = parse_encoding(encoding_array.value(i))?;
                Ok(encoding.to_image_format([width_array.value(i), height_array.value(i)]))
            })
            .try_collect()?;

        let array_ref =
            ImageFormat::to_arrow(formats.iter()).map_err(|err| Error::Other(err.to_string()))?;

        Ok(Some(array_ref))
    }
}

/// Returns a pipe-compatible function that extracts image buffer data from a struct with
/// `width`, `height`, `step`, `encoding`, and `data` fields.
pub(crate) fn extract_image_buffer()
-> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        re_tracing::profile_function!();

        let source = try_downcast::<StructArray>(source, "extract_image_buffer input")?;

        let width_array = get_field_as::<UInt32Array>(source, "width")?;
        let height_array = get_field_as::<UInt32Array>(source, "height")?;
        let step_array = get_field_as::<UInt32Array>(source, "step")?;
        let encoding_array = get_field_as::<StringArray>(source, "encoding")?;
        let data_array = get_blob_field_as_binary(source, "data")?;

        let len = source.len();
        let mut buffer: Vec<u8> = Vec::new();
        let mut offsets: Vec<i32> = Vec::with_capacity(len + 1);
        offsets.push(0);

        for i in 0..len {
            if data_array.is_null(i) {
                push_offset(&buffer, &mut offsets)?;
                continue;
            }

            let height = height_array.value(i) as usize;
            let blob = data_array.value(i);

            // How many bytes Rerun expects for this encoding (e.g. 8x8 16UC1 -> 128).
            let encoding = parse_encoding(encoding_array.value(i))?;
            let total_num_bytes = encoding
                .to_image_format([width_array.value(i), height_array.value(i)])
                .num_bytes();

            // Row stride: trust `step` when set, otherwise if unset (0) fall back to deriving it from the data size.
            let step = step_array.value(i) as usize;
            let row_stride = if step > 0 {
                step
            } else {
                blob.len().checked_div(height).unwrap_or(0)
            };

            // Bytes per row without any padding.
            let bytes_per_row = total_num_bytes.checked_div(height).unwrap_or(0);

            if row_stride > bytes_per_row && height > 0 {
                // Row stride larger than the actual pixel data -- strip per-row padding.
                for row in 0..height {
                    let start = row * row_stride;
                    buffer.extend_from_slice(&blob[start..start + bytes_per_row]);
                }
            } else if blob.len() > total_num_bytes {
                // Data is larger than expected but rows aren't padded.
                // Common with ToF sensors that append metadata after pixel data.
                buffer.extend_from_slice(&blob[..total_num_bytes]);
            } else {
                buffer.extend_from_slice(blob);
            }

            push_offset(&buffer, &mut offsets)?;
        }

        let values = UInt8Array::from(buffer);
        let field = Arc::new(Field::new_list_field(DataType::UInt8, false));

        Ok(Some(Arc::new(ListArray::new(
            field,
            OffsetBuffer::new(offsets.into()),
            Arc::new(values),
            source.nulls().cloned(),
        )) as ArrayRef))
    }
}

/// Appends the current buffer length as the next offset for building a `ListArray`.
fn push_offset(buffer: &[u8], offsets: &mut Vec<i32>) -> Result<(), Error> {
    offsets.push(
        i32::try_from(buffer.len()).map_err(|_err| Error::OffsetOverflow {
            actual: buffer.len(),
            expected_type: "i32",
        })?,
    );
    Ok(())
}

pub(super) fn is_single_channel_encoding(s: &str) -> Result<bool, Error> {
    Ok(parse_encoding(s)?.is_single_channel())
}

fn parse_encoding(s: &str) -> Result<ImageEncoding, Error> {
    s.parse().map_err(|_err| Error::UnexpectedValue {
        expected: ImageEncoding::VARIANTS,
        actual: s.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks that every encoding reported in errors is accepted by the parser.
    #[test]
    fn parses_all_image_encoding_names() {
        for name in ImageEncoding::VARIANTS {
            parse_encoding(name).unwrap();
        }
    }
}
