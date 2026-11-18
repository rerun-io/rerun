//! Helpers for reading and writing the property columns of a segment.

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, AsArray as _, ListArray, RecordBatch, StringArray};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field};

/// The column holding component `component` of the property logged under
/// `__properties/{property_key}`, i.e. `property:{property_key}:{component}`.
pub(super) fn property_column(property_key: &str, component: &str) -> String {
    format!("property:{property_key}:{component}")
}

/// A single-row `List<Utf8>` column holding `values`.
pub(super) fn string_list(values: &[&str]) -> ArrayRef {
    let values = Arc::new(StringArray::from(values.to_vec()));
    let field = Arc::new(Field::new("item", DataType::Utf8, true));
    Arc::new(ListArray::new(
        field,
        OffsetBuffer::from_lengths([values.len()]),
        values,
        None,
    ))
}

/// Return the list-element array for `column` at `row`, handling both `List` and `LargeList`.
pub(super) fn list_values_at(batch: &RecordBatch, row: usize, column: &str) -> Option<ArrayRef> {
    let col = batch.column_by_name(column)?;
    if row >= col.len() {
        return None;
    }
    if let Some(list) = col.as_list_opt::<i32>() {
        return list.is_valid(row).then(|| list.value(row));
    }
    if let Some(list) = col.as_list_opt::<i64>() {
        return list.is_valid(row).then(|| list.value(row));
    }
    None
}

/// The first non-null string of a `Utf8`, `LargeUtf8`, or `Utf8View` array, in order.
///
/// Used to read a scalar string component, which is stored list-wrapped like any other property.
pub(super) fn first_string(values: &dyn Array) -> Option<String> {
    let first = |len: usize, valid: &dyn Fn(usize) -> bool, value: &dyn Fn(usize) -> String| {
        (0..len).find(|&i| valid(i)).map(value)
    };

    if let Some(array) = values.as_string_opt::<i32>() {
        return first(array.len(), &|i| array.is_valid(i), &|i| {
            array.value(i).to_owned()
        });
    }
    if let Some(array) = values.as_string_opt::<i64>() {
        return first(array.len(), &|i| array.is_valid(i), &|i| {
            array.value(i).to_owned()
        });
    }
    if let Some(array) = values.as_string_view_opt() {
        return first(array.len(), &|i| array.is_valid(i), &|i| {
            array.value(i).to_owned()
        });
    }
    None
}
