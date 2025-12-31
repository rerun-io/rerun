use arrow::array::{Array, ArrayRef, ListArray, RecordBatch, StructArray, UnionArray, StringArray};
use arrow::buffer::{Buffer, ScalarBuffer};
use arrow::datatypes::{ArrowNativeType, DataType, Field, Fields, Schema, UnionFields, UnionMode};

use super::SizeBytes;

impl SizeBytes for Buffer {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.capacity() as u64
    }
}

impl SizeBytes for dyn Array {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        array_slice_memory_size(self)
    }
}

impl<T: Array> SizeBytes for &T {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        array_slice_memory_size(self)
    }
}

impl SizeBytes for ArrayRef {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        array_slice_memory_size(self)
    }
}

impl<T: ArrowNativeType> SizeBytes for ScalarBuffer<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.inner().len() as u64
    }
}

impl SizeBytes for ListArray {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        array_slice_memory_size(self)
    }
}

impl SizeBytes for RecordBatch {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.schema().heap_size_bytes()
            + self
                .columns()
                .iter()
                .map(|array| array.heap_size_bytes())
                .sum::<u64>()
    }
}

impl SizeBytes for Schema {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self { fields, metadata } = self;
        fields.heap_size_bytes() + metadata.heap_size_bytes()
    }
}

impl SizeBytes for Fields {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.iter().map(|field| field.heap_size_bytes()).sum()
    }
}

impl SizeBytes for Field {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.name().heap_size_bytes() + self.data_type().heap_size_bytes()
    }
}

impl SizeBytes for DataType {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Null
            | Self::Boolean
            | Self::Int8
            | Self::Int16
            | Self::Int32
            | Self::Int64
            | Self::UInt8
            | Self::UInt16
            | Self::UInt32
            | Self::UInt64
            | Self::Float16
            | Self::Float32
            | Self::Float64
            | Self::Date32
            | Self::Date64
            | Self::Binary
            | Self::LargeBinary
            | Self::Utf8
            | Self::LargeUtf8
            | Self::BinaryView
            | Self::Decimal32(_, _)
            | Self::Decimal64(_, _)
            | Self::Decimal128(_, _)
            | Self::Decimal256(_, _)
            | Self::FixedSizeBinary(_)
            | Self::Utf8View => 0,
            Self::Timestamp(_time_unit, _tz) => 0,

            Self::Time32(_time_unit) | Self::Time64(_time_unit) | Self::Duration(_time_unit) => 0,

            Self::Interval(_interval_unit) => 0,

            Self::List(field)
            | Self::ListView(field)
            | Self::FixedSizeList(field, _)
            | Self::LargeList(field)
            | Self::Map(field, _)
            | Self::LargeListView(field) => field.heap_size_bytes(),

            Self::Union(fields, _) => fields.heap_size_bytes(),
            Self::Struct(fields) => fields.heap_size_bytes(),

            Self::Dictionary(key, value) => key.heap_size_bytes() + value.heap_size_bytes(),

            Self::RunEndEncoded(field, field1) => {
                field.heap_size_bytes() + field1.heap_size_bytes()
            }
        }
    }
}

impl SizeBytes for UnionFields {
    fn heap_size_bytes(&self) -> u64 {2
        self.iter().map(|(_, field)| field.heap_size_bytes()).sum()
    }
}

// ---

/// Returns the accurate memory size of an Arrow array, accounting for slicing.
///
/// For `ListArray`s, `StructArray`s, and `UnionArray`s, this manually calculates the size by only counting
/// the memory used by the entries that are actually in the slice, since Arrow's
/// `get_slice_memory_size()` doesn't properly handle the case where sliced arrays still
/// reference large inner data.
fn array_slice_memory_size(array: &dyn Array) -> u64 {
    // Special handling for ListArrays to get accurate slice memory size
    if let Some(list_array) = array.as_any().downcast_ref::<ListArray>() {
        return list_array_slice_memory_size(list_array);
    }

    // Special handling for StructArrays to get accurate slice memory size
    if let Some(struct_array) = array.as_any().downcast_ref::<StructArray>() {
        return struct_array_slice_memory_size(struct_array);
    }

    // Special handling for UnionArrays (dense unions) to get accurate slice memory size
    if let Some(union_array) = array.as_any().downcast_ref::<UnionArray>() {
        return union_array_slice_memory_size(union_array);
    }

    // Special handling for StringArrays to get accurate slice memory size
    if let Some(string_array) = array.as_any().downcast_ref::<StringArray>() {
        return string_array_slice_memory_size(string_array);
    }

    // For other array types, use Arrow's built-in slice memory sizing
    let slice_memory = array
        .to_data()
        .get_slice_memory_size()
        .unwrap_or_else(|_| array.get_buffer_memory_size()) as u64;
    re_log::trace!("\tSlice memory here: {slice_memory}");
    slice_memory
}

/// Calculate the size of the validity buffer for an array.
fn validity_buffer_size(array: &dyn Array) -> u64 {
    if let Some(validity) = array.nulls() {
        validity.len().div_ceil(8) as u64 // bits to bytes, rounded up
    } else {
        0
    }
}

/// Calculate the accurate memory size of a sliced `ListArray`.
///
/// This manually computes the size by only counting the memory of the list entries
/// that are actually accessible in the slice, rather than the entire underlying data.
fn list_array_slice_memory_size(list_array: &ListArray) -> u64 {
    // Base size: offsets buffer + validity buffer + metadata
    let mut total_size = 0u64;

    // Offsets buffer: (length + 1) * size_of::<i32>()
    total_size += (list_array.len() + 1) as u64 * std::mem::size_of::<i32>() as u64;

    // Validity buffer if present
    total_size += validity_buffer_size(list_array);

    // Calculate the range of inner values that are actually used by this slice
    let offsets = list_array.value_offsets();
    if offsets.len() < 2 {
        return total_size; // Empty array
    }

    let start_offset = offsets[0] as usize;
    let end_offset = offsets[offsets.len() - 1] as usize;
    let values_len = end_offset - start_offset;

    if values_len > 0 {
        // Get the inner array and slice it to only the range we actually use
        let inner_array = list_array.values();
        let sliced_inner = inner_array.slice(start_offset, values_len);
        // Recursively calculate the size of the sliced inner array
        total_size += array_slice_memory_size(sliced_inner.as_ref());
    }

    total_size
}

/// Calculate the accurate memory size of a sliced `StructArray`.
///
/// This manually computes the size by only counting the memory of the struct fields
/// that are actually accessible in the slice, rather than the entire underlying data.
fn struct_array_slice_memory_size(struct_array: &StructArray) -> u64 {
    // Base size: validity buffer + metadata
    let mut total_size = validity_buffer_size(struct_array);

    // For each child array, we need to ensure it's properly sliced to match the struct array
    for child_array in struct_array.columns() {
        // If the struct array has been sliced, we need to slice the child arrays to match
        // the same offset and length as the struct array
        let sliced_child = if struct_array.offset() > 0 || child_array.len() != struct_array.len() {
            child_array.slice(struct_array.offset(), struct_array.len())
        } else {
            child_array.clone()
        };

        // Recursively calculate the size of the properly sliced child array
        total_size += array_slice_memory_size(sliced_child.as_ref());
    }

    re_log::trace!("\tTotal struct array size here: {total_size}");
    total_size
}

/// Calculate the accurate memory size of a sliced `UnionArray` (dense union).
///
/// This manually computes the size by only counting the memory of the union variants
/// that are actually accessible in the slice, rather than the entire underlying data.
fn union_array_slice_memory_size(union_array: &UnionArray) -> u64 {
    // Base size: type buffer + validity buffer
    let mut total_size = validity_buffer_size(union_array);

    // Type buffer: one byte per element in the slice
    total_size += union_array.len() as u64;

    // Check if this is a dense union by examining the underlying data
    let data = union_array.to_data();
    let is_dense = matches!(data.data_type(), arrow::datatypes::DataType::Union(_, arrow::datatypes::UnionMode::Dense));

    if is_dense {
        // Offsets buffer: 4 bytes (i32) per element in the slice
        total_size += union_array.len() as u64 * std::mem::size_of::<i32>() as u64;
    }

    // Get the type IDs present in this union
    let type_ids = union_array.type_ids();
    let mut processed_types = std::collections::HashSet::new();

    // Iterate through all type IDs used in this slice to find child arrays
    for &type_id in type_ids {
        if !processed_types.contains(&type_id) {
            processed_types.insert(type_id);

            let child_array = union_array.child(type_id);

            if is_dense {
                // For dense unions, we need to be more careful about slicing
                let sliced_child = if union_array.offset() > 0 || child_array.len() != union_array.len() {
                    child_array.slice(union_array.offset(), union_array.len())
                } else {
                    child_array.clone()
                };
                total_size += array_slice_memory_size(sliced_child.as_ref());
            } else {
                // For sparse unions, calculate the full child array size
                total_size += array_slice_memory_size(child_array.as_ref());
            }
        }
    }

    re_log::trace!("\tTotal union array size here: {total_size}");
    total_size
}

/// Calculate the accurate memory size of a sliced `StringArray`.
///
/// This manually computes the size by only counting the string data
/// that is actually accessible in the slice.
fn string_array_slice_memory_size(string_array: &StringArray) -> u64 {
    // Base size: offsets buffer + validity buffer
    let mut total_size = validity_buffer_size(string_array);

    // Offsets buffer: (length + 1) * size_of::<i32>()
    total_size += (string_array.len() + 1) as u64 * std::mem::size_of::<i32>() as u64;

    // Calculate the actual string data size for this slice
    let offsets = string_array.value_offsets();
    if offsets.len() >= 2 {
        let start_offset = offsets[0] as u64;
        let end_offset = offsets[offsets.len() - 1] as u64;
        total_size += end_offset - start_offset; // Actual string bytes
    }

    re_log::trace!("\tTotal string array size here: {total_size}");
    total_size
}
