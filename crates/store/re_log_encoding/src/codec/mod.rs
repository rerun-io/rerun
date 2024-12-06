// pub mod file;
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

    #[error("Invalid file header")]
    InvalidFileHeader,

    #[error("Unknown message header")]
    UnknownMessageHeader,

    #[error("Invalid message header")]
    InvalidMessageHeader,

    #[error("Unknown message kind {0}")]
    UnknownMessageKind(u8),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}
