//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

mod errors;
mod helpers;
mod stream;
mod streaming;

pub use self::{
    errors::{DecodeError, NotAnRrdError},
    helpers::options_from_bytes,
    stream::{
        FileEncoded, StreamDecoder, StreamDecoderApp, StreamDecoderIterator, StreamDecoderTransport,
    },
    streaming::{StreamingDecoder, StreamingDecoderOptions, StreamingLogMsg},
};
