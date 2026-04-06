//! Basic transforms for common operations.

use std::sync::Arc;

use arrow::array::{ArrayRef, FixedSizeListArray, StructArray};
use arrow::datatypes::{DataType, Field};

use re_lenses_core::combinators::{Error, StructToFixedList, Transform as _};

/// Extracts named fields from a struct, packs them into a [`FixedSizeListArray`],
/// and casts the element type to `Float32`.
pub fn struct_to_fixed_size_list_f32(
    field_names: impl IntoIterator<Item = impl Into<String>>,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    let field_names: Vec<String> = field_names.into_iter().map(Into::into).collect();
    move |source: &ArrayRef| {
        let struct_array = source
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "StructArray".to_owned(),
                actual: source.data_type().clone(),
                context: "struct_to_fixed_size_list_f32 input".to_owned(),
            })?;
        let fixed = StructToFixedList::new(field_names.iter().map(String::as_str))
            .transform(struct_array)?;
        match fixed {
            Some(arr) => {
                let (_field, size, values, nulls) = arr.into_parts();
                let cast_values = arrow::compute::cast(&values, &DataType::Float32)?;
                let new_field = Arc::new(Field::new_list_field(
                    DataType::Float32,
                    cast_values.is_nullable(),
                ));
                Ok(Some(
                    Arc::new(FixedSizeListArray::new(new_field, size, cast_values, nulls))
                        as ArrayRef,
                ))
            }
            None => Ok(None),
        }
    }
}

/// Creates a transform that casts the input array to a new [`DataType`].
pub fn cast(
    to_type: DataType,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let cast_values = arrow::compute::cast(source, &to_type)?;
        Ok(Some(cast_values))
    }
}
