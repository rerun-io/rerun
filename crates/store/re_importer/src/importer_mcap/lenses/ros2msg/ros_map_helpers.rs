use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, Int8Array, ListArray, ListBuilder, StructArray, UInt8Builder, UInt32Array,
};
use re_lenses_core::combinators::Error;
use re_sdk_types::Loggable as _;
use re_sdk_types::components::{Colormap, ImageFormat};
use re_sdk_types::datatypes::{ChannelDatatype, ColorModel};

use crate::importer_mcap::lenses::helpers::get_field_as;

/// Returns a pipe-compatible function that converts ROS maps
/// stored in i8 buffers to Rerun image buffers.
///
/// ROS map buffers start at map cell `(0, 0)`, i.e. the bottom image row,
/// while Rerun's image buffers are consumed top row first.
pub(crate) fn map_buffer_to_image_buffer(
    metadata_field: &'static str,
    width_field: &'static str,
    height_field: &'static str,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let source = source
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "StructArray".to_owned(),
                actual: source.data_type().clone(),
                context: "map_buffer_to_image_buffer input".to_owned(),
            })?;

        let data = get_field_as::<ListArray>(source, "data")?;
        let metadata = get_field_as::<StructArray>(source, metadata_field)?;
        let width = get_field_as::<UInt32Array>(&metadata, width_field)?;
        let height = get_field_as::<UInt32Array>(&metadata, height_field)?;
        let values = data
            .values()
            .as_any()
            .downcast_ref::<Int8Array>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "Int8Array".to_owned(),
                actual: data.values().data_type().clone(),
                context: "map_buffer_to_image_buffer values".to_owned(),
            })?;

        ros_map_buffer_to_image_buffer(source, &data, &metadata, &width, &height, |builder, idx| {
            // Preserve ROS occupancy byte conventions, in particular `-1 -> 255` for unknown cells.
            builder.append_value(values.value(idx) as u8);
        })
    }
}

/// Returns a pipe-compatible function that converts a struct with `width` and `height`
/// fields into a grayscale 8-bit Rerun [`ImageFormat`] struct array.
pub(crate) fn map_dimensions_to_l8_image_format()
-> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let source = source
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "StructArray".to_owned(),
                actual: source.data_type().clone(),
                context: "map_dimensions_to_l8_image_format input".to_owned(),
            })?;

        let width_array = get_field_as::<UInt32Array>(source, "width")?;
        let height_array = get_field_as::<UInt32Array>(source, "height")?;

        let formats: Vec<ImageFormat> = (0..source.len())
            .map(|i| {
                ImageFormat::from_color_model(
                    [width_array.value(i), height_array.value(i)],
                    ColorModel::L,
                    ChannelDatatype::U8,
                )
            })
            .collect();

        ImageFormat::to_arrow_opt(formats.iter().map(Some))
            .map(Some)
            .map_err(|err| Error::Other(err.to_string()))
    }
}

/// Returns a pipe-compatible function that fills a Rerun [`Colormap`] array with a
/// single repeated ROS map colormap value.
pub(crate) fn default_ros_map_colormap(
    colormap: Colormap,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let len = source.len();
        Colormap::to_arrow_opt(std::iter::repeat_n(Some(colormap), len))
            .map(Some)
            .map_err(|err| Error::Other(err.to_string()))
    }
}

/// Reorders a ROS map buffer into top-row-first image order for `GridMap`.
fn ros_map_buffer_to_image_buffer(
    source: &StructArray,
    data: &ListArray,
    metadata: &StructArray,
    width: &UInt32Array,
    height: &UInt32Array,
    mut append_value: impl FnMut(&mut UInt8Builder, usize),
) -> Result<Option<ArrayRef>, Error> {
    let mut builder = ListBuilder::new(UInt8Builder::with_capacity(data.values().len()));
    let row_nulls = data.values().nulls();

    for row in 0..source.len() {
        if data.is_null(row) || metadata.is_null(row) || width.is_null(row) || height.is_null(row) {
            builder.append(false);
            continue;
        }

        let row_width = width.value(row) as usize;
        let row_height = height.value(row) as usize;
        let row_len = row_width.checked_mul(row_height).ok_or_else(|| {
            Error::Other("ros_map_buffer_to_image_buffer dimensions overflow".to_owned())
        })?;

        let start = data.value_offsets()[row] as usize;
        let end = data.value_offsets()[row + 1] as usize;
        if end - start != row_len {
            return Err(Error::Other(format!(
                "ros_map_buffer_to_image_buffer expected {} cells from {}x{} grid, got {}",
                row_len,
                row_width,
                row_height,
                end - start
            )));
        }

        for image_row in (0..row_height).rev() {
            let row_start = start + image_row * row_width;
            let row_end = row_start + row_width;
            for idx in row_start..row_end {
                if row_nulls.is_some_and(|nulls| !nulls.is_valid(idx)) {
                    builder.values().append_null();
                } else {
                    append_value(builder.values(), idx);
                }
            }
        }

        builder.append(true);
    }

    Ok(Some(Arc::new(builder.finish()) as ArrayRef))
}
