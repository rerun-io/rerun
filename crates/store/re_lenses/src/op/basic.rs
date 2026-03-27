//! Basic list-level transforms for common operations.

use std::sync::Arc;

use arrow::array::ListArray;
use arrow::datatypes::{DataType, Field};

use re_lenses_core::combinators::{Error, Transform};

/// Casts the inner values of a [`ListArray`] to a new [`DataType`].
pub struct Cast {
    to_type: DataType,
}

impl Transform for Cast {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &ListArray) -> Result<Option<ListArray>, Error> {
        let (field, offsets, values, nulls) = source.clone().into_parts();
        let cast_values = arrow::compute::cast(&values, &self.to_type)?;
        let new_field = Arc::new(Field::new_list_field(
            self.to_type.clone(),
            field.is_nullable(),
        ));
        Ok(Some(ListArray::new(new_field, offsets, cast_values, nulls)))
    }
}

/// Creates a [`Cast`] transform that casts inner values to the given [`DataType`].
pub fn cast(to_type: DataType) -> Cast {
    Cast { to_type }
}
