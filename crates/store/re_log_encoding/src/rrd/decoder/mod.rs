//! Decoding `LogMsg`:es from `.rrd` files/streams.

pub mod state_machine;

mod iterator;
mod stream;

pub use self::iterator::DecoderIterator;
pub use self::state_machine::{Decoder, DecoderApp, DecoderTransport};
pub use self::stream::DecoderStream;

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

/// Largest byte chunk the `BufRead`-driven decoders push into the state machine at once.
///
/// A decoded message aliases the chunk it was read from, so this bounds how much memory a message
/// that is kept alive can pin beyond its own bytes. Without it, reading from an in-memory slice
/// would push the whole input as one chunk. Messages larger than this are gathered into their own
/// buffer instead.
///
/// 1 MiB is above the size of all but blob-carrying messages, so nearly every message is a view,
/// while the memory a single message can pin stays negligible next to any memory limit.
const MAX_BYTE_CHUNK_LEN: usize = 1024 * 1024;

// ---

use re_build_info::CrateVersion;

use crate::{ApplicationIdInjector, ToApplication as _};

/// Implemented for top-level types that can kickoff decoding.
///
/// There are only two of them in this crate:
/// * [`re_log_msg::LogMsg`]: application-level root message
/// * [`re_protos::log_msg::v1alpha1::log_msg::Msg`]: transport-level root message
///
/// This can be used to generically instantiate transport- and/or application-level decoders.
/// See also:
/// * [`DecoderTransport`]
/// * [`DecoderApp`]
pub trait DecoderEntrypoint: Sized {
    fn decode(
        data_excluding_headers: bytes::Bytes,
        byte_span_excluding_headers: re_chunk::Span<u64>,
        message_kind: crate::rrd::MessageKind,
        app_id_injector: &mut impl ApplicationIdInjector,
        patched_version: Option<CrateVersion<'static>>,
    ) -> Result<Option<Self>, crate::rrd::CodecError>;
}

impl DecoderEntrypoint for re_log_msg::LogMsg {
    fn decode(
        data_excluding_headers: bytes::Bytes,
        byte_span_excluding_headers: re_chunk::Span<u64>,
        message_kind: crate::rrd::MessageKind,
        app_id_injector: &mut impl ApplicationIdInjector,
        patched_version: Option<CrateVersion<'static>>,
    ) -> Result<Option<Self>, crate::rrd::CodecError> {
        let Some(log_msg) = re_protos::log_msg::v1alpha1::log_msg::Msg::decode(
            data_excluding_headers,
            byte_span_excluding_headers,
            message_kind,
            app_id_injector,
            patched_version,
        )?
        else {
            return Ok(None);
        };

        log_msg
            .to_application((app_id_injector, patched_version))
            .map(Some)
    }
}

impl DecoderEntrypoint for re_protos::log_msg::v1alpha1::log_msg::Msg {
    fn decode(
        data_excluding_headers: bytes::Bytes,
        _byte_span_excluding_headers: re_chunk::Span<u64>,
        message_kind: crate::rrd::MessageKind,
        _app_id_injector: &mut impl ApplicationIdInjector,
        _patched_version: Option<CrateVersion<'static>>,
    ) -> Result<Option<Self>, crate::rrd::CodecError> {
        crate::rrd::decode_transport_msg(message_kind, data_excluding_headers)
    }
}
