pub(crate) mod arrow;
pub mod file;
pub mod wire;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("Arrow IPC serialization error: {0}")]
    ArrowSerialization(::arrow::error::ArrowError),

    #[error("Invalid Chunk: {0}")]
    InvalidChunk(::arrow::error::ArrowError),

    #[error("Arrow IPC deserialization error: {0}")]
    ArrowDeserialization(::arrow::error::ArrowError),

    #[error("Failed to decode message header {0}")]
    HeaderDecoding(std::io::Error),

    #[error("Failed to encode message header {0}")]
    HeaderEncoding(std::io::Error),

    #[error("Missing record batch")]
    MissingRecordBatch,

    #[error("Unexpected stream state")]
    UnexpectedStreamState,

    #[error("Unsupported encoding, expected Arrow IPC")]
    UnsupportedEncoding,

    #[error("Unknown message header")]
    UnknownMessageHeader,
}
