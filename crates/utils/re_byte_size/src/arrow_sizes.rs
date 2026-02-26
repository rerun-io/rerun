use arrow::array::{Array, ListArray, RecordBatch};
use arrow::buffer::ScalarBuffer;
use arrow::datatypes::{ArrowNativeType, DataType, Field, Fields, Schema, UnionFields};

#[expect(unused_imports)] // for docs
use arrow::array::ArrayData;

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
                .map(std::sync::Arc::heap_size_bytes)
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

/// Returns the total number of the bytes of memory occupied by the buffers by this slice of
/// [`ArrayData`] (See also diagram on [`ArrayData`]).
///
/// This is approximately the number of bytes if a new [`ArrayData`] was formed by creating new
/// `Buffer`s with exactly the data needed.
///
/// For example, a [`DataType::Int64`] with `100` elements, [`ArrayData::get_slice_memory_size`] would
/// return `100 * 8 = 800`. If the [`ArrayData`] was then [`Array::slice`]ed to refer to its first
/// `20` elements, then [`ArrayData::get_slice_memory_size`] on the sliced [`ArrayData`] would return
/// `20 * 8 = 160`.
///
/// ## Important notes regarding deep vs. shallow slicing
///
/// Deeply nested data that was shallow-sliced (i.e. using [`Array::slice]` instead of the deep-slicing
/// helpers from `re_arrow_util`) might report sizes that do not make any intuitive sense.
/// Always prefer deep-slicing when you need to reliably measure the physical size of the sliced data.
fn array_slice_memory_size(array: &dyn Array) -> u64 {
    array
        .to_data()
        .get_slice_memory_size()
        .unwrap_or_else(|_| array.get_buffer_memory_size()) as u64
}
