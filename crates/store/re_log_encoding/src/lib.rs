//! Crate that handles encoding of rerun log types.

pub mod codec;

#[cfg(feature = "decoder")]
pub mod decoder {
    pub use super::codec::rrd::decoder::decode_bytes;
    pub use super::codec::rrd::decoder::streaming::StreamingDecoder;
    pub use super::codec::rrd::decoder::Decoder;
    pub use super::codec::DecodeError;
}

#[cfg(feature = "encoder")]
pub mod encoder {
    pub use super::codec::rrd::encoder::encode;
    pub use super::codec::rrd::encoder::encode_as_bytes;
    pub use super::codec::rrd::encoder::encode_as_bytes_local;
    pub use super::codec::rrd::encoder::encode_ref;
    pub use super::codec::rrd::encoder::encode_ref_as_bytes_local;
    pub use super::codec::rrd::encoder::encode_to_bytes;
    pub use super::codec::rrd::encoder::local_raw_encoder;
    pub use super::codec::rrd::encoder::DroppableEncoder;
    pub use super::codec::rrd::encoder::Encoder;
    pub use super::codec::EncodeError;
}

pub use codec::rrd::Compression;
pub use codec::rrd::EncodingOptions;
pub use codec::rrd::VersionPolicy;

pub mod protobuf_conversions;

#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
mod file_sink;

#[cfg(feature = "stream_from_http")]
pub mod stream_rrd_from_http;

#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
pub use file_sink::{FileSink, FileSinkError};
