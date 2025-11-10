//! Semantic array transforms for concrete applications.

use std::marker::PhantomData;
use std::sync::Arc;

use arrow::array::{
    Array as _, GenericBinaryArray, GenericListArray, Int32Array, Int64Array, OffsetSizeTrait,
    StructArray,
};
use arrow::datatypes::{DataType, Field};

use crate::{Error, Transform};

/// Converts binary arrays to list arrays where each binary element becomes a list of `u8`.
///
/// The underlying bytes buffer is reused, making this transformation almost zero-copy.
#[derive(Clone, Debug, Default)]
pub struct BinaryToListUInt8<O1: OffsetSizeTrait, O2: OffsetSizeTrait = O1> {
    _from_offset: PhantomData<O1>,
    _to_offset: PhantomData<O2>,
}

impl<O1: OffsetSizeTrait, O2: OffsetSizeTrait> BinaryToListUInt8<O1, O2> {
    /// Create a new transformation to convert a binary array to a list array of `u8` arrays.
    pub fn new() -> Self {
        Default::default()
    }
}

impl<O1: OffsetSizeTrait, O2: OffsetSizeTrait> Transform for BinaryToListUInt8<O1, O2> {
    type Source = GenericBinaryArray<O1>;
    type Target = GenericListArray<O2>;

    fn transform(&self, source: &GenericBinaryArray<O1>) -> Result<Self::Target, Error> {
        use arrow::array::UInt8Array;
        use arrow::buffer::ScalarBuffer;

        let scalar_buffer: ScalarBuffer<u8> = ScalarBuffer::from(source.values().clone());
        let uint8_array = UInt8Array::new(scalar_buffer, None);

        // Convert from O1 to O2. Most offset buffers will be small in real-world
        // examples, so we're fine copying them.
        //
        // This could be true zero copy if Rust had specialization.
        // More info: https://std-dev-guide.rust-lang.org/policy/specialization.html
        let old_offsets = source.offsets().iter();
        let new_offsets: Result<Vec<O2>, Error> = old_offsets
            .map(|&offset| {
                let offset_usize = offset.as_usize();
                O2::from_usize(offset_usize).ok_or_else(|| Error::OffsetOverflow {
                    actual: offset_usize,
                    expected_type: std::any::type_name::<O2>(),
                })
            })
            .collect();
        let offsets = arrow::buffer::OffsetBuffer::new(new_offsets?.into());

        let list = Self::Target::new(
            Arc::new(Field::new_list_field(DataType::UInt8, false)),
            offsets,
            Arc::new(uint8_array),
            source.nulls().cloned(),
        );

        Ok(list)
    }
}

/// Converts `StructArray` of timestamps with `seconds` (i64) and `nanos` (i32) fields
/// to `Int64Array` containing the corresponding total nanoseconds timestamps.
#[derive(Default)]
pub struct TimeSpecToNanos {}

impl Transform for TimeSpecToNanos {
    type Source = StructArray;
    type Target = Int64Array;

    fn transform(&self, source: &StructArray) -> Result<Self::Target, Error> {
        let available_fields: Vec<String> =
            source.fields().iter().map(|f| f.name().clone()).collect();

        let seconds_array =
            source
                .column_by_name("seconds")
                .ok_or_else(|| Error::MissingStructField {
                    field_name: "seconds".to_owned(),
                    struct_fields: available_fields.clone(),
                })?;
        let nanos_array =
            source
                .column_by_name("nanos")
                .ok_or_else(|| Error::MissingStructField {
                    field_name: "nanos".to_owned(),
                    struct_fields: available_fields,
                })?;

        let seconds_array = seconds_array
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or_else(|| Error::UnexpectedListValueType {
                expected: "Int64Array".to_owned(),
                actual: seconds_array.data_type().clone(),
            })?;
        let nanos_array = nanos_array
            .as_any()
            .downcast_ref::<Int32Array>()
            .ok_or_else(|| Error::UnexpectedListValueType {
                expected: "Int32Array".to_owned(),
                actual: nanos_array.data_type().clone(),
            })?;

        let mut output_builder = Int64Array::builder(source.len());

        for i in 0..source.len() {
            if source.is_null(i) {
                output_builder.append_null();
            } else {
                let seconds = seconds_array.value(i);
                let nanos = nanos_array.value(i);
                let total_nanos = seconds * 1_000_000_000 + nanos as i64;
                output_builder.append_value(total_nanos);
            }
        }

        Ok(output_builder.finish())
    }
}
