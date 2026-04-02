//! Helper functions for converting raw image data to Rerun image components.

use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, BinaryArray, ListArray, StringArray, StructArray, UInt8Array, UInt32Array,
};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field};
use re_lenses_core::combinators::Error;
use re_sdk_types::Loggable as _;
use re_sdk_types::datatypes::ImageFormat;

use super::helpers::get_field_as;

/// Returns a pipe-compatible function that converts a struct with `width`, `height`, and
/// `encoding` fields into a Rerun [`ImageFormat`] struct array.
pub(crate) fn encoding_to_image_format()
-> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let source = source
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "StructArray".to_owned(),
                actual: source.data_type().clone(),
                context: "encoding_to_image_format input".to_owned(),
            })?;

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
            .collect::<Result<_, Error>>()?;

        let array_ref = ImageFormat::to_arrow_opt(formats.iter().map(|f| f.as_ref()))
            .map_err(|err| Error::Other(err.to_string()))?;

        Ok(Some(array_ref))
    }
}

/// Returns a pipe-compatible function that extracts image buffer data from a struct with
/// `width`, `height`, `step`, `encoding`, and `data` fields.
pub(crate) fn extract_image_buffer()
-> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        re_tracing::profile_function!();

        let source = source
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "StructArray".to_owned(),
                actual: source.data_type().clone(),
                context: "extract_image_buffer input".to_owned(),
            })?;

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

            // How many bytes Rerun expects for this encoding (e.g. 8x8 16UC1 -> 128).
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

/// Parses an encoding string into an [`re_mcap::ImageEncoding`], mapping the error for use in transforms.
fn parse_encoding(s: &str) -> Result<re_mcap::ImageEncoding, Error> {
    s.parse().map_err(|_err| Error::UnexpectedValue {
        expected: re_mcap::ImageEncoding::NAMES,
        actual: s.to_owned(),
    })
}
