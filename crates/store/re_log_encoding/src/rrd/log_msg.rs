use re_protos::log_msg::v1alpha1::{ArrowMsg, BlueprintActivationCommand, SetStoreInfo};

use crate::rrd::{CodecError, Decodable, Encodable, MessageHeader, MessageKind};

impl Encodable for re_protos::log_msg::v1alpha1::log_msg::Msg {
    /// Serializes the appropriate `MessageHeader` too!
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, CodecError> {
        use re_protos::external::prost::Message as _;

        let before = out.len() as u64;

        match self {
            Self::SetStoreInfo(set_store_info) => {
                let header = MessageHeader {
                    kind: MessageKind::SetStoreInfo,
                    len: set_store_info.encoded_len() as u64,
                };
                header.to_rrd_bytes(out)?;
                set_store_info.encode(out)?;
            }

            Self::ArrowMsg(arrow_msg) => {
                let header = MessageHeader {
                    kind: MessageKind::ArrowMsg,
                    len: arrow_msg.encoded_len() as u64,
                };
                header.to_rrd_bytes(out)?;
                arrow_msg.encode(out)?;
            }

            Self::BlueprintActivationCommand(blueprint_activation_command) => {
                let header = MessageHeader {
                    kind: MessageKind::BlueprintActivationCommand,
                    len: blueprint_activation_command.encoded_len() as u64,
                };
                header.to_rrd_bytes(out)?;
                blueprint_activation_command.encode(out)?;
            }
        }

        Ok(out.len() as u64 - before)
    }
}

impl Encodable for re_protos::log_msg::v1alpha1::ArrowMsg {
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, CodecError> {
        use re_protos::external::prost::Message as _;

        let before = out.len() as u64;
        self.encode(out)?;
        Ok(out.len() as u64 - before)
    }
}

impl Encodable for re_protos::log_msg::v1alpha1::RrdFooter {
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, CodecError> {
        use re_protos::external::prost::Message as _;

        let before = out.len() as u64;
        self.encode(out)?;
        Ok(out.len() as u64 - before)
    }
}

impl Encodable for re_protos::log_msg::v1alpha1::RrdManifest {
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, CodecError> {
        use re_protos::external::prost::Message as _;

        let before = out.len() as u64;
        self.encode(out)?;
        Ok(out.len() as u64 - before)
    }
}

// NOTE: This is implemented for `Option<_>` because, in the native RRD protocol, the message kind
// might be `MessageKind::End` (signifying end-of-stream), which has no representation in our Protobuf
// definitions. I.e. `MessageKind::End` yields `None`.
impl Decodable for Option<re_protos::log_msg::v1alpha1::log_msg::Msg> {
    /// This expects `data` to carry the `MessageHeader` bytes too!
    fn from_rrd_bytes(mut data: impl bytes::Buf) -> Result<Self, CodecError> {
        let mut header = [0u8; MessageHeader::ENCODED_SIZE_BYTES];
        data.try_copy_to_slice(&mut header).map_err(|err| {
            CodecError::FrameDecoding(format!(
                "message of {} bytes is too short to hold a {}-byte header",
                err.available, err.requested
            ))
        })?;
        decode_transport_msg(MessageHeader::from_rrd_bytes(header.as_slice())?.kind, data)
    }
}

/// Decodes the payload of a message of the given `kind`, excluding its `MessageHeader`.
///
/// `MessageKind::End` yields `None`. See [`Decodable`] for how the type of `payload`
/// affects copies and lifetimes.
pub fn decode_transport_msg(
    kind: MessageKind,
    payload: impl bytes::Buf,
) -> Result<Option<re_protos::log_msg::v1alpha1::log_msg::Msg>, CodecError> {
    use re_protos::log_msg::v1alpha1::log_msg::Msg;

    Ok(Some(match kind {
        MessageKind::SetStoreInfo => Msg::SetStoreInfo(SetStoreInfo::from_rrd_bytes(payload)?),
        MessageKind::ArrowMsg => Msg::ArrowMsg(ArrowMsg::from_rrd_bytes(payload)?),
        MessageKind::BlueprintActivationCommand => {
            Msg::BlueprintActivationCommand(BlueprintActivationCommand::from_rrd_bytes(payload)?)
        }
        MessageKind::End => return Ok(None),
    }))
}

macro_rules! impl_decodable_message {
    ($($ty:ty),* $(,)?) => {$(
        impl Decodable for $ty {
            fn from_rrd_bytes(data: impl bytes::Buf) -> Result<Self, CodecError> {
                use re_protos::external::prost::Message as _;
                Ok(Self::decode(data)?)
            }
        }
    )*};
}

impl_decodable_message!(
    ArrowMsg,
    BlueprintActivationCommand,
    SetStoreInfo,
    re_protos::log_msg::v1alpha1::RrdFooter,
    re_protos::log_msg::v1alpha1::RrdManifest,
);
