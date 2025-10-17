//! Decoding `LogMsg`:es from `.rrd` files/streams.

mod stream;
mod streaming; // TODO(cmc): destroy

pub use self::{
    stream::{Decoder, DecoderApp, DecoderEntrypoint, DecoderIterator, DecoderTransport},
    streaming::{StreamingDecoder, StreamingDecoderOptions, StreamingLogMsg},
};

/// On failure to decode or serialize a `LogMsg`.
#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    #[error("Codec error: {0}")]
    Codec(#[from] crate::rrd::CodecError),

    #[error("Failed to read: {0}")]
    Read(#[from] std::io::Error),
}

const _: () = assert!(
    std::mem::size_of::<DecodeError>() <= 64,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);
