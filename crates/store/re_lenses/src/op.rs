//! Provides commonly used transformations of Arrow arrays.
//!
//! These operations should not be exposed publicly, but instead be wrapped by the [`crate::Op`] abstraction.

// TODO(grtlr): Eventually we will want to make the types in here compatible with Datafusion UDFs.

use std::sync::Arc;

use arrow::array::{Array as _, ListArray};
use arrow::compute;
use arrow::datatypes::{DataType, Field};
use re_arrow_combinators::map::MapList;
use re_arrow_combinators::reshape::GetField;
use re_arrow_combinators::Transform as _;

/// Errors that occur during low-level operation execution on columns.
#[derive(Debug, thiserror::Error)]
pub enum OpError {
    /// Error from Arrow combinator transformations.
    #[error(transparent)]
    Transform(#[from] re_arrow_combinators::Error),

    /// Error from Arrow operations.
    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    /// Other custom errors.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

/// Extracts a specific field from a struct component within a `ListArray`.
#[derive(Debug)]
pub struct AccessField {
    pub(crate) field_name: String,
}

impl AccessField {
    pub fn call(&self, list_array: &ListArray) -> Result<ListArray, OpError> {
        MapList::new(GetField::new(self.field_name.clone()))
            .transform(list_array)
            .map_err(Into::into)
    }
}

/// Casts the `value_type` (inner array) of a `ListArray` to a different data type.
#[derive(Debug)]
pub struct Cast {
    pub(crate) to_inner_type: DataType,
}

impl Cast {
    pub fn call(&self, list_array: &ListArray) -> Result<ListArray, OpError> {
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
