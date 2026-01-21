//! Semantic array transforms for concrete applications.
//!
//! Note: These should not be exposed as part of the public API, but rather wrapped in [`crate::Op`].

use std::marker::PhantomData;
use std::sync::Arc;

use arrow::array::{
    Array as _, ArrowNativeTypeOp as _, GenericBinaryArray, GenericListArray, Int64Array,
    OffsetSizeTrait, StringArray, StructArray, UInt32Array, UInt32Builder,
};
use arrow::datatypes::{DataType, Field, Int32Type, Int64Type};
use arrow::error::ArrowError;
use re_sdk_types::components::VideoCodec;

use re_arrow_combinators::cast::DowncastRef;
use re_arrow_combinators::reshape::GetField;
use re_arrow_combinators::{Error, Transform};

/// Converts binary arrays to list arrays where each binary element becomes a list of `u8`.
///
/// The underlying bytes buffer is reused, making this transformation almost zero-copy.
#[derive(Clone, Debug, Default)]
pub struct BinaryToListUInt8<O1: OffsetSizeTrait, O2: OffsetSizeTrait = O1> {
    _from_offset: PhantomData<O1>,
    _to_offset: PhantomData<O2>,

    /// This transform is specifically intended for contiguous byte data,
    /// so we default to non-nullable lists.
    nullable: bool,
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
            Arc::new(Field::new_list_field(DataType::UInt8, self.nullable)),
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
        let seconds_array = GetField::new("seconds")
            .then(DowncastRef::<Int64Type>::new())
            .transform(source)?;
        let nanos_array = GetField::new("nanos")
            .then(DowncastRef::<Int32Type>::new())
            .transform(source)?;

        Ok(arrow::compute::try_binary(
            &seconds_array,
            &nanos_array,
            |seconds: i64, nanos: i32| -> Result<i64, ArrowError> {
                seconds
                    .mul_checked(1_000_000_000)?
                    .add_checked(nanos as i64)
            },
        )?)
    }
}

/// Transforms a `StringArray` of video codec names to a `UInt32Array`,
/// where each u32 corresponds to a Rerun `VideoCodec` enum value.
#[derive(Default)]
pub struct StringToVideoCodecUInt32 {}

impl Transform for StringToVideoCodecUInt32 {
    type Source = StringArray;
    type Target = UInt32Array;

    fn transform(&self, source: &StringArray) -> Result<Self::Target, Error> {
        Ok(source
            .iter()
            .try_fold(
                UInt32Builder::with_capacity(source.len()),
                |mut builder, maybe_str| {
                    if let Some(codec_str) = maybe_str {
                        let codec = match codec_str.to_lowercase().as_str() {
                            "h264" => VideoCodec::H264,
                            "h265" => VideoCodec::H265,
                            "av1" => VideoCodec::AV1,
                            _ => {
                                return Err(Error::UnexpectedValue {
                                    expected: &["h264", "h265", "av1"],
                                    actual: codec_str.to_owned(),
                                });
                            }
                        };
                        builder.append_value(codec as u32);
                    } else {
                        builder.append_null();
                    }
                    Ok(builder)
                },
            )?
            .finish())
    }
}
