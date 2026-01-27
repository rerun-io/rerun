//! Transforms that extract and reshape arrays.

use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, FixedSizeListArray, ListArray, StructArray, UInt32Array, UInt64Array,
};
use arrow::buffer::{NullBuffer, OffsetBuffer};
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
        let field_array = source
            .column_by_name(&self.field_name)
            .ok_or_else(|| {
                let available_fields = source.fields().iter().map(|f| f.name().clone()).collect();
                Error::FieldNotFound {
                    field_name: self.field_name.clone(),
                    available_fields,
                }
            })?
            .clone();

        // If the struct has nulls, we need to combine them with the field's nulls
        // because in Arrow, when a struct is null, its fields should also be null
        if let Some(struct_nulls) = source.nulls() {
            let field_data = field_array.to_data();

            // Combine struct nulls with field nulls
            let combined_nulls = if let Some(field_nulls) = field_data.nulls() {
                // Both struct and field have nulls - combine them with AND
                let combined: Vec<bool> = (0..source.len())
                    .map(|i| struct_nulls.is_valid(i) && field_nulls.is_valid(i))
                    .collect();
                NullBuffer::from(combined)
            } else {
                // Only struct has nulls - use those
                struct_nulls.clone()
            };

            let new_data = field_data
                .into_builder()
                .nulls(Some(combined_nulls))
                .build()?;
            Ok(arrow::array::make_array(new_data))
        } else {
            // No struct nulls - just return the field as-is
            Ok(field_array)
        }
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

            let field = Arc::new(Field::new_list_field(
                inner_values.data_type().clone(),
                true,
            ));
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

        let field = Arc::new(Field::new_list_field(
            inner_values.data_type().clone(),
            true,
        ));
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

        let field = Arc::new(Field::new_list_field(element_type, true));

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

/// Explodes a list by scattering each inner element to a separate row.
///
/// Takes a `List<T>` and returns a flattened `List<T>` where each inner element
/// becomes its own row.
///
/// # Example
///
/// - `[[1, 2, 3], [4, 5]]` → `[[1], [2], [3], [4], [5]]` (each element becomes a row)
/// - `[[1, 2], null, [], [3]]` → `[[1], [2], null, [], [3]]` (nulls and empties preserved)
/// - `[[[1, 2], [3]], [[4, 5, 6]]]` → `[[1, 2], [3], [4, 5, 6]]` (flatten one level)
pub struct Explode;

impl Transform for Explode {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &Self::Source) -> Result<Self::Target, Error> {
        let values_array = source.values();
        let offsets = source.offsets();

        // Compute exact output size: each non-null/non-empty element produces one row,
        // plus one row for each null or empty list
        let mut capacity = 0;
        for i in 0..source.len() {
            let start = offsets[i];
            let end = offsets[i + 1];

            if source.is_null(i) || start == end {
                capacity += 1; // One row for null or empty
            } else {
                capacity += (end - start) as usize; // One row per element
            }
        }

        // Pre-allocate vectors with exact capacity
        let mut indices = Vec::with_capacity(capacity);
        let mut new_offsets = Vec::with_capacity(capacity + 1);
        new_offsets.push(0i32);
        let mut new_validity = Vec::with_capacity(capacity);
        let mut current_offset = 0i32;

        for i in 0..source.len() {
            let start = offsets[i] as usize;
            let end = offsets[i + 1] as usize;

            if source.is_null(i) {
                // Null row: add a null row with no values
                new_validity.push(false);
                new_offsets.push(current_offset);
            } else if start == end {
                // Empty list: add an empty row
                new_validity.push(true);
                new_offsets.push(current_offset);
            } else {
                // Non-empty list: explode each element to its own row
                for j in start..end {
                    indices.push(j as u32);
                    current_offset += 1;
                    new_offsets.push(current_offset);
                    new_validity.push(values_array.is_valid(j));
                }
            }
        }

        // Verify that we calculated the correct size and no reallocation occurred
        debug_assert_eq!(
            new_offsets.len(),
            capacity + 1,
            "new_offsets length mismatch: expected {}, got {}",
            capacity + 1,
            new_offsets.len()
        );
        debug_assert_eq!(
            new_validity.len(),
            capacity,
            "new_validity length mismatch: expected {}, got {}",
            capacity,
            new_validity.len()
        );

        // Extract values using take
        let values = if indices.is_empty() {
            values_array.slice(0, 0)
        } else {
            let indices_array = UInt32Array::from(indices);
            // We explicitly allow `take` here because we care about nulls.
            #[expect(clippy::disallowed_methods)]
            arrow::compute::take(values_array.as_ref(), &indices_array, None)?
        };

        let field = Arc::new(Field::new_list_field(source.value_type(), true));
        Ok(ListArray::new(
            field,
            OffsetBuffer::new(new_offsets.into()),
            values,
            Some(NullBuffer::from(new_validity)),
        ))
    }
}

/// Reorders a `FixedSizeListArray`, where each `FixedSizeList` stores matrix elements
/// in flat row-major order, to `FixedSizeList`s in column-major order.
///
/// The source array is expected to have a value length of `output_rows * output_columns`.
#[derive(Clone, Debug)]
pub struct RowMajorToColumnMajor {
    output_rows: usize,
    output_columns: usize,
    permutation_per_list: Vec<usize>,
}

impl RowMajorToColumnMajor {
    /// Create a new row-major to column-major transformation for the desired output shape.
    pub fn new(output_rows: usize, output_columns: usize) -> Self {
        let mut permutation = Vec::with_capacity(output_rows * output_columns);
        for column in 0..output_columns {
            for row in 0..output_rows {
                let row_major_pos = row * output_columns + column;
                permutation.push(row_major_pos);
            }
        }
        Self {
            output_rows,
            output_columns,
            permutation_per_list: permutation,
        }
    }
}

impl Transform for RowMajorToColumnMajor {
    type Source = FixedSizeListArray;
    type Target = FixedSizeListArray;

    fn transform(&self, source: &Self::Source) -> Result<Self::Target, Error> {
        // First, check that the input array has the expected value length.
        let expected_list_size = self.output_rows * self.output_columns;
        let value_length = source.value_length() as usize;
        if value_length != expected_list_size {
            return Err(Error::UnexpectedListValueLength {
                expected: expected_list_size,
                actual: value_length,
            });
        }

        // Create indices for extracting column-major values as row-major, for all input lists.
        let total_values = source.values().len();
        let indices_to_take: UInt64Array = (0..total_values)
            .map(|value_index| {
                let list_index = value_index / expected_list_size;
                let value_index_within_list = value_index % expected_list_size;
                let next_index_to_take = list_index * expected_list_size
                    + self.permutation_per_list[value_index_within_list];
                next_index_to_take as u64
            })
            .collect();

        // Reorder values into a new FixedSizeListArray.
        // We explicitly allow `take` here because we care about nulls.
        #[expect(clippy::disallowed_methods)]
        let reordered_values = arrow::compute::take(source.values(), &indices_to_take, None)?;

        let field = Arc::new(Field::new_list_field(
            source.value_type().clone(),
            source.is_nullable(),
        ));
        Ok(FixedSizeListArray::new(
            field,
            source.value_length(),
            reordered_values,
            source.nulls().cloned(),
        ))
    }
}

/// Extracts a single element at a specific index from a list array.
///
/// For `ListArray`, this returns the element at the given index from each list row.
/// If the index is out of bounds for a particular row, the result for that row will be null.
///
/// # Example
///
/// - `[[1, 2, 3], [4, 5]]` with index 1 → `[2, 5]`
/// - `[[1, 2], [3]]` with index 1 → `[2, null]` (second list too short)
/// - `null` → `null` (null rows produce null results)
#[derive(Clone, Debug)]
pub struct GetIndexList {
    index: u64,
}

impl GetIndexList {
    /// Create a new index extractor for the given index.
    pub fn new(index: u64) -> Self {
        Self { index }
    }
}

impl Transform for GetIndexList {
    type Source = ListArray;
    type Target = ArrayRef;

    fn transform(&self, source: &ListArray) -> Result<ArrayRef, Error> {
        let offsets = source.offsets();
        let values = source.values();

        // If values is empty, all lists are empty, so all results are null.
        if values.is_empty() {
            return Ok(arrow::array::new_null_array(
                values.data_type(),
                source.len(),
            ));
        }

        // Collect indices to extract from the values array
        let mut indices = Vec::with_capacity(source.len());
        let mut validity = Vec::with_capacity(source.len());

        for row_idx in 0..source.len() {
            if source.is_null(row_idx) {
                // Null row produces null result
                indices.push(0u64); // Placeholder index (will be marked null)
                validity.push(false);
            } else {
                let start = offsets[row_idx];
                let end = offsets[row_idx + 1];
                let length = end - start;

                if self.index < length as u64 {
                    // Index is within bounds
                    indices.push(start as u64 + self.index);
                    validity.push(true);
                } else {
                    // Index out of bounds produces null
                    indices.push(0u64); // Placeholder index
                    validity.push(false);
                }
            }
        }

        // Extract values using take
        let indices_array = UInt64Array::from(indices);
        let options = arrow::compute::TakeOptions { check_bounds: true };
        // We explicitly allow `take` here because we care about nulls.
        #[expect(clippy::disallowed_methods)]
        let mut result = arrow::compute::take(values.as_ref(), &indices_array, Some(options))?;

        // Combine the validity mask with existing nulls from the values array
        // The validity mask marks out-of-bounds and null list rows as null
        // We need to intersect this with nulls from the source values
        let validity_buffer = NullBuffer::from(validity);
        let result_data = result.to_data();
        let combined_nulls = match result_data.nulls() {
            Some(existing_nulls) => {
                // Intersect existing nulls with our validity mask using bitwise AND
                let combined_buffer = existing_nulls.inner() & validity_buffer.inner();
                Some(NullBuffer::new(combined_buffer))
            }
            None => Some(validity_buffer),
        };

        let new_data = result_data.into_builder().nulls(combined_nulls).build()?;
        result = arrow::array::make_array(new_data);

        Ok(result)
    }
}
