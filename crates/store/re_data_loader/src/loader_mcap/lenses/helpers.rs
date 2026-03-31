//! Common helper functions for transforming Arrow data in lenses.

use arrow::array::{
    Array, FixedSizeListArray, Float32Array, Float64Array, ListArray, StructArray, UInt32Array,
};
use re_lenses_core::combinators::{
    GetField, ListToFixedSizeList, MapFixedSizeList, MapList, PrimitiveCast, RowMajorToColumnMajor,
    StructToFixedList, Transform,
};

/// Returns a transform that converts a struct with `x`, `y`, `z` fields to a fixed-size list of 3 f32 values.
pub fn xyz_struct_to_fixed() -> impl Transform<Source = StructArray, Target = FixedSizeListArray> {
    StructToFixedList::new(["x", "y", "z"]).then(MapFixedSizeList::new(PrimitiveCast::<
        Float64Array,
        Float32Array,
    >::new()))
}

/// Returns a transform that converts a struct with `x`, `y`, `z`, `w` fields to a fixed-size list of 4 f32 values (quaternions).
pub fn xyzw_struct_to_fixed() -> impl Transform<Source = StructArray, Target = FixedSizeListArray> {
    StructToFixedList::new(["x", "y", "z", "w"]).then(MapFixedSizeList::new(PrimitiveCast::<
        Float64Array,
        Float32Array,
    >::new()))
}

/// Returns a transform that converts u32 width and height fields to a Resolution component (fixed-size list of 2 f32 values).
pub fn width_height_to_resolution()
-> impl Transform<Source = StructArray, Target = FixedSizeListArray> {
    StructToFixedList::new(["width", "height"]).then(MapFixedSizeList::new(PrimitiveCast::<
        UInt32Array,
        Float32Array,
    >::new()))
}

/// Returns a transform that converts 3x3 row-major f64 matrices stored in variable-size lists to column-major f32 fixed-size lists.
pub fn row_major_3x3_to_column_major()
-> impl Transform<Source = ListArray, Target = FixedSizeListArray> {
    ListToFixedSizeList::new(9)
        .then(RowMajorToColumnMajor::new(3, 3))
        .then(MapFixedSizeList::new(PrimitiveCast::<
            Float64Array,
            Float32Array,
        >::new()))
}

/// Extracts a struct field by name and downcasts it to the expected array type.
pub fn get_field_as<T: Array + Clone + 'static>(
    source: &StructArray,
    name: &str,
) -> Result<T, re_lenses_core::combinators::Error> {
    let array_ref = GetField::new(name).transform(source)?.ok_or_else(|| {
        re_lenses_core::combinators::Error::FieldNotFound {
            field_name: name.to_owned(),
            available_fields: source.fields().iter().map(|f| f.name().clone()).collect(),
        }
    })?;
    array_ref
        .as_any()
        .downcast_ref::<T>()
        .cloned()
        .ok_or_else(|| re_lenses_core::combinators::Error::TypeMismatch {
            expected: std::any::type_name::<T>().to_owned(),
            actual: array_ref.data_type().clone(),
            context: name.to_owned(),
        })
}

/// Converts a list of structs with `latitude`, `longitude` fields to a list of fixed-size lists with two f64 values.
pub fn list_lat_lon_struct_to_list_fixed() -> impl Transform<Source = ListArray, Target = ListArray>
{
    // [`re_sdk_types::components::LatLon`] (`DVec2D`) requires non-null f64 fields.
    MapList::new(StructToFixedList::new(["latitude", "longitude"]).with_nullable(false))
}
