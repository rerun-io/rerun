use arrow::{datatypes::DataType, error::ArrowError};

/// Different variants of errors that can happen when executing lenses.
#[expect(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("expected data type `{expected}` but found data type `{actual}`")]
    TypeMismatch {
        actual: DataType,
        expected: DataType,
    },

    #[error("missing field `{expected}, found {}`", found.join(", "))]
    MissingField {
        expected: String,
        found: Vec<String>,
    },

    #[error("unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error(transparent)]
    Arrow(#[from] ArrowError),

    #[error(transparent)]
    Other(Box<dyn std::error::Error>),
}
