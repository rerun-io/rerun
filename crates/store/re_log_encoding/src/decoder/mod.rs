//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

mod errors;
mod helpers;
mod stream;
mod streaming;

pub use self::{
    errors::{DecodeError, NotAnRrdError},
    helpers::options_from_bytes,
    stream::{Decoder, DecoderApp, DecoderIterator, DecoderTransport, FileEncoded},
    streaming::{StreamingDecoder, StreamingDecoderOptions, StreamingLogMsg},
};
