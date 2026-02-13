//! Common helper functions for transforming Arrow data in lenses.

use arrow::array::{Float32Array, Float64Array, ListArray, UInt32Array};
use re_arrow_combinators::Transform as _;
use re_arrow_combinators::cast::{ListToFixedSizeList, PrimitiveCast};
use re_arrow_combinators::map::{MapFixedSizeList, MapList};
use re_arrow_combinators::reshape::{RowMajorToColumnMajor, StructToFixedList};
use re_lenses::OpError;

/// Converts a list of structs with `x`, `y`, `z` fields to a list of fixed-size lists with 3 f32 values.
pub fn list_xyz_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, OpError> {
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z"]).then(
        MapFixedSizeList::new(PrimitiveCast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

/// Converts a list of structs with `x`, `y`, `z`, `w` fields to a list of fixed-size lists with 4 f32 values (quaternions).
pub fn list_xyzw_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, OpError> {
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z", "w"]).then(
        MapFixedSizeList::new(PrimitiveCast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

/// Converts 3x3 row-major f64 matrices stored in variable-size lists to column-major f32 fixed-size lists.
pub fn list_3x3_row_major_to_column_major(list_array: &ListArray) -> Result<ListArray, OpError> {
    let pipeline = MapList::new(
        ListToFixedSizeList::new(9)
            .then(RowMajorToColumnMajor::new(3, 3))
            .then(MapFixedSizeList::new(PrimitiveCast::<
                Float64Array,
                Float32Array,
            >::new())),
    );
    Ok(pipeline.transform(list_array)?)
}

/// Converts u32 width and height fields to a `Resolution` component (fixed-size list with two f32 values).
pub fn width_height_to_resolution(list_array: &ListArray) -> Result<ListArray, OpError> {
    let pipeline = MapList::new(StructToFixedList::new(["width", "height"]).then(
        MapFixedSizeList::new(PrimitiveCast::<UInt32Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}
