//! Everything needed to encode/decode and serialize/deserialize RRD streams.
//!
//! ⚠️Make sure to familiarize yourself with the [crate-level docs](crate) first. ⚠️
//!
//! RRD streams are used everywhere gRPC isn't: files, standard I/O, HTTP fetches, data-loaders, etc.
//! This module is completely unrelated to the Rerun Data Protocol (Redap) gRPC API.
//! This module is also completely unrelated to the legacy SDK comms gRPC API.
//!
//! ## [`Encodable`]/[`Decodable`] vs. `Encoder`/`Decoder`
//!
//! The [`Encodable`]/[`Decodable`] traits specify how transport-level types should be encoded to
//! and decoded from top-level RRD types, respectively. Only transport-level types can be encoded/decoded.
//!
//! That is all these traits do. They do not perform any kind of IO, they do not keep track of any
//! sort of state. That's the job of the `Encoder` and `Decoder`: they provide the IO and the
//! state machines that turn collections of `Encodable`s and `Decodable`s into actual RRD streams.

mod errors;
mod footer;
mod frames;
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

#[cfg(feature = "decoder")]
pub use self::decoder::{
    DecodeError, Decoder, DecoderApp, DecoderEntrypoint, DecoderIterator, DecoderStream,
    DecoderTransport,
};
#[cfg(feature = "encoder")]
pub use self::encoder::{EncodeError, Encoder};
pub use self::errors::{CodecError, CodecResult, NotAnRrdError, OptionsError};
#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
pub use self::file_sink::{FileFlushError, FileSink, FileSinkError};
pub use self::footer::{
    RawRrdManifest, RrdFooter, RrdManifest, RrdManifestBuilder, RrdManifestSha256,
    RrdManifestStaticMap, RrdManifestTemporalMap, RrdManifestTemporalMapEntry,
};
pub use self::frames::{
    Compression, CrateVersion, EncodingOptions, MessageHeader, MessageKind, Serializer,
    StreamFooter, StreamFooterEntry, StreamHeader,
};

// ---

/// The currently used `FourCC` for Rerun RRD files.
pub const RRD_FOURCC: [u8; 4] = *b"RRF2";

/// Previously used `FourCC`s for Rerun RRD files.
pub const OLD_RRD_FOURCC: &[[u8; 4]] = &[*b"RRF0", *b"RRF1"];

// ---

/// Encodes transport-level types (i.e. Protobuf objects) into RRD bytes.
///
/// The RRD protocol is pretty complex and mixes layers of custom binary, Protobuf and
/// Arrow encoded data, as well layer-specific compression schemes.
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
/// The RRD protocol is pretty complex and mixes layers of custom binary, Protobuf and
/// Arrow encoded data, as well layer-specific compression schemes.
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
