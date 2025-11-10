//! Custom conversion functions for the Foxglove lenses example.

use arrow::array::{Float32Array, Float64Array, ListArray};

// `re_arrow_combinators` provides the building blocks from which we compose the conversions.
use re_arrow_combinators::{
    Transform as _,
    cast::PrimitiveCast,
    map::MapFixedSizeList,
    map::MapList,
    reshape::StructToFixedList,
    semantic::{BinaryToListUInt8, StringToVideoCodecUInt32, TimeSpecToNanos},
};

use rerun::lenses::Error;

/// Converts a list of binary arrays to a list of uint8 arrays.
pub fn list_binary_to_list_uint8(input: &ListArray) -> Result<ListArray, Error> {
    Ok(MapList::new(BinaryToListUInt8::<i32>::new()).transform(input)?)
}

/// Converts a list of structs with `x`, `y`, `z` fields to a list of fixed-size lists with 3 f32 values.
pub fn list_xyz_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, Error> {
    // Arrow transformations can work on any Arrow-level.
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z"]).then(
        MapFixedSizeList::new(PrimitiveCast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

/// Converts a list of video codec strings to Rerun `VideoCodec` values (as u32).
pub fn list_string_to_list_codec_uint32(list_array: &ListArray) -> Result<ListArray, Error> {
    let pipeline = MapList::new(StringToVideoCodecUInt32::default());
    Ok(pipeline.transform(list_array)?)
}

// Converts a list of structs with i64 `seconds` and i32 `nanos` fields to a list of timestamps in nanoseconds (i64).
pub fn list_timespec_to_list_nanos(list_array: &ListArray) -> Result<ListArray, Error> {
    let pipeline = MapList::new(TimeSpecToNanos::default());
    Ok(pipeline.transform(list_array)?)
}
