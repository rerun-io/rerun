//! Common helper functions for transforming Arrow data in lenses.

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, Float32Array, Float64Array, ListArray, StructArray};
use re_lenses_core::combinators::{
    Error, GetField, ListToFixedSizeList, MapFixedSizeList, PrimitiveCast, RowMajorToColumnMajor,
    StructToFixedList, Transform as _,
};

/// Returns a pipe-compatible function that converts 3x3 row-major f64 matrices stored in variable-size lists to column-major f32 fixed-size lists.
pub fn row_major_3x3_to_column_major()
-> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let list_array =
            source
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| Error::TypeMismatch {
                    expected: "ListArray".to_owned(),
                    actual: source.data_type().clone(),
                    context: "row_major_3x3_to_column_major input".to_owned(),
                })?;
        let transform = ListToFixedSizeList::new(9)
            .then(RowMajorToColumnMajor::new(3, 3))
            .then(MapFixedSizeList::new(PrimitiveCast::<
                Float64Array,
                Float32Array,
            >::new()));
        Ok(transform
            .transform(list_array)?
            .map(|arr| Arc::new(arr) as ArrayRef))
    }
}

/// Extracts a struct field by name and downcasts it to the expected array type.
pub fn get_field_as<T: Array + Clone + 'static>(
    source: &StructArray,
    name: &str,
) -> Result<T, Error> {
    let array_ref = GetField::new(name)
        .transform(source)?
        .ok_or_else(|| Error::FieldNotFound {
            field_name: name.to_owned(),
            available_fields: source.fields().iter().map(|f| f.name().clone()).collect(),
        })?;
    array_ref
        .as_any()
        .downcast_ref::<T>()
        .cloned()
        .ok_or_else(|| Error::TypeMismatch {
            expected: std::any::type_name::<T>().to_owned(),
            actual: array_ref.data_type().clone(),
            context: name.to_owned(),
        })
}

/// Converts a struct with `latitude`, `longitude` fields to a fixed-size list with two f64 values.
pub fn lat_lon_struct_to_fixed()
-> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let struct_array = source
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "StructArray".to_owned(),
                actual: source.data_type().clone(),
                context: "lat_lon_struct_to_fixed input".to_owned(),
            })?;
        // [`re_sdk_types::components::LatLon`] (`DVec2D`) requires non-null f64 fields.
        let transform = StructToFixedList::new(["latitude", "longitude"]).with_nullable(false);
        Ok(transform
            .transform(struct_array)?
            .map(|arr| Arc::new(arr) as ArrayRef))
    }
}
