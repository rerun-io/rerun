//! Semantic array transforms for concrete applications.

use std::marker::PhantomData;
use std::sync::Arc;

use arrow::array::{
    Array as _, ArrowNativeTypeOp as _, AsArray as _, GenericBinaryArray, GenericListArray,
    Int64Array, OffsetSizeTrait, StringArray, StructArray, UInt32Array, UInt32Builder,
};
use arrow::datatypes::{DataType, Field, Float64Type, Int32Type, Int64Type};
use arrow::error::ArrowError;
use re_sdk_types::components::VideoCodec;

use re_lenses_core::combinators::{DowncastRef, Error, GetField, Transform};

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

    fn transform(&self, source: &GenericBinaryArray<O1>) -> Result<Option<Self::Target>, Error> {
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

        Ok(Some(list))
    }
}

/// Converts `StructArray` of timestamps with `seconds`/`nanos` or `sec`/`nsec` fields (i64/i32)
/// to `Int64Array` containing the corresponding total nanoseconds timestamps.
#[derive(Default)]
pub struct TimeSpecToNanos {}

impl TimeSpecToNanos {
    /// Extracts a struct field from different possible field name variants,
    /// by trying each name in order. Casts to the target primitive type.
    fn get_field_from_variants<TargetType: arrow::array::ArrowPrimitiveType>(
        source: &StructArray,
        field_names: &[&str],
    ) -> Result<Option<arrow::array::PrimitiveArray<TargetType>>, Error> {
        for &name in field_names {
            if let Ok(Some(array_ref)) = GetField::new(name).transform(source) {
                let casted = arrow::compute::cast(&array_ref, &TargetType::DATA_TYPE)?;
                let downcasted = DowncastRef::<TargetType>::new().transform(&casted)?;

                re_log::debug_assert!(
                    downcasted.is_some(),
                    "downcasting directly after casting should not fail"
                );

                return Ok(downcasted);
            }
        }
        Err(Error::FieldNotFound {
            field_name: field_names.join(" | "),
            available_fields: source.fields().iter().map(|f| f.name().clone()).collect(),
        })
    }
}

impl Transform for TimeSpecToNanos {
    type Source = StructArray;
    type Target = Int64Array;

    fn transform(&self, source: &StructArray) -> Result<Option<Self::Target>, Error> {
        let (Some(seconds_array), Some(nanos_array)) = (
            Self::get_field_from_variants::<Int64Type>(source, &["seconds", "sec"])?,
            Self::get_field_from_variants::<Int32Type>(source, &["nanos", "nsec"])?,
        ) else {
            return Ok(None);
        };

        Ok(Some(arrow::compute::try_binary(
            &seconds_array,
            &nanos_array,
            |seconds: i64, nanos: i32| -> Result<i64, ArrowError> {
                seconds
                    .mul_checked(1_000_000_000)?
                    .add_checked(nanos as i64)
            },
        )?))
    }
}

/// Transforms a `StringArray` of video codec names to a `UInt32Array`,
/// where each u32 corresponds to a Rerun `VideoCodec` enum value.
#[derive(Default)]
pub struct StringToVideoCodecUInt32 {}

impl Transform for StringToVideoCodecUInt32 {
    type Source = StringArray;
    type Target = UInt32Array;

    fn transform(&self, source: &StringArray) -> Result<Option<Self::Target>, Error> {
        Ok(Some(
            source
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
                .finish(),
        ))
    }
}

/// Converts RGBA structs (r, g, b, a as f32 or f64 in 0..1) to packed RGBA u32 values.
#[derive(Default)]
pub struct RgbaStructToUInt32 {}

impl Transform for RgbaStructToUInt32 {
    type Source = StructArray;
    type Target = UInt32Array;

    fn transform(&self, source: &StructArray) -> Result<Option<Self::Target>, Error> {
        // Helper to extract a color channel field, supporting both f32 and f64.
        let get_channel = |name: &str| -> Result<_, Error> {
            let field =
                GetField::new(name)
                    .transform(source)?
                    .ok_or_else(|| Error::FieldNotFound {
                        field_name: name.to_owned(),
                        available_fields: source
                            .fields()
                            .iter()
                            .map(|f| f.name().to_owned())
                            .collect(),
                    })?;
            Ok(arrow::compute::cast(&field, &DataType::Float64)?
                .as_primitive::<Float64Type>()
                .clone())
        };

        let r = get_channel("r")?;
        let g = get_channel("g")?;
        let b = get_channel("b")?;
        let a = get_channel("a")?;

        let result: UInt32Array = (0..source.len())
            .map(|i| {
                if source.is_null(i) {
                    None
                } else {
                    let rv = (r.value(i).clamp(0.0, 1.0) * 255.0).round() as u32;
                    let gv = (g.value(i).clamp(0.0, 1.0) * 255.0).round() as u32;
                    let bv = (b.value(i).clamp(0.0, 1.0) * 255.0).round() as u32;
                    let av = (a.value(i).clamp(0.0, 1.0) * 255.0).round() as u32;
                    Some((rv << 24) | (gv << 16) | (bv << 8) | av)
                }
            })
            .collect();

        Ok(Some(result))
    }
}

/// Converts binary data (i32 offsets) to a list of `u8` values.
pub fn binary_to_list_uint8() -> BinaryToListUInt8<i32> {
    BinaryToListUInt8::new()
}

/// Converts a timestamp struct (`seconds`/`nanos`) to nanoseconds.
pub fn timespec_to_nanos() -> TimeSpecToNanos {
    TimeSpecToNanos::default()
}

/// Converts video codec name strings to `VideoCodec` enum values.
pub fn string_to_video_codec() -> StringToVideoCodecUInt32 {
    StringToVideoCodecUInt32::default()
}

/// Converts structs with r, g, b, a fields to packed RGBA u32 values.
///
/// Supports both f32 and f64 field types, and handles clamping and nulls.
pub fn rgba_struct_to_uint32() -> RgbaStructToUInt32 {
    RgbaStructToUInt32::default()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{
        Array as _, Float32Array, Float64Array, GenericByteBuilder, Int32Array, Int64Array,
        StringArray, StructArray, UInt32Array,
    };
    use arrow::datatypes::{DataType, Field, GenericBinaryType};
    use re_lenses_core::combinators::{Error, Transform as _};
    use re_sdk_types::components::VideoCodec;
    use re_sdk_types::reflection::Enum as _;

    use super::*;

    // Generic test for binary arrays where the offset is the same.
    fn impl_binary_test<O1: OffsetSizeTrait, O2: OffsetSizeTrait>() -> Result<(), Error> {
        let mut builder = GenericByteBuilder::<GenericBinaryType<O1>>::new();
        builder.append_value(b"hello");
        builder.append_value(b"world");
        builder.append_null();
        builder.append_value(b"");
        builder.append_value([0x00, 0xFF, 0x42]);
        let binary_array = builder.finish();

        let result = BinaryToListUInt8::<O1, O2>::new()
            .transform(&binary_array)?
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

        Ok(())
    }

    #[test]
    fn test_binary_to_list_uint8() -> Result<(), Error> {
        // We test the different offset combinations.
        impl_binary_test::<i32, i32>()?;
        impl_binary_test::<i64, i32>()?;
        impl_binary_test::<i32, i64>()?;
        impl_binary_test::<i64, i64>()?;

        Ok(())
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
    fn test_timespec_to_nanos() -> Result<(), Error> {
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
            .transform(&struct_array)?
            .unwrap();
        let expected_array = Int64Array::from(vec![
            Some(1_500_000_000),
            None,
            None,
            Some(3_250_000_000),
            None,
        ]);
        assert_eq!(output_array, expected_array);

        Ok(())
    }

    /// Tests that timespec structs with `sec`/`nsec` field names work too.
    #[test]
    fn test_timespec_to_nanos_sec_nsec() -> Result<(), Error> {
        let seconds_field = Arc::new(Field::new("sec", DataType::Int64, true));
        let nanos_field = Arc::new(Field::new("nsec", DataType::Int32, true));

        let seconds_array = Arc::new(Int64Array::from(vec![Some(1), Some(2)]));
        let nanos_array = Arc::new(Int32Array::from(vec![Some(500_000_000), Some(0)]));

        let struct_array = StructArray::new(
            vec![seconds_field, nanos_field].into(),
            vec![seconds_array, nanos_array],
            None,
        );
        let output_array = TimeSpecToNanos::default()
            .transform(&struct_array)?
            .unwrap();
        let expected_array = Int64Array::from(vec![Some(1_500_000_000), Some(2_000_000_000)]);
        assert_eq!(output_array, expected_array);

        Ok(())
    }

    /// Tests that timespec with uint32 seconds and nanos fields are cast correctly.
    #[test]
    fn test_timespec_to_nanos_uint32() -> Result<(), Error> {
        let seconds_field = Arc::new(Field::new("sec", DataType::UInt32, false));
        let nanos_field = Arc::new(Field::new("nsec", DataType::UInt32, false));

        let seconds_array = Arc::new(UInt32Array::from(vec![1u32, 2]));
        let nanos_array = Arc::new(UInt32Array::from(vec![500_000_000u32, 0]));

        let struct_array = StructArray::new(
            vec![seconds_field, nanos_field].into(),
            vec![seconds_array, nanos_array],
            None,
        );
        let output_array = TimeSpecToNanos::default()
            .transform(&struct_array)?
            .unwrap();
        let expected_array = Int64Array::from(vec![1_500_000_000i64, 2_000_000_000]);
        assert_eq!(output_array, expected_array);

        Ok(())
    }

    /// Tests that supported codecs are correctly converted, and checks case-insensitivity and null handling.
    #[test]
    fn test_string_to_codec_uint32() -> Result<(), Error> {
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
            .transform(&input_array)?
            .unwrap();
        assert_eq!(output_array.null_count(), 1);
        let expected_array = UInt32Array::from(vec![
            Some(VideoCodec::H264 as u32),
            None,
            Some(VideoCodec::H264 as u32),
            Some(VideoCodec::H265 as u32),
            Some(VideoCodec::AV1 as u32),
        ]);
        assert_eq!(output_array, expected_array);

        Ok(())
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
    fn test_string_to_codec_uint32_all_supported() -> Result<(), Error> {
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
            .transform(&input_array)?
            .unwrap();
        let expected_array = UInt32Array::from(
            variants
                .iter()
                .map(|v| Some(*v as u32))
                .collect::<Vec<Option<u32>>>(),
        );
        assert_eq!(output_array, expected_array);

        Ok(())
    }

    /// Helper to build an RGBA struct array from channel arrays of a given type.
    fn make_rgba_struct<T: arrow::array::Array + 'static>(
        r: T,
        g: T,
        b: T,
        a: T,
        nulls: Option<arrow::buffer::NullBuffer>,
    ) -> StructArray {
        let dt = r.data_type().clone();
        StructArray::new(
            vec![
                Arc::new(Field::new("r", dt.clone(), true)),
                Arc::new(Field::new("g", dt.clone(), true)),
                Arc::new(Field::new("b", dt.clone(), true)),
                Arc::new(Field::new("a", dt, true)),
            ]
            .into(),
            vec![Arc::new(r), Arc::new(g), Arc::new(b), Arc::new(a)],
            nulls,
        )
    }

    /// Tests RGBA conversion with f64 fields, including clamping and null handling.
    #[test]
    fn test_rgba_struct_to_uint32_f64() {
        let nulls = arrow::buffer::NullBuffer::from(vec![true, true, false, true]);
        let struct_array = make_rgba_struct(
            Float64Array::from(vec![1.0, 0.0, 0.0, 1.5]),
            Float64Array::from(vec![0.5, 0.0, 0.0, -0.1]),
            Float64Array::from(vec![0.0, 1.0, 0.0, 0.0]),
            Float64Array::from(vec![1.0, 0.5, 0.0, 1.0]),
            Some(nulls),
        );
        let output = RgbaStructToUInt32::default()
            .transform(&struct_array)
            .expect("transformation failed");
        let expected = UInt32Array::from(vec![
            Some(0xFF80_00FF), // r=255, g=128, b=0, a=255
            Some(0x0000_FF80), // r=0, g=0, b=255, a=128
            None,              // struct-level null
            Some(0xFF00_00FF), // clamped: r=255, g=0, b=0, a=255
        ]);
        assert_eq!(output, Some(expected));
    }

    /// Tests RGBA conversion with f32 fields, including clamping and null handling.
    #[test]
    fn test_rgba_struct_to_uint32_f32() {
        let nulls = arrow::buffer::NullBuffer::from(vec![true, false, true, true]);
        let struct_array = make_rgba_struct(
            Float32Array::from(vec![1.0f32, 0.0, 0.0, 1.5]),
            Float32Array::from(vec![0.0f32, 0.0, 1.0, -0.1]),
            Float32Array::from(vec![0.0f32, 0.0, 0.0, 0.0]),
            Float32Array::from(vec![1.0f32, 0.0, 1.0, 1.0]),
            Some(nulls),
        );
        let output = RgbaStructToUInt32::default()
            .transform(&struct_array)
            .expect("transformation failed with f32 input");
        let expected = UInt32Array::from(vec![
            Some(0xFF00_00FF), // red
            None,              // struct-level null
            Some(0x00FF_00FF), // green
            Some(0xFF00_00FF), // clamped: r=255, g=0, b=0, a=255
        ]);
        assert_eq!(output, Some(expected));
    }
}
