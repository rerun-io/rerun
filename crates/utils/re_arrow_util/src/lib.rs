//! Helpers for working with arrow

mod arrays;
mod batches;
mod compare;
mod format;
mod format_data_type;
mod string_view;
mod test_extensions;

// ----------------------------------------------------------------
use std::sync::Arc;

use arrow::array::{Array as _, AsArray as _, ListArray};
use arrow::datatypes::{DataType, Field};

pub use self::arrays::*;
pub use self::batches::*;
pub use self::compare::*;
pub use self::format::{
    RecordBatchFormatOpts, format_record_batch, format_record_batch_opts,
    format_record_batch_with_width,
};
pub use self::format_data_type::*;
pub use self::string_view::*;
pub use self::test_extensions::*;

/// Convert any `BinaryArray` to `LargeBinaryArray`, because we treat them logically the same
pub fn widen_binary_arrays(list_array: &ListArray) -> ListArray {
    let list_data_type = list_array.data_type();
    if let DataType::List(field) = list_data_type
        && field.data_type() == &DataType::Binary
    {
        re_tracing::profile_function!();
        let large_binary_field = Field::new("item", DataType::LargeBinary, true);
        let target_type = DataType::List(Arc::new(large_binary_field));

        #[expect(clippy::unwrap_used)]
        arrow::compute::kernels::cast::cast(list_array, &target_type)
            .unwrap()
            .as_list()
            .clone()
    } else {
        list_array.clone()
    }
}

#[cfg(test)]
mod tests {
    use arrow::array::{BinaryBuilder, ListBuilder};

    use super::*;

    #[test]
    fn test_widen_list_binary() {
        // Create test data
        let mut list_builder = ListBuilder::new(BinaryBuilder::new());

        // First list: [b"hello", b"world"]
        list_builder.values().append_value(b"hello");
        list_builder.values().append_value(b"world");
        list_builder.append(true);

        // Second list: [b"rust", b"arrow"]
        list_builder.values().append_value(b"rust");
        list_builder.values().append_value(b"arrow");
        list_builder.append(true);

        // Third list: null
        list_builder.append_null();

        let original_list = list_builder.finish();

        // Widen to LargeBinaryArray
        let widened_list = widen_binary_arrays(&original_list);

        // Verify the result
        assert_eq!(widened_list.len(), 3);
        assert!(!widened_list.is_null(0));
        assert!(!widened_list.is_null(1));
        assert!(widened_list.is_null(2));

        // Check data type
        if let DataType::List(field) = widened_list.data_type() {
            assert_eq!(field.data_type(), &DataType::LargeBinary);
        } else {
            panic!("Expected List data type");
        }
    }
}

// ----------------------------------------------------------------

/// Error used when a column is missing from a record batch
#[derive(Debug, Clone, thiserror::Error)]
pub struct MissingColumnError {
    pub missing: String,
    pub available: Vec<String>,
}

impl std::fmt::Display for MissingColumnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { missing, available } = self;
        write!(f, "Missing column: {missing:?}. Available: {available:?}")
    }
}

// ----------------------------------------------------------------

/// Error used for arrow datatype mismatch.
#[derive(Debug, Clone, thiserror::Error)]
pub struct WrongDatatypeError {
    pub column_name: Option<String>,
    pub expected: Box<DataType>,
    pub actual: Box<DataType>,
}

impl WrongDatatypeError {
    pub fn ensure_datatype(field: &Field, expected: &DataType) -> Result<(), Self> {
        if field.data_type() == expected {
            Ok(())
        } else {
            Err(Self {
                column_name: Some(field.name().to_owned()),
                expected: expected.clone().into(),
                actual: field.data_type().clone().into(),
            })
        }
    }
}

impl std::fmt::Display for WrongDatatypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            column_name,
            expected,
            actual,
        } = self;
        if let Some(column_name) = column_name {
            write!(
                f,
                "Expected column {column_name:?} to be {expected}, got {actual}"
            )
        } else {
            write!(f, "Expected {expected}, got {actual}")
        }
    }
}
