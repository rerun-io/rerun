//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

mod app_id_injector;
mod errors;
mod stream;
mod streaming;

pub use self::{
    app_id_injector::{
        ApplicationIdInjector, CachingApplicationIdInjector, DummyApplicationIdInjector,
    },
    errors::{DecodeError, NotAnRrdError},
    stream::{Decoder, DecoderApp, DecoderIterator, DecoderTransport, FileEncoded},
    streaming::{StreamingDecoder, StreamingDecoderOptions, StreamingLogMsg},
};
