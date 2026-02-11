//! Error types used in the `re_arrow_combinators` crate.

use std::num::TryFromIntError;
use std::sync::Arc;

use arrow::datatypes::DataType;
use arrow::error::ArrowError;

/// Errors that can occur during array transformations.
#[derive(Debug, thiserror::Error, Clone)]
pub enum Error {
    #[error("Field '{field_name}' not found. Available fields: [{}]", available_fields.join(", "))]
    FieldNotFound {
        field_name: String,
        available_fields: Vec<String>,
    },

    #[error("Field '{field_name}' has wrong type: expected {expected_type}, got {actual_type}")]
    FieldTypeMismatch {
        field_name: String,
        expected_type: String,
        actual_type: DataType,
    },

    #[error("Type mismatch in {context}: expected {expected}, got {actual}")]
    TypeMismatch {
        expected: String,
        actual: DataType,
        context: String,
    },

    #[error("Struct is missing required field '{field_name}'. Available fields: [{}]", struct_fields.join(", "))]
    MissingStructField {
        field_name: String,
        struct_fields: Vec<String>,
    },

    #[error("Unexpected value: expected one of {expected:?}, got {actual}")]
    UnexpectedValue {
        expected: &'static [&'static str],
        actual: String,
    },

    #[error("List contains unexpected value type: expected {expected}, got {actual}")]
    UnexpectedListValueType { expected: String, actual: DataType },

    #[error("Expected list with {expected} elements, got {actual}")]
    UnexpectedListValueLength { expected: usize, actual: usize },

    #[error("Fixed-size list contains unexpected value type: expected {expected}, got {actual}")]
    UnexpectedFixedSizeListValueType { expected: String, actual: DataType },

    #[error("Expected list to contain struct values, but got {actual}")]
    ExpectedStructInList { actual: DataType },

    #[error(
        "Field '{field_name}' has type {actual_type}, but expected {expected_type} (inferred from field '{reference_field}')"
    )]
    InconsistentFieldTypes {
        field_name: String,
        actual_type: DataType,
        reference_field: String,
        expected_type: DataType,
    },

    #[error("Cannot create fixed-size list with {actual} fields: {err}")]
    InvalidNumberOfFields { actual: usize, err: TryFromIntError },

    #[error("At least one field name is required")]
    NoFieldNames,

    #[error("Offset overflow: cannot fit {actual} into {expected_type}")]
    OffsetOverflow {
        actual: usize,
        expected_type: &'static str,
    },

    #[error("Index {index} out of bounds for array of length {length}")]
    IndexOutOfBounds { index: usize, length: usize },

    #[error(transparent)]
    Arrow(Arc<ArrowError>),

    /// Placeholder for a custom error message that doesn't fit into the above categories.
    #[error("{0}")]
    Other(String),
}

impl From<ArrowError> for Error {
    fn from(err: ArrowError) -> Self {
        Self::Arrow(Arc::new(err))
    }
}

impl From<crate::selector::Error> for Error {
    fn from(err: crate::selector::Error) -> Self {
        match err {
            // If the selector error is already a runtime error, unwrap it
            crate::selector::Error::Runtime(e) => e,
            // For lex/parse errors, wrap them in a generic error message
            // These shouldn't typically happen at runtime since selectors are pre-parsed
            other => ArrowError::InvalidArgumentError(format!("Selector error: {other}")).into(),
        }
    }
}
