use re_chunk::ChunkError;

pub type ChunkIndexResult<T> = Result<T, ChunkIndexError>;

/// Possible errors when building, validating, or merging chunk indexes.
///
/// None of these involve IO: the only way to get one is from a malformed or inconsistent index.
#[derive(Debug, thiserror::Error)]
pub enum ChunkIndexError {
    #[error("Arrow IPC deserialization error: {0}")]
    ArrowDeserialization(::arrow::error::ArrowError),

    #[error("Arrow IPC serialization error: {0}")]
    ArrowSerialization(::arrow::error::ArrowError),

    #[error("Invalid chunk: {0}")]
    Chunk(Box<ChunkError>),

    #[error(transparent)]
    GetColumn(#[from] re_arrow_util::GetColumnError),

    #[error("Invalid timeline name: {0}")]
    InvalidTimelineName(#[from] re_log_types::InvalidTimelineNameError),

    #[error("Failed to merge manifests: {0}")]
    Merge(String),

    /// A column was missing, had the wrong datatype, or had unexpected nulls.
    #[error(transparent)]
    Quiver(#[from] quiver::Error),

    #[error("Sorbet error: {0}")]
    Sorbet(#[from] re_sorbet::SorbetError),
}

const _: () = assert!(
    std::mem::size_of::<ChunkIndexError>() <= 64,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

impl From<ChunkError> for ChunkIndexError {
    fn from(value: ChunkError) -> Self {
        Self::Chunk(Box::new(value))
    }
}
