use thiserror::Error;

// TODO(grtlr): Since we have builtin support for CDR and protobuf,
// it might make sense to add distinct error variants for those.

#[derive(Error, Debug)]
/// The base error type for handling MCAP files.
///
/// Where possible, the specialized error variants should be used.
/// The [`Error::Other`] variant can be used in all other cases.
pub enum Error {
    #[error("Channel {0} does not define a schema")]
    NoSchema(String),

    #[error("Invalid schema {schema}: {source}")]
    InvalidSchema {
        schema: String,
        source: anyhow::Error,
    },

    #[error(transparent)]
    Mcap(#[from] ::mcap::McapError),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Serialization(#[from] re_sdk_types::SerializationError),

    #[error(transparent)]
    Chunk(#[from] re_chunk::ChunkError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
