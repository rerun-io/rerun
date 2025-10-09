//! Crate that handles encoding of rerun log types.

pub mod codec;
pub mod protobuf_conversions;

#[cfg(feature = "decoder")]
mod decoder;

#[cfg(feature = "encoder")]
mod encoder;

#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
mod file_sink;

#[cfg(feature = "stream_from_http")]
pub mod stream_rrd_from_http;

pub mod external {
    #[cfg(feature = "decoder")]
    pub use lz4_flex;
}

#[cfg(feature = "decoder")]
pub use self::decoder::{
    ApplicationIdInjector, CachingApplicationIdInjector, DecodeError, Decoder, DecoderApp,
    DecoderIterator, DecoderTransport, DummyApplicationIdInjector, FileEncoded, NotAnRrdError,
    StreamingDecoder, StreamingDecoderOptions, StreamingLogMsg,
};

#[cfg(feature = "encoder")]
pub use self::encoder::{EncodeError, Encoder, EncodingOptions};

#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
pub use self::file_sink::{FileFlushError, FileSink, FileSinkError};

// ----------------------------------------------------------------------------

/// The currently used `FourCC` for Rerun RRD files.
pub const RRD_FOURCC: [u8; 4] = *b"RRF2";

/// Previously used `FourCC`s for Rerun RRD files.
pub const OLD_RRD_FOURCC: &[[u8; 4]] = &[*b"RRF0", *b"RRF1"];
