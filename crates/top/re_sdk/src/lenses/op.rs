//! Provides commonly used transformations of Arrow arrays.
//!
//! These operations should not be exposed publicly, but instead be wrapped by the [`super::ast::Op`] abstraction.

// TODO(grtlr): Eventually we will want to make the types in here compatible with Datafusion UDFs.

use std::sync::Arc;

use arrow::datatypes::Fields;
use re_chunk::{
    ArrowArray as _,
    external::arrow::{
        array::{ListArray, StructArray},
        compute,
        datatypes::{DataType, Field},
    },
};

use super::Error;

/// Extracts a specific field from a struct component within a `ListArray`.
#[derive(Debug)]
pub struct AccessField {
    pub(super) field_name: String,
}

impl AccessField {
    pub fn call(&self, list_array: ListArray) -> Result<ListArray, Error> {
        let (field, offsets, values, nulls) = list_array.into_parts();
        let struct_array = values
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::TypeMismatch {
                actual: field.data_type().clone(),
                // TODO(grtlr): The struct should not actually be empty, but rather
                // contain the field that we're looking for. But we don't know it's
                // type here. When we implement schemas for ops, we probably need
                // to create our own wrapper types (similar to Datafusion).
                expected: DataType::Struct(Fields::empty()),
            })?;

        let column = struct_array
            .column_by_name(&self.field_name)
            .ok_or_else(|| Error::MissingField {
                expected: self.field_name.clone(),
                found: struct_array
                    .fields()
                    .iter()
                    .map(|f| f.name().clone())
                    .collect(),
            })?;

        Ok(ListArray::new(
            Arc::new(Field::new_list_field(column.data_type().clone(), true)),
            offsets,
            column.clone(),
            nulls,
        ))
    }
}

/// Casts the `value_type` (inner array) of a `ListArray` to a different data type.
#[derive(Debug)]
pub struct Cast {
    pub(super) to_inner_type: DataType,
}

impl Cast {
    pub fn call(&self, list_array: ListArray) -> Result<ListArray, Error> {
        let (_field, offsets, ref array, nulls) = list_array.into_parts();
        let res = compute::cast(array, &self.to_inner_type)?;
        Ok(ListArray::new(
            Arc::new(Field::new_list_field(res.data_type().clone(), true)),
            offsets,
            res,
            nulls,
        ))
    }
}
