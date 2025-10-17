use crate::rrd::{CodecError, Decodable, Encodable, MessageHeader, MessageKind};

// TODO: you will never use `LogMsg` here, but i should maybe explain why.

// TODO: basically we want Read and Write but scoped to in-memory non-fallible implementations only
// maybe i do something silly like this.
// trait InMemoryIO: Read + Write {}
// impl InMemoryIO for Cursor<Vec<u8>> {}
// impl<'a> InMemoryIO for Cursor<&'a [u8]> {}

// --- LogMsg (transport layer): re_protos::log_msg::v1alpha1::log_msg::Msg ---

impl Encodable for re_protos::log_msg::v1alpha1::log_msg::Msg {
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

// NOTE: This is implemented for `Option<_>` because, in the native RRD protocol, the message kind
// might be `MessageKind::End` (signifying end-of-stream), which has no representation in our Protobuf
// definitions. I.e. `MessageKind::End` yields `None`.
impl Decodable for Option<re_protos::log_msg::v1alpha1::log_msg::Msg> {
    // NOTE: This is required because, in the native RRD protocol, `LogMsg`s are encoded _without_
    // the associated `oneof` Protobuf layer. I.e. we do not serialize `re_protos::log_msg::v1alpha1::LogMsg`
    // objects, but rather we serialize `re_protos::log_msg::v1alpha1::log_msg::Msg` objects.
    //
    // Therefore, we need an out-of-band `MessageKind` to know what we're trying to decode in the
    // first place.
    type Context<'a> = crate::rrd::MessageKind;

    fn from_rrd_bytes(data: &[u8], msg_kind: Self::Context<'_>) -> Result<Self, CodecError> {
        use re_protos::external::prost::Message as _;
        use re_protos::log_msg::v1alpha1::{ArrowMsg, BlueprintActivationCommand, SetStoreInfo};

        let msg = match msg_kind {
            MessageKind::SetStoreInfo => {
                Some(re_protos::log_msg::v1alpha1::log_msg::Msg::SetStoreInfo(
                    SetStoreInfo::decode(data)?,
                ))
            }

            MessageKind::ArrowMsg => {
                let msg = ArrowMsg::decode(data)?;
                Some(re_protos::log_msg::v1alpha1::log_msg::Msg::ArrowMsg(msg))
            }

            MessageKind::BlueprintActivationCommand => {
                let msg = BlueprintActivationCommand::decode(data)?;
                Some(re_protos::log_msg::v1alpha1::log_msg::Msg::BlueprintActivationCommand(msg))
            }

            MessageKind::End => None,
        };

        Ok(msg)
    }
}
