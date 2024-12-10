pub mod wire;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("Arrow serialization error: {0}")]
    ArrowSerialization(arrow2::error::Error),

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
