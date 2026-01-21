#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use arrow::array::{
    Array as _, GenericByteBuilder, Int32Array, Int64Array, StringArray, StructArray, UInt32Array,
};
use arrow::datatypes::{DataType, Field, GenericBinaryType};
use re_arrow_combinators::semantic::{
    BinaryToListUInt8, StringToVideoCodecUInt32, TimeSpecToNanos,
};
use re_arrow_combinators::{Error, Transform as _};
use re_sdk_types::components::VideoCodec;
use re_sdk_types::reflection::Enum as _;

mod util;

// Generic test for binary arrays where the offset is the same.
fn impl_binary_test<O1: arrow::array::OffsetSizeTrait, O2: arrow::array::OffsetSizeTrait>() {
    println!(
        "Testing '{}' -> '{}'",
        std::any::type_name::<O1>(),
        std::any::type_name::<O2>()
    );

    let mut builder = GenericByteBuilder::<GenericBinaryType<O1>>::new();
    builder.append_value(b"hello");
    builder.append_value(b"world");
    builder.append_null();
    builder.append_value(b"");
    builder.append_value([0x00, 0xFF, 0x42]);
    let binary_array = builder.finish();

    println!("Input:");
    println!("{}", util::DisplayRB(binary_array.clone()));

    let result = BinaryToListUInt8::<O1, O2>::new()
        .transform(&binary_array)
        .unwrap();

    println!("Output:");
    println!("{}", util::DisplayRB(result.clone()));

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
