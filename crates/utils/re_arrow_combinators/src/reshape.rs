//! Transforms that extract and reshape arrays.

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, FixedSizeListArray, ListArray, StructArray};
use arrow::datatypes::Field;

use crate::{Error, Transform};

/// Extracts a field from a struct array.
///
/// Returns the field's array if it exists, otherwise returns an error.
#[derive(Clone)]
pub struct GetField {
    field_name: String,
}

impl GetField {
    /// Create a new field extractor for the given field name.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
        }
    }
}

impl Transform for GetField {
    type Source = StructArray;
    type Target = ArrayRef;

    fn transform(&self, source: &StructArray) -> Result<ArrayRef, Error> {
        source
            .column_by_name(&self.field_name)
            .ok_or_else(|| {
                let available_fields = source.fields().iter().map(|f| f.name().clone()).collect();
                Error::FieldNotFound {
                    field_name: self.field_name.clone(),
                    available_fields,
                }
            })
            .map(Clone::clone)
    }
}

/// Flattens a nested list array by one level.
///
/// Takes `List<List<T>>` and flattens it to `List<T>` by concatenating all inner lists
/// within each outer list row.
///
/// # Example
///
/// - `[[1, 2], [3, 4]]` → `[1, 2, 3, 4]` (concatenates inner lists)
/// - `[[5], [6, 7, 8]]` → `[5, 6, 7, 8]`
/// - `[[]]` → `[]` (empty inner list produces empty result)
/// - `null` → `null` (null rows are preserved)
#[derive(Clone, Debug, Default)]
pub struct Flatten;

impl Flatten {
    /// Create a new flatten transformation.
    pub fn new() -> Self {
        Self
    }
}

impl Transform for Flatten {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &ListArray) -> Result<ListArray, Error> {
        let values = source.values();

        // The values should be a ListArray that we want to flatten
        let inner_list =
            values
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| Error::TypeMismatch {
                    expected: "List".to_owned(),
                    actual: values.data_type().clone(),
                    context: "Flatten expects List<List<T>>".to_owned(),
                })?;

        let outer_offsets = source.offsets();
        let inner_offsets = inner_list.offsets();
        let inner_values = inner_list.values();

        // Fast path: check if each outer list contains at most one inner list
        // In this case, we can just unwrap directly
        let mut is_trivial = true;
        for outer_row_idx in 0..source.len() {
            if !source.is_null(outer_row_idx) {
                let outer_start = outer_offsets[outer_row_idx] as usize;
                let outer_end = outer_offsets[outer_row_idx + 1] as usize;
                let count = outer_end - outer_start;
                if count > 1 {
                    is_trivial = false;
                    break;
                }
            }
        }

        if is_trivial {
            // Each outer list has 0 or 1 inner lists - just unwrap
            // Map outer offsets through inner offsets
            let mut new_offsets = Vec::with_capacity(source.len() + 1);

            for outer_row_idx in 0..=source.len() {
                let outer_idx = outer_offsets[outer_row_idx] as usize;
                let inner_offset = inner_offsets[outer_idx];
                new_offsets.push(inner_offset);
            }

            let field = Arc::new(Field::new("item", inner_values.data_type().clone(), true));
            let offsets = arrow::buffer::OffsetBuffer::new(new_offsets.into());

            return Ok(ListArray::new(
                field,
                offsets,
                inner_values.clone(),
                source.nulls().cloned(),
            ));
        }

        // General case: build new offsets and collect value ranges
        let mut new_offsets = Vec::with_capacity(source.len() + 1);
        new_offsets.push(0i32);

        let mut current_offset = 0i32;

        // Collect ranges of values to copy (as (start, length) pairs)
        let mut value_ranges: Vec<(i32, i32)> = Vec::new();

        for outer_row_idx in 0..source.len() {
            if source.is_null(outer_row_idx) {
                new_offsets.push(current_offset);
                continue;
            }

            let outer_start = outer_offsets[outer_row_idx];
            let outer_end = outer_offsets[outer_row_idx + 1];

            for inner_idx in outer_start..outer_end {
                let inner_idx = inner_idx as usize;
                if !inner_list.is_null(inner_idx) {
                    let inner_start = inner_offsets[inner_idx];
                    let inner_end = inner_offsets[inner_idx + 1];
                    let length = inner_end - inner_start;

                    if length > 0 {
                        // Try to merge with previous range if contiguous
                        if let Some((last_start, last_len)) = value_ranges.last_mut() {
                            if *last_start + *last_len == inner_start {
                                *last_len += length;
                            } else {
                                value_ranges.push((inner_start, length));
                            }
                        } else {
                            value_ranges.push((inner_start, length));
                        }
                        current_offset += length;
                    }
                }
            }

            new_offsets.push(current_offset);
        }

        // Build flattened values by slicing larger contiguous chunks
        let flattened_values = if value_ranges.is_empty() {
            inner_values.slice(0, 0)
        } else if value_ranges.len() == 1 {
            // Single contiguous range - just slice once
            let (start, length) = value_ranges[0];
            inner_values.slice(start as usize, length as usize)
        } else {
            // Multiple ranges - slice and concatenate
            let slices: Vec<_> = value_ranges
                .iter()
                .map(|&(start, length)| inner_values.slice(start as usize, length as usize))
                .collect();
            let refs: Vec<&dyn Array> = slices.iter().map(|a| a.as_ref()).collect();
            re_arrow_util::concat_arrays(&refs)?
        };

        let field = Arc::new(Field::new("item", inner_values.data_type().clone(), true));
        let offsets = arrow::buffer::OffsetBuffer::new(new_offsets.into());

        Ok(ListArray::new(
            field,
            offsets,
            flattened_values,
            source.nulls().cloned(),
        ))
    }
}

/// Converts a struct to a fixed-size list array by extracting specified fields.
///
/// This transformation takes a list of field names and extracts them from each struct,
/// packing them into a fixed-size list. The size of the list equals the number of field names.
///
/// Null handling: Individual field values can be null (represented as null in the flattened array),
/// but the outer list entries are never null - missing fields result in null values in the list.
#[derive(Clone)]
pub struct StructToFixedList {
    field_names: Vec<String>,
}

impl StructToFixedList {
    /// Create a new struct-to-fixed-list transformer.
    ///
    /// The field names specify which fields to extract and in what order.
    /// The resulting fixed-size list will have length equal to `field_names.len()`.
    pub fn new(field_names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            field_names: field_names.into_iter().map(|s| s.into()).collect(),
        }
    }
}

impl Transform for StructToFixedList {
    type Source = StructArray;
    type Target = FixedSizeListArray;

    fn transform(&self, source: &StructArray) -> Result<FixedSizeListArray, Error> {
        if self.field_names.is_empty() {
            return Err(Error::NoFieldNames);
        }

        // Get the first field to determine the element type
        let first_field_name = &self.field_names[0];
        let first_array = GetField::new(first_field_name).transform(source)?;
        let element_type = first_array.data_type().clone();

        // Collect all field arrays, ensuring they all have the same type
        let mut field_arrays = Vec::new();
        field_arrays.push(first_array);

        for field_name in &self.field_names[1..] {
            let array = GetField::new(field_name).transform(source)?;

            // Verify type consistency
            if array.data_type() != &element_type {
                return Err(Error::InconsistentFieldTypes {
                    field_name: field_name.clone(),
                    actual_type: array.data_type().clone(),
                    reference_field: first_field_name.clone(),
                    expected_type: element_type.clone(),
                });
            }

            field_arrays.push(array);
        }

        // Build the flattened values array by concatenating field arrays
        let mut concatenated_arrays = Vec::new();
        for row_idx in 0..source.len() {
            for field_array in &field_arrays {
                concatenated_arrays.push(field_array.slice(row_idx, 1));
            }
        }

        // Concatenate all slices into a single array
        let refs: Vec<&dyn Array> = concatenated_arrays.iter().map(|a| a.as_ref()).collect();
        let values = re_arrow_util::concat_arrays(&refs)?;

        let field = Arc::new(Field::new("item", element_type, true));

        let list_size = self.field_names.len();
        let list_size = i32::try_from(list_size).map_err(|err| Error::InvalidNumberOfFields {
            actual: list_size,
            err,
        })?;
        Ok(FixedSizeListArray::new(
            field, list_size, values, None, // No outer nulls
        ))
    }
}
