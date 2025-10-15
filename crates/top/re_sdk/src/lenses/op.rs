//! Provides commonly used transformations of Arrow arrays.
//!
//! These operations should not be exposed publicly, but instead be wrapped by the [`super::ast::Op`] abstraction.

// TODO(grtlr): Eventually we will want to make the types in here compatible with Datafusion UDFs.

use std::sync::Arc;

use re_chunk::{
    ArrowArray as _,
    external::arrow::{
        array::ListArray,
        compute,
        datatypes::{DataType, Field},
    },
};

use super::Error;
use re_arrow_util::transform::{GetField, MapList, Transform as _};

/// Extracts a specific field from a struct component within a `ListArray`.
#[derive(Debug)]
pub struct AccessField {
    pub(super) field_name: String,
}

impl AccessField {
    pub fn call(&self, list_array: &ListArray) -> Result<ListArray, Error> {
        MapList::new(GetField::new(self.field_name.clone()))
            .transform(list_array)
            .map_err(Into::into)
    }
}

/// Casts the `value_type` (inner array) of a `ListArray` to a different data type.
#[derive(Debug)]
pub struct Cast {
    pub(super) to_inner_type: DataType,
}

impl Cast {
    pub fn call(&self, list_array: &ListArray) -> Result<ListArray, Error> {
        let (_field, offsets, ref array, nulls) = list_array.clone().into_parts();
        let res = compute::cast(array, &self.to_inner_type)?;
        Ok(ListArray::new(
            Arc::new(Field::new_list_field(res.data_type().clone(), true)),
            offsets,
            res,
            nulls,
        ))
    }
}
