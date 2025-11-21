use arrow::{
    array::{Array, ArrayRef, ListArray, RecordBatch},
    buffer::ScalarBuffer,
    datatypes::{ArrowNativeType, DataType, Field, Fields, Schema, UnionFields},
};

use super::SizeBytes;

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
    fn heap_size_bytes(&self) -> u64 {
        self.iter().map(|(_, field)| field.heap_size_bytes()).sum()
    }
}

// ---

/// Returns the accurate memory size of an Arrow array, accounting for slicing.
///
/// For `ListArray`s, this manually calculates the size by only counting the memory used by
/// the list entries that are actually in the slice, since Arrow's `get_slice_memory_size()`
/// doesn't properly handle the case where sliced `ListArray`s still reference large inner data.
fn array_slice_memory_size(array: &dyn Array) -> u64 {
    // Special handling for ListArrays to get accurate slice memory size
    if let Some(list_array) = array.as_any().downcast_ref::<ListArray>() {
        return list_array_slice_memory_size(list_array);
    }

    // For other array types, use Arrow's built-in slice memory sizing
    array
        .to_data()
        .get_slice_memory_size()
        .unwrap_or_else(|_| array.get_buffer_memory_size()) as u64
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
    if let Some(validity) = list_array.nulls() {
        total_size += validity.len().div_ceil(8) as u64; // bits to bytes, rounded up
    }

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
