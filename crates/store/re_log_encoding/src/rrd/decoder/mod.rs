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

// ---

use re_build_info::CrateVersion;

use crate::rrd::Decodable as _;
use crate::{ApplicationIdInjector, MessageKind, ToApplication as _};

/// Implemented for top-level types that can kickoff decoding.
///
/// There are only two of them in this crate:
/// * [`re_log_types::LogMsg`]: application-level root message
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
        patched_version: Option<CrateVersion>,
    ) -> Result<Option<Self>, crate::rrd::CodecError>;
}

impl DecoderEntrypoint for re_log_types::LogMsg {
    fn decode(
        data_excluding_headers: bytes::Bytes,
        byte_span_excluding_headers: re_chunk::Span<u64>,
        message_kind: crate::rrd::MessageKind,
        app_id_injector: &mut impl ApplicationIdInjector,
        patched_version: Option<CrateVersion>,
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
        _patched_version: Option<CrateVersion>,
    ) -> Result<Option<Self>, crate::rrd::CodecError> {
        let data = &data_excluding_headers;

        Ok(Some(match message_kind {
            MessageKind::SetStoreInfo => Self::SetStoreInfo(
                re_protos::log_msg::v1alpha1::SetStoreInfo::from_rrd_bytes(data)?,
            ),

            MessageKind::ArrowMsg => Self::ArrowMsg(
                re_protos::log_msg::v1alpha1::ArrowMsg::from_rrd_bytes(data)?,
            ),

            MessageKind::BlueprintActivationCommand => Self::BlueprintActivationCommand(
                re_protos::log_msg::v1alpha1::BlueprintActivationCommand::from_rrd_bytes(data)?,
            ),

            MessageKind::End => return Ok(None),
        }))
    }
}
