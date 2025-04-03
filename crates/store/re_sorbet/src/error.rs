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
    WrongDatatypeError(#[from] crate::WrongDatatypeError),

    #[error(transparent)]
    ArrowError(#[from] ArrowError),

    #[error("Bad chunk schema: {reason}")]
    Custom { reason: String },
}

impl SorbetError {
    pub fn custom(reason: impl Into<String>) -> Self {
        Self::Custom {
            reason: reason.into(),
        }
    }
}
