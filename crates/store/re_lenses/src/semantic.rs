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
use re_arrow_combinators::{Error, Transform, reshape};

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
        let seconds_array = reshape::GetField::new("seconds")
            .then(DowncastRef::<Int64Type>::new())
            .transform(source)?;
        let nanos_array = reshape::GetField::new("nanos")
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{
        Array as _, GenericByteBuilder, Int32Array, Int64Array, StringArray, StructArray,
        UInt32Array,
    };
    use arrow::datatypes::{DataType, Field, GenericBinaryType};
    use re_arrow_combinators::{Error, Transform as _};
    use re_sdk_types::components::VideoCodec;
    use re_sdk_types::reflection::Enum as _;

    use super::*;

    // Generic test for binary arrays where the offset is the same.
    fn impl_binary_test<O1: OffsetSizeTrait, O2: OffsetSizeTrait>() {
        let mut builder = GenericByteBuilder::<GenericBinaryType<O1>>::new();
        builder.append_value(b"hello");
        builder.append_value(b"world");
        builder.append_null();
        builder.append_value(b"");
        builder.append_value([0x00, 0xFF, 0x42]);
        let binary_array = builder.finish();

        let result = BinaryToListUInt8::<O1, O2>::new()
            .transform(&binary_array)
            .unwrap();

        // Verify structure
        assert_eq!(result.len(), 5);
        assert!(!result.is_null(0));
        assert!(!result.is_null(1));
        assert!(result.is_null(2));
        assert!(!result.is_null(3));
        assert!(!result.is_null(4));

        {
            let list = result.value(0);
            let uint8 = list
                .as_any()
                .downcast_ref::<arrow::array::UInt8Array>()
                .unwrap();
            assert_eq!(uint8.len(), 5);
            assert_eq!(uint8.value(0) as char, 'h');
            assert_eq!(uint8.value(1) as char, 'e');
            assert_eq!(uint8.value(2) as char, 'l');
            assert_eq!(uint8.value(3) as char, 'l');
            assert_eq!(uint8.value(4) as char, 'o');
        }

        {
            let list = result.value(1);
            let uint8 = list
                .as_any()
                .downcast_ref::<arrow::array::UInt8Array>()
                .unwrap();
            assert_eq!(list.len(), 5);
            assert_eq!(uint8.value(0) as char, 'w');
            assert_eq!(uint8.value(1) as char, 'o');
            assert_eq!(uint8.value(2) as char, 'r');
            assert_eq!(uint8.value(3) as char, 'l');
            assert_eq!(uint8.value(4) as char, 'd');
        }

        assert!(result.is_null(2));

        {
            let list = result.value(3);
            let uint8 = list
                .as_any()
                .downcast_ref::<arrow::array::UInt8Array>()
                .unwrap();
            assert_eq!(uint8.len(), 0);
        }

        {
            let list = result.value(4);
            let uint8 = list
                .as_any()
                .downcast_ref::<arrow::array::UInt8Array>()
                .unwrap();
            assert_eq!(uint8.len(), 3);
            assert_eq!(uint8.value(0), 0x00);
            assert_eq!(uint8.value(1), 0xFF);
            assert_eq!(uint8.value(2), 0x42);
        }
    }

    #[test]
    fn test_binary_to_list_uint8() {
        // We test the different offset combinations.
        impl_binary_test::<i32, i32>();
        impl_binary_test::<i64, i32>();
        impl_binary_test::<i32, i64>();
        impl_binary_test::<i64, i64>();
    }

    #[test]
    fn test_binary_offset_overflow() {
        use arrow::array::LargeBinaryArray;
        use arrow::buffer::OffsetBuffer;

        // Create a LargeBinaryArray with an offset that exceeds i32::MAX
        let large_offset = i32::MAX as i64 + 1;

        let offsets = vec![0i64, large_offset];
        let offsets_buffer = OffsetBuffer::new(offsets.into());

        let values = vec![0u8; large_offset as usize];

        let large_binary = LargeBinaryArray::new(offsets_buffer, values.into(), None);

        // Try to convert from LargeBinaryArray (i64 offsets) to ListArray (i32 offsets)
        let transform = BinaryToListUInt8::<i64, i32>::new();
        let result = transform.transform(&large_binary);

        // Should fail with OffsetOverflow
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::OffsetOverflow {
                actual,
                expected_type,
            } => {
                assert_eq!(actual, large_offset as usize);
                assert_eq!(expected_type, "i32");
            }
            other => panic!("Expected OffsetOverflow error, got: {other:?}"),
        }
    }

    /// Tests that timespec structs are correctly converted to nanoseconds, including (mixed) null handling.
    #[test]
    fn test_timespec_to_nanos() {
        let seconds_field = Arc::new(Field::new("seconds", DataType::Int64, true));
        let nanos_field = Arc::new(Field::new("nanos", DataType::Int32, true));

        let seconds_array = Arc::new(Int64Array::from(vec![
            Some(1),
            Some(2),
            None,
            Some(3),
            None,
        ]));
        let nanos_array = Arc::new(Int32Array::from(vec![
            Some(500_000_000),
            None,
            Some(0),
            Some(250_000_000),
            None,
        ]));

        let struct_array = StructArray::new(
            vec![seconds_field, nanos_field].into(),
            vec![seconds_array, nanos_array],
            None,
        );
        let output_array = TimeSpecToNanos::default()
            .transform(&struct_array)
            .expect("transformation failed");
        let expected_array = Int64Array::from(vec![
            Some(1_500_000_000),
            None,
            None,
            Some(3_250_000_000),
            None,
        ]);
        assert_eq!(output_array, expected_array);
    }

    /// Tests that supported codecs are correctly converted, and checks case-insensitivity and null handling.
    #[test]
    fn test_string_to_codec_uint32() {
        // Note: mixed codecs normally don't make sense, but should be fine from a pure conversion perspective.
        let input_array = StringArray::from(vec![
            Some("H264"),
            None,
            Some("h264"),
            Some("H265"),
            Some("aV1"),
        ]);
        assert_eq!(input_array.null_count(), 1);
        let output_array = StringToVideoCodecUInt32::default()
            .transform(&input_array)
            .expect("transformation failed");
        assert_eq!(output_array.null_count(), 1);
        let expected_array = UInt32Array::from(vec![
            Some(VideoCodec::H264 as u32),
            None,
            Some(VideoCodec::H264 as u32),
            Some(VideoCodec::H265 as u32),
            Some(VideoCodec::AV1 as u32),
        ]);
        assert_eq!(output_array, expected_array);
    }

    /// Tests that we return the correct error when an unsupported codec is in the data.
    #[test]
    fn test_string_to_codec_uint32_unsupported() {
        let unsupported_codecs = ["vp9"];
        for &bad_codec in &unsupported_codecs {
            let input_array = StringArray::from(vec![Some("h264"), Some(bad_codec)]);
            let result = StringToVideoCodecUInt32::default().transform(&input_array);
            assert!(result.is_err());
            let Err(Error::UnexpectedValue { actual, .. }) = result else {
                panic!("wrong error type");
            };
            assert_eq!(actual, bad_codec);
        }
    }

    /// Tests that all codecs defined in `VideoCodec` are accepted.
    #[test]
    fn test_string_to_codec_uint32_all_supported() {
        let variants = VideoCodec::variants();
        let variant_names = variants
            .iter()
            .map(|v| format!("{v:?}").to_lowercase())
            .collect::<Vec<String>>();
        let input_array = StringArray::from(
            variant_names
                .iter()
                .map(|name| Some(name.as_str()))
                .collect::<Vec<Option<&str>>>(),
        );
        let output_array = StringToVideoCodecUInt32::default()
            .transform(&input_array)
            .expect("transformation failed - are all variants of VideoCodec supported?");
        let expected_array = UInt32Array::from(
            variants
                .iter()
                .map(|v| Some(*v as u32))
                .collect::<Vec<Option<u32>>>(),
        );
        assert_eq!(output_array, expected_array);
    }
}
