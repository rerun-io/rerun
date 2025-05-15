use std::sync::Arc;

use arrow::{
    array::{ArrayRef, BooleanArray},
    datatypes::DataType,
};

/// Construct a default array for a given type, with a given length.
///
/// * bool: false
/// * numbers: 0
/// * string: ""
pub fn default_constructor_for_type(datatype: &DataType) -> Option<Box<dyn Fn(usize) -> ArrayRef>> {
    match datatype {
        DataType::Boolean => Some(Box::new(|length| {
            Arc::new(BooleanArray::from(vec![false; length]))
        })),

        DataType::Int8 => Some(Box::new(|length| {
            Arc::new(arrow::array::Int8Array::from(vec![0; length]))
        })),
        DataType::Int16 => Some(Box::new(|length| {
            Arc::new(arrow::array::Int16Array::from(vec![0; length]))
        })),
        DataType::Int32 => Some(Box::new(|length| {
            Arc::new(arrow::array::Int32Array::from(vec![0; length]))
        })),
        DataType::Int64 => Some(Box::new(|length| {
            Arc::new(arrow::array::Int64Array::from(vec![0; length]))
        })),

        DataType::UInt8 => Some(Box::new(|length| {
            Arc::new(arrow::array::UInt8Array::from(vec![0; length]))
        })),
        DataType::UInt16 => Some(Box::new(|length| {
            Arc::new(arrow::array::UInt16Array::from(vec![0; length]))
        })),
        DataType::UInt32 => Some(Box::new(|length| {
            Arc::new(arrow::array::UInt32Array::from(vec![0; length]))
        })),
        DataType::UInt64 => Some(Box::new(|length| {
            Arc::new(arrow::array::UInt64Array::from(vec![0; length]))
        })),

        DataType::Float32 => Some(Box::new(|length| {
            Arc::new(arrow::array::Float32Array::from(vec![0.0; length]))
        })),
        DataType::Float64 => Some(Box::new(|length| {
            Arc::new(arrow::array::Float64Array::from(vec![0.0; length]))
        })),

        DataType::Utf8 => Some(Box::new(|length| {
            Arc::new(arrow::array::StringArray::from(vec![String::new(); length]))
        })),

        _ => None,
    }
}
