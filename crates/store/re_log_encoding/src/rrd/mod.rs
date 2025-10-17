//! Everything need to encode/decode and serialize/deserialize RRD streams.
//!
//! RRD streams are used where streaming Rerun data via files, standard I/O, HTTP, data loaders, etc.
//!
//! This is completely unrelated to the Rerun Data Protocol (Redap) gRPC API.
//! This is also completely unrelated to the legacy SDK comms gRPC API.

mod errors;
mod headers;
mod log_msg;

#[cfg(feature = "decoder")]
mod decoder;

#[cfg(feature = "encoder")]
mod encoder;

#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
mod file_sink;

#[cfg(feature = "stream_from_http")]
pub mod stream_from_http;

pub use self::errors::{CodecError, NotAnRrdError, OptionsError};
pub use self::headers::{
    Compression, CrateVersion, EncodingOptions, MessageHeader, MessageKind, Serializer,
    StreamHeader,
};

#[cfg(feature = "decoder")]
pub use self::decoder::{
    DecodeError, Decoder, DecoderApp, DecoderEntrypoint, DecoderIterator, DecoderStream,
    DecoderTransport, StreamingDecoder, StreamingDecoderOptions, StreamingLogMsg,
};

#[cfg(feature = "encoder")]
pub use self::encoder::{EncodeError, Encoder};

#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
pub use self::file_sink::{FileFlushError, FileSink, FileSinkError};

// ---

/// The currently used `FourCC` for Rerun RRD files.
pub const RRD_FOURCC: [u8; 4] = *b"RRF2";

/// Previously used `FourCC`s for Rerun RRD files.
pub const OLD_RRD_FOURCC: &[[u8; 4]] = &[*b"RRF0", *b"RRF1"];

// ---

// TODO:
// * explain wtf are app-level vs. transport-level types
// * why you cannot take app-level types

/// Encodes transport-level types (i.e. Protobuf objects) into RRD bytes.
///
/// The RRD protocol is pretty complex and mixes layers of custom binary protocols, Protobuf and
/// Arrow encoded, and layer-specific compression schemes.
/// This trait takes care of all of that for you.
///
/// This exclusively performs encoding and _nothing else_. In particular, it does not:
/// * perform any kind of IO
/// * perform any kind of data migration
/// * perform any kind of data patching (app ID injection, version propagation, other BW-compat hacks)
/// * perform any kind of compression
/// * etc
///
/// All it does is map transport types to their RRD representation. If you're wondering how to turn
/// application-level types into transport-level types for encoding, have a look at the
/// [`ToTransport`] trait.
///
/// The only way this can fail is due to invalid data.
///
/// [`ToTransport`]: crate::ToTransport
//
// TODO(cmc): technically this should be a sealed trait, but things are complicated enough as is.
pub trait Encodable {
    /// Returns number of encoded bytes.
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, CodecError>;
}

/// Decodes RRD bytes into transport-level types (i.e. Protobuf objects).
///
/// The RRD protocol is pretty complex and mixes layers of custom binary protocols, Protobuf and
/// Arrow encoded, and layer-specific compression schemes.
/// This trait takes care of all of that for you.
///
/// This exclusively performs encoding and _nothing else_. In particular, it does not:
/// * perform any kind of IO
/// * perform any kind of data migration
/// * perform any kind of data patching (app ID injection, version propagation, other BW-compat hacks)
/// * perform any kind of compression
/// * etc
///
/// All it does is map RRD bytes to transport-level types. If you're interested into turning these
/// transport-level types into higher-level objects (such as [`re_log_types::LogMsg`] with all kinds
/// of application-level transformations applied (such as the one mentioned above), then have a look at the
/// [`ToApplication`] trait.
///
/// The only way this can fail is due to invalid data.
///
/// [`ToApplication`]: crate::ToApplication
//
// TODO(cmc): technically this should be a sealed trait, but things are complicated enough as is.
pub trait Decodable: Sized {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, CodecError>;
}
