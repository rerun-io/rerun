//! Index-based access to list arrays.

use arrow::array::{Array as _, ArrayRef, ListArray, UInt64Array};
use arrow::buffer::NullBuffer;

use crate::{Error, Transform};

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
pub(crate) struct GetIndexList {
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

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{AsArray as _, ListArray};
    use arrow::datatypes::Int32Type;

    #[test]
    fn test_get_index_basic() {
        let input = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
            Some(vec![Some(1), Some(2), Some(3)]),
            Some(vec![Some(4), Some(5)]),
        ]);

        let result = GetIndexList::new(0).transform(&input).unwrap();
        let result_i32 = result.as_primitive::<Int32Type>();

        assert_eq!(result_i32.len(), 2);
        assert_eq!(result_i32.value(0), 1);
        assert_eq!(result_i32.value(1), 4);
    }

    #[test]
    fn test_get_index_out_of_bounds() {
        let input = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
            Some(vec![Some(1), Some(2)]),
            Some(vec![Some(3)]),
            Some(vec![]),
        ]);

        let result = GetIndexList::new(5).transform(&input).unwrap();
        let result_i32 = result.as_primitive::<Int32Type>();

        assert_eq!(result_i32.len(), 3);
        assert!(result_i32.is_null(0)); // Out of bounds
        assert!(result_i32.is_null(1)); // Out of bounds
        assert!(result_i32.is_null(2)); // Empty list
    }

    #[test]
    fn test_get_index_with_nulls() {
        let input = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
            Some(vec![Some(1), Some(2)]),
            None,
            Some(vec![Some(3), None, Some(5)]),
            Some(vec![]),
        ]);

        let result = GetIndexList::new(1).transform(&input).unwrap();
        let result_i32 = result.as_primitive::<Int32Type>();

        assert_eq!(result_i32.len(), 4);
        assert_eq!(result_i32.value(0), 2);
        assert!(result_i32.is_null(1)); // Null row
        assert!(result_i32.is_null(2)); // Null element at index 1
        assert!(result_i32.is_null(3)); // Out of bounds (empty list)
    }
}
