use arrow::array::{
    Array, Float32Array, Float64Array, Int32Array, Int64Array, ListArray, StringArray, StructArray,
    UInt32Array, UInt32Builder,
};
use re_arrow_combinators::{
    BinaryToListUInt8, Cast, MapFixedSizeList, MapList, StructToFixedList, Transform,
};

use rerun::{components::VideoCodec, lenses::Error};

/// Converts a list of binary arrays to a list of uint8 arrays.
pub fn list_binary_to_list_uint8(input: &ListArray) -> Result<ListArray, Error> {
    Ok(MapList::new(BinaryToListUInt8::<i32>::new()).transform(input)?)
}

/// Converts a list of structs with `x`, `y`, `z` fields to a list of fixed-size lists with 3 f32 values.
pub fn list_xyz_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, Error> {
    // Arrow transformations can work on any Arrow-level.
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z"]).then(
        MapFixedSizeList::new(Cast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

/// Converts a list of video codec strings to Rerun VideoCodec values (as u32).
pub fn list_string_to_list_codec_uint32(list_array: &ListArray) -> Result<ListArray, Error> {
    let pipeline = MapList::new(StringToCodecUInt32::default());
    Ok(pipeline.transform(list_array)?)
}

// Converts a list of structs with i64 `seconds` and i32 `nanos` fields to a list of timestamps in nanoseconds (i64).
pub fn list_seconds_nanos_to_list_nanos(list_array: &ListArray) -> Result<ListArray, Error> {
    let pipeline = MapList::new(SecondsNanosToNanos::default());
    Ok(pipeline.transform(list_array)?)
}

#[derive(Default)]
struct SecondsNanosToNanos {}

impl Transform for SecondsNanosToNanos {
    type Source = StructArray;
    type Target = Int64Array;

    fn transform(&self, source: &StructArray) -> Result<Self::Target, re_arrow_combinators::Error> {
        use re_arrow_combinators::Error;

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

/// Transforms a StringArray of Foxglove video codec names to a UInt32Array,
/// where each u32 corresponds to a Rerun VideoCodec enum value.
#[derive(Default)]
struct StringToCodecUInt32 {}

impl Transform for StringToCodecUInt32 {
    type Source = StringArray;
    type Target = UInt32Array;

    fn transform(&self, source: &StringArray) -> Result<Self::Target, re_arrow_combinators::Error> {
        use re_arrow_combinators::Error;

        let mut output_builder = UInt32Builder::with_capacity(source.len());

        for i in 0..source.len() {
            if source.is_null(i) {
                output_builder.append_null();
            } else {
                // The actual conversion:
                let codec = match source.value(i).to_lowercase().as_str() {
                    "h264" => Ok(VideoCodec::H264),
                    "h265" => Ok(VideoCodec::H265),
                    unsupported => Err(Error::UnexpectedValue {
                        expected: "'h264' or 'h265'".to_owned(),
                        actual: unsupported.to_owned(),
                    }),
                }?;
                output_builder.append_value(codec as u32);
            }
        }

        Ok(output_builder.finish())
    }
}

/// Tests that supported codecs are correctly converted, and checks case-insensitivity and null handling.
#[test]
fn test_string_to_codec_uint32() {
    // Note: mixed codecs normally don't make sense, but should be fine from a pure conversion perspective.
    let input_array = StringArray::from(vec![Some("H264"), None, Some("h264"), Some("H265")]);
    assert_eq!(input_array.null_count(), 1);
    let output_array = StringToCodecUInt32::default()
        .transform(&input_array)
        .expect("transformation failed");
    assert_eq!(output_array.null_count(), 1);
    let expected_array = UInt32Array::from(vec![
        Some(VideoCodec::H264 as u32),
        None,
        Some(VideoCodec::H264 as u32),
        Some(VideoCodec::H265 as u32),
    ]);
    assert_eq!(output_array, expected_array);
}

/// Tests that we return the correct error when an unsupported codec is in the data.
/// See here for possible values: https://docs.foxglove.dev/docs/sdk/schemas/compressed-video#data
#[test]
fn test_string_to_codec_uint32_unsupported() {
    let unsupported_codecs = ["vp9", "av1"];
    for &bad_codec in &unsupported_codecs {
        let input_array = StringArray::from(vec![Some("h264"), Some(bad_codec)]);
        let result = StringToCodecUInt32::default().transform(&input_array);
        assert!(result.is_err());
        let Err(re_arrow_combinators::Error::UnexpectedValue { actual, .. }) = result else {
            panic!("wrong error type");
        };
        assert_eq!(actual, bad_codec);
    }
}
