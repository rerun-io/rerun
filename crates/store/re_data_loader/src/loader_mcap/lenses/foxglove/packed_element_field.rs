//! Helper functions for decoding byte arrays of [`PackedElementField`] data,
//! e.g. for extracting positions and colors from [`foxglove.PointCloud`] messages.
//!
//! [`PackedElementField`]: https://docs.foxglove.dev/docs/sdk/schemas/packed-element-field
//! [`foxglove.PointCloud`]: https://docs.foxglove.dev/docs/sdk/schemas/point-cloud

use arrow::array::builder::{FixedSizeListBuilder, Float32Builder, ListBuilder, UInt32Builder};
use arrow::array::{
    Array as _, BinaryArray, Int32Array, ListArray, StringArray, StructArray, UInt32Array,
};
use arrow::datatypes::{DataType, Field};
use re_arrow_combinators::Transform;
use re_arrow_combinators::map::MapList;
use re_arrow_combinators::reshape::Flatten;
use re_lenses::OpError;

use crate::loader_mcap::lenses::helpers::get_field_as;

/// Extracts position data from point cloud messages as a `List<FixedSizeList<f32, 3>>`.
pub fn extract_positions(list_array: &ListArray) -> Result<ListArray, OpError> {
    Ok(MapList::new(ExtractPositions)
        .then(Flatten::new())
        .transform(list_array)?)
}

/// Extracts RGBA color data from point cloud messages as a `List<u32>`.
pub fn extract_colors(list_array: &ListArray) -> Result<ListArray, OpError> {
    Ok(MapList::new(ExtractColors)
        .then(Flatten::new())
        .transform(list_array)?)
}

/// Foxglove [`NumericType`] enum.
///
/// [`NumericType`]: https://docs.foxglove.dev/docs/sdk/schemas/numeric-type
#[derive(Clone, Copy)]
#[repr(i32)]
enum NumericType {
    Uint8 = 1,
    Int8 = 2,
    Uint16 = 3,
    Int16 = 4,
    Uint32 = 5,
    Int32 = 6,
    Float32 = 7,
    Float64 = 8,
}

impl TryFrom<i32> for NumericType {
    type Error = re_arrow_combinators::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Uint8),
            2 => Ok(Self::Int8),
            3 => Ok(Self::Uint16),
            4 => Ok(Self::Int16),
            5 => Ok(Self::Uint32),
            6 => Ok(Self::Int32),
            7 => Ok(Self::Float32),
            8 => Ok(Self::Float64),
            _ => Err(re_arrow_combinators::Error::Other(format!(
                "unknown NumericType value: {value}"
            ))),
        }
    }
}

impl NumericType {
    fn byte_size(self) -> usize {
        match self {
            Self::Uint8 | Self::Int8 => 1,
            Self::Uint16 | Self::Int16 => 2,
            Self::Uint32 | Self::Int32 | Self::Float32 => 4,
            Self::Float64 => 8,
        }
    }

    /// Reads a value from packed data at the given byte offset and converts it to `f32`.
    #[expect(clippy::cast_possible_wrap)]
    fn read_as_f32(self, data: &[u8], byte_offset: usize) -> f32 {
        if byte_offset + self.byte_size() > data.len() {
            return 0.0;
        }
        let bytes = &data[byte_offset..];
        match self {
            Self::Uint8 => bytes[0] as f32,
            Self::Int8 => (bytes[0] as i8) as f32,
            Self::Uint16 => u16::from_le_bytes([bytes[0], bytes[1]]) as f32,
            Self::Int16 => i16::from_le_bytes([bytes[0], bytes[1]]) as f32,
            Self::Uint32 => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32,
            Self::Int32 => i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32,
            Self::Float32 => f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            Self::Float64 => f64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]) as f32,
        }
    }

    /// Reads a numeric value from packed data at the given byte offset and clamps/converts it to `u8`.
    fn read_as_u8(self, data: &[u8], byte_offset: usize) -> u8 {
        if byte_offset + self.byte_size() > data.len() {
            return 0;
        }
        let bytes = &data[byte_offset..];
        match self {
            Self::Uint8 => bytes[0],
            // intentional reinterpretation of raw byte as signed
            #[expect(clippy::cast_possible_wrap)]
            Self::Int8 => (bytes[0] as i8).clamp(0, i8::MAX) as u8,
            Self::Uint16 => u16::from_le_bytes([bytes[0], bytes[1]]).min(255) as u8,
            Self::Int16 => i16::from_le_bytes([bytes[0], bytes[1]]).clamp(0, 255) as u8,
            Self::Uint32 => {
                u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).min(255) as u8
            }
            Self::Int32 => {
                i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).clamp(0, 255) as u8
            }
            Self::Float32 => (f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) * 255.0)
                .clamp(0.0, 255.0) as u8,
            Self::Float64 => (f64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]) * 255.0)
                .clamp(0.0, 255.0) as u8,
        }
    }
}

/// Byte offset and numeric type of a packed field within a point.
struct FieldDescriptor {
    byte_offset: usize,
    numeric_type: NumericType,
}

/// Searches the `fields` struct array for entries matching the given names and returns
/// their byte offsets and numeric types.
fn find_field_descriptors(
    fields_struct: &StructArray,
    names: &[&str],
) -> Result<Vec<Option<FieldDescriptor>>, re_arrow_combinators::Error> {
    let name_array = fields_struct
        .column_by_name("name")
        .and_then(|a| a.as_any().downcast_ref::<StringArray>().cloned());
    let offset_array = fields_struct
        .column_by_name("offset")
        .and_then(|a| a.as_any().downcast_ref::<UInt32Array>().cloned());
    // Protobuf enums are stored as Struct{name: Utf8, value: Int32}; extract the `value` field.
    let type_array = fields_struct
        .column_by_name("type")
        .and_then(|a| a.as_any().downcast_ref::<StructArray>())
        .and_then(|s| s.column_by_name("value"))
        .and_then(|a| a.as_any().downcast_ref::<Int32Array>().cloned());

    let (Some(name_array), Some(offset_array), Some(type_array)) =
        (name_array, offset_array, type_array)
    else {
        return Ok(names.iter().map(|_| None).collect());
    };

    names
        .iter()
        .map(|target_name| {
            for i in 0..name_array.len() {
                if !name_array.is_null(i) && name_array.value(i) == *target_name {
                    return Ok(Some(FieldDescriptor {
                        byte_offset: offset_array.value(i) as usize,
                        numeric_type: NumericType::try_from(type_array.value(i))?,
                    }));
                }
            }
            Ok(None)
        })
        .collect()
}

struct ExtractPositions;

impl Transform for ExtractPositions {
    type Source = StructArray;
    type Target = ListArray;

    fn transform(&self, source: &StructArray) -> Result<ListArray, re_arrow_combinators::Error> {
        re_tracing::profile_function!();

        let point_stride_array = get_field_as::<UInt32Array>(source, "point_stride")?;
        let fields_array = get_field_as::<ListArray>(source, "fields")?;
        let data_array = get_field_as::<BinaryArray>(source, "data")?;

        let mut builder = ListBuilder::new(
            FixedSizeListBuilder::new(Float32Builder::new(), 3).with_field(Field::new(
                "item",
                DataType::Float32,
                false,
            )),
        );

        for i in 0..source.len() {
            if source.is_null(i) || data_array.is_null(i) || fields_array.is_null(i) {
                builder.append_null();
                continue;
            }

            let point_stride = point_stride_array.value(i) as usize;
            let data = data_array.value(i);
            let fields_value = fields_array.value(i);
            let fields_struct = fields_value
                .as_any()
                .downcast_ref::<StructArray>()
                .ok_or_else(|| re_arrow_combinators::Error::TypeMismatch {
                    expected: "StructArray".to_owned(),
                    actual: fields_value.data_type().clone(),
                    context: "fields element".to_owned(),
                })?;

            let descriptors = find_field_descriptors(fields_struct, &["x", "y", "z"])?;

            if let [Some(x_desc), Some(y_desc), Some(z_desc)] = &descriptors[..]
                && point_stride > 0
            {
                let num_points = data.len() / point_stride;
                let points_builder = builder.values();
                for p in 0..num_points {
                    let base = p * point_stride;
                    points_builder.values().append_value(
                        x_desc
                            .numeric_type
                            .read_as_f32(data, base + x_desc.byte_offset),
                    );
                    points_builder.values().append_value(
                        y_desc
                            .numeric_type
                            .read_as_f32(data, base + y_desc.byte_offset),
                    );
                    points_builder.values().append_value(
                        z_desc
                            .numeric_type
                            .read_as_f32(data, base + z_desc.byte_offset),
                    );
                    points_builder.append(true);
                }
                builder.append(true);
            } else {
                builder.append_null();
            }
        }

        Ok(builder.finish())
    }
}

struct ExtractColors;

impl Transform for ExtractColors {
    type Source = StructArray;
    type Target = ListArray;

    fn transform(&self, source: &StructArray) -> Result<ListArray, re_arrow_combinators::Error> {
        re_tracing::profile_function!();

        let point_stride_array = get_field_as::<UInt32Array>(source, "point_stride")?;
        let fields_array = get_field_as::<ListArray>(source, "fields")?;
        let data_array = get_field_as::<BinaryArray>(source, "data")?;

        let mut builder = ListBuilder::new(UInt32Builder::new());

        for i in 0..source.len() {
            if source.is_null(i) || data_array.is_null(i) || fields_array.is_null(i) {
                builder.append_null();
                continue;
            }

            let point_stride = point_stride_array.value(i) as usize;
            let data = data_array.value(i);
            let fields_value = fields_array.value(i);
            let fields_struct = fields_value
                .as_any()
                .downcast_ref::<StructArray>()
                .ok_or_else(|| re_arrow_combinators::Error::TypeMismatch {
                    expected: "StructArray".to_owned(),
                    actual: fields_value.data_type().clone(),
                    context: "fields element".to_owned(),
                })?;

            let descriptors =
                find_field_descriptors(fields_struct, &["red", "green", "blue", "alpha"])?;

            if let (Some(r_desc), Some(g_desc), Some(b_desc)) =
                (&descriptors[0], &descriptors[1], &descriptors[2])
                && point_stride > 0
            {
                let alpha_desc = &descriptors[3];
                let num_points = data.len() / point_stride;
                for p in 0..num_points {
                    let base = p * point_stride;
                    let r = r_desc
                        .numeric_type
                        .read_as_u8(data, base + r_desc.byte_offset);
                    let g = g_desc
                        .numeric_type
                        .read_as_u8(data, base + g_desc.byte_offset);
                    let b = b_desc
                        .numeric_type
                        .read_as_u8(data, base + b_desc.byte_offset);
                    let a = alpha_desc.as_ref().map_or(255, |d| {
                        d.numeric_type.read_as_u8(data, base + d.byte_offset)
                    });
                    // Convert to packed RGBA u32 format expected by Rerun.
                    builder.values().append_value(
                        ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32),
                    );
                }
                builder.append(true);
            } else {
                builder.append_null();
            }
        }

        Ok(builder.finish())
    }
}
