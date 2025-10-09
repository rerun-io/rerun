//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

mod app_id_injector;
mod errors;
mod helpers;
mod stream;
mod streaming;

pub use self::{
    app_id_injector::{
        ApplicationIdInjector, CachingApplicationIdInjector, DummyApplicationIdInjector,
    },
    errors::{DecodeError, NotAnRrdError},
    helpers::options_from_bytes,
    stream::{Decoder, DecoderApp, DecoderIterator, DecoderTransport, FileEncoded},
    streaming::{StreamingDecoder, StreamingDecoderOptions, StreamingLogMsg},
};
