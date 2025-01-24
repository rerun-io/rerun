use ::arrow::error::ArrowError;
use re_build_info::CrateVersion;
use re_chunk::ChunkError;
use rrd::OptionsError;

pub(crate) mod arrow;
pub mod rrd;
pub mod wire;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("Arrow IPC serialization error: {0}")]
    ArrowSerialization(::arrow::error::ArrowError),

    #[error("Arrow2 IPC serialization error: {0}")]
    Arrow2Serialization(::arrow2::error::Error),

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

/// On failure to encode or serialize a [`LogMsg`].
#[derive(thiserror::Error, Debug)]
pub enum EncodeError {
    #[error("Failed to write: {0}")]
    Write(#[from] std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4(#[from] lz4_flex::block::CompressError),

    #[error("MsgPack error: {0}")]
    MsgPack(#[from] rmp_serde::encode::Error),

    #[error("Protobuf error: {0}")]
    Protobuf(#[from] re_protos::external::prost::EncodeError),

    #[error("Arrow error: {0}")]
    Arrow(#[from] ArrowError),

    #[error("{0}")]
    Codec(#[from] CodecError),

    #[error("Chunk error: {0}")]
    Chunk(#[from] ChunkError),

    #[error("Called append on already finished encoder")]
    AlreadyFinished,
}

/// On failure to encode or serialize a [`LogMsg`].
#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    #[error("Not an .rrd file")]
    NotAnRrd,

    #[error("Data was from an old, incompatible Rerun version")]
    OldRrdVersion,

    #[error("Data from Rerun version {file}, which is incompatible with the local Rerun version {local}")]
    IncompatibleRerunVersion {
        file: CrateVersion,
        local: CrateVersion,
    },

    #[error("Failed to decode the options: {0}")]
    Options(#[from] OptionsError),

    #[error("Failed to read: {0}")]
    Read(#[from] std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4(#[from] lz4_flex::block::DecompressError),

    #[error("Protobuf error: {0}")]
    Protobuf(#[from] re_protos::external::prost::DecodeError),

    #[error("Could not convert type from protobuf: {0}")]
    TypeConversion(#[from] re_protos::TypeConversionError),

    #[error("Failed to read chunk: {0}")]
    Chunk(#[from] re_chunk::ChunkError),

    #[error("Arrow error: {0}")]
    Arrow(#[from] ArrowError),

    #[error("MsgPack error: {0}")]
    MsgPack(#[from] rmp_serde::decode::Error),

    #[error("Codec error: {0}")]
    Codec(#[from] CodecError),
}
