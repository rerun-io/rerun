use arrow::array::{
    Array, Float32Array, Float64Array, ListArray, StringArray, UInt32Array, UInt32Builder,
};
use re_arrow_util::transform::{
    BinaryToListUInt8, Cast, MapFixedSizeList, MapList, StructToFixedList, Transform,
};

use rerun::{components::VideoCodec, lenses::Error};

pub fn convert_list_binary_to_list_uint8(input: &ListArray) -> Result<ListArray, Error> {
    Ok(MapList::new(BinaryToListUInt8::<i32>::new()).transform(input)?)
}

pub fn convert_list_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, Error> {
    // Arrow transformations can work on any Arrow-level.
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z"]).then(
        MapFixedSizeList::new(Cast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

pub fn convert_list_string_to_list_codec_uint32(
    list_array: &ListArray,
) -> Result<ListArray, Error> {
    let pipeline = MapList::new(StringToCodecUint32::default());
    Ok(pipeline.transform(list_array)?)
}

/// Transforms a StringArray of Foxglove video codec names to a UInt32Array,
/// where each u32 corresponds to a Rerun VideoCodec enum value.
#[derive(Default)]
struct StringToCodecUint32 {}

impl Transform for StringToCodecUint32 {
    type Source = StringArray;
    type Target = UInt32Array;

    fn transform(
        &self,
        source: &StringArray,
    ) -> Result<Self::Target, re_arrow_util::transform::Error> {
        use re_arrow_util::transform::Error;

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
