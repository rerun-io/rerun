pub(crate) mod arrow;
pub mod file;
pub mod wire;

// ---

/// Compression format used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Compression {
    Off = 0,

    /// Very fast compression and decompression, but not very good compression ratio.
    LZ4 = 1,
}

/// How we serialize the data
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Serializer {
    Protobuf = 2,
}

// ---

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
