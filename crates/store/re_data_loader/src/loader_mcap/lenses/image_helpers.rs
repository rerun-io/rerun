//! Helper functions for converting raw image data to Rerun image components.

use std::sync::Arc;

use arrow::array::{
    Array as _, BinaryArray, ListArray, StringArray, StructArray, UInt8Array, UInt32Array,
};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field};
use re_arrow_combinators::Transform;
use re_arrow_combinators::map::MapList;
use re_lenses::OpError;
use re_sdk_types::Loggable as _;
use re_sdk_types::datatypes::ImageFormat;

use super::helpers::get_field_as;

/// Converts a struct with `width`, `height`, and `encoding` fields into a Rerun
/// [`ImageFormat`] struct array, using [`re_mcap::ImageEncoding`].
pub fn encoding_to_image_format(list_array: &ListArray) -> Result<ListArray, OpError> {
    Ok(MapList::new(EncodingToImageFormat).transform(list_array)?)
}

/// Extracts image buffer data from a struct with `width`, `height`, `step`, `encoding`,
/// and `data` fields. Strips row padding when the data is larger than expected for the
/// given encoding.
pub fn extract_image_buffer(list_array: &ListArray) -> Result<ListArray, OpError> {
    Ok(MapList::new(ExtractImageBuffer).transform(list_array)?)
}

struct EncodingToImageFormat;

impl Transform for EncodingToImageFormat {
    type Source = StructArray;
    type Target = StructArray;

    fn transform(&self, source: &StructArray) -> Result<StructArray, re_arrow_combinators::Error> {
        let width_array = get_field_as::<UInt32Array>(source, "width")?;
        let height_array = get_field_as::<UInt32Array>(source, "height")?;
        let encoding_array = get_field_as::<StringArray>(source, "encoding")?;

        let formats: Vec<Option<ImageFormat>> = (0..source.len())
            .map(|i| {
                if encoding_array.is_null(i) {
                    return Ok(None);
                }
                let encoding = parse_encoding(encoding_array.value(i))?;
                Ok(Some(encoding.to_image_format([
                    width_array.value(i),
                    height_array.value(i),
                ])))
            })
            .collect::<Result<_, re_arrow_combinators::Error>>()?;

        let array_ref = ImageFormat::to_arrow_opt(formats.iter().map(|f| f.as_ref()))
            .map_err(|err| re_arrow_combinators::Error::Other(err.to_string()))?;

        array_ref
            .as_any()
            .downcast_ref::<StructArray>()
            .cloned()
            .ok_or_else(|| re_arrow_combinators::Error::TypeMismatch {
                expected: "StructArray".to_owned(),
                actual: array_ref.data_type().clone(),
                context: "ImageFormat serialization".to_owned(),
            })
    }
}

struct ExtractImageBuffer;

impl Transform for ExtractImageBuffer {
    type Source = StructArray;
    type Target = ListArray;

    fn transform(&self, source: &StructArray) -> Result<ListArray, re_arrow_combinators::Error> {
        re_tracing::profile_function!();

        let width_array = get_field_as::<UInt32Array>(source, "width")?;
        let height_array = get_field_as::<UInt32Array>(source, "height")?;
        let step_array = get_field_as::<UInt32Array>(source, "step")?;
        let encoding_array = get_field_as::<StringArray>(source, "encoding")?;
        let data_array = get_field_as::<BinaryArray>(source, "data")?;

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

            // How many bytes Rerun expects for this encoding (e.g. 8×8 16UC1 -> 128).
            let encoding = parse_encoding(encoding_array.value(i))?;
            let total_num_bytes = encoding
                .to_image_format([width_array.value(i), height_array.value(i)])
                .num_bytes();

            // Row stride: trust `step` when set, otherwise if unset (0) fall back to deriving it from the data size.
            let step = step_array.value(i) as usize;
            let row_stride = if step > 0 {
                step
            } else if height > 0 {
                blob.len() / height
            } else {
                0
            };

            // Bytes per row without any padding.
            let bytes_per_row = if height > 0 {
                total_num_bytes / height
            } else {
                0
            };

            if row_stride > bytes_per_row && height > 0 {
                // Row stride larger than the actual pixel data — strip per-row padding.
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

        Ok(ListArray::new(
            field,
            OffsetBuffer::new(offsets.into()),
            Arc::new(values),
            source.nulls().cloned(),
        ))
    }
}

/// Appends the current buffer length as the next offset for building a `ListArray`.
fn push_offset(buffer: &[u8], offsets: &mut Vec<i32>) -> Result<(), re_arrow_combinators::Error> {
    offsets.push(i32::try_from(buffer.len()).map_err(|_err| {
        re_arrow_combinators::Error::OffsetOverflow {
            actual: buffer.len(),
            expected_type: "i32",
        }
    })?);
    Ok(())
}

/// Parses an encoding string into an [`re_mcap::ImageEncoding`], mapping the error for use in transforms.
fn parse_encoding(s: &str) -> Result<re_mcap::ImageEncoding, re_arrow_combinators::Error> {
    s.parse()
        .map_err(|_err| re_arrow_combinators::Error::UnexpectedValue {
            expected: re_mcap::ImageEncoding::NAMES,
            actual: s.to_owned(),
        })
}
