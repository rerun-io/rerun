use arrow::error::ArrowError;

#[derive(thiserror::Error, Debug)]
pub enum SorbetError {
    #[error(transparent)]
    UnknownColumnKind(#[from] crate::UnknownColumnKind),

    #[error(transparent)]
    MissingMetadataKey(#[from] crate::MissingMetadataKey),

    #[error(transparent)]
    MissingFieldMetadata(#[from] crate::MissingFieldMetadata),

    #[error(transparent)]
    UnsupportedTimeType(#[from] crate::UnsupportedTimeType),

    #[error(transparent)]
    WrongDatatypeError(#[from] re_arrow_util::WrongDatatypeError),

    #[error(transparent)]
    ArrowError(#[from] ArrowError),

    #[error("Missing chunk ID")]
    MissingChunkId,

    #[error("Missing entity path")]
    MissingEntityPath,

    #[error("Missing RowId column")]
    MissingRowIdColumn,

    #[error("Invalid column order: {0}")]
    InvalidColumnOrder(String),

    #[error("Multiple RowId columns found: {0}")]
    MultipleRowIdColumns(usize),

    #[error("Failed to deserialize chunk ID: {0}")]
    ChunkIdDeserializationError(String),
}

const _: () = assert!(
    std::mem::size_of::<SorbetError>() <= 64,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);
