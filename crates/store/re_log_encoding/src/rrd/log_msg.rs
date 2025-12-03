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
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, CodecError> {
        use re_protos::log_msg::v1alpha1::{ArrowMsg, BlueprintActivationCommand, SetStoreInfo};

        let header = MessageHeader::from_rrd_bytes(&data[..MessageHeader::ENCODED_SIZE_BYTES])?;

        let data = &data[MessageHeader::ENCODED_SIZE_BYTES..];
        let msg = match header.kind {
            MessageKind::SetStoreInfo => {
                Some(re_protos::log_msg::v1alpha1::log_msg::Msg::SetStoreInfo(
                    SetStoreInfo::from_rrd_bytes(data)?,
                ))
            }

            MessageKind::ArrowMsg => Some(re_protos::log_msg::v1alpha1::log_msg::Msg::ArrowMsg(
                ArrowMsg::from_rrd_bytes(data)?,
            )),

            MessageKind::BlueprintActivationCommand => Some(
                re_protos::log_msg::v1alpha1::log_msg::Msg::BlueprintActivationCommand(
                    BlueprintActivationCommand::from_rrd_bytes(data)?,
                ),
            ),

            MessageKind::End => None,
        };

        Ok(msg)
    }
}

impl Decodable for re_protos::log_msg::v1alpha1::SetStoreInfo {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, CodecError> {
        use re_protos::external::prost::Message as _;
        Ok(Self::decode(data)?)
    }
}

impl Decodable for re_protos::log_msg::v1alpha1::ArrowMsg {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, CodecError> {
        use re_protos::external::prost::Message as _;
        Ok(Self::decode(data)?)
    }
}

impl Decodable for re_protos::log_msg::v1alpha1::BlueprintActivationCommand {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, CodecError> {
        use re_protos::external::prost::Message as _;
        Ok(Self::decode(data)?)
    }
}

impl Decodable for re_protos::log_msg::v1alpha1::RrdFooter {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, CodecError> {
        use re_protos::external::prost::Message as _;
        Ok(Self::decode(data)?)
    }
}

impl Decodable for re_protos::log_msg::v1alpha1::RrdManifest {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, CodecError> {
        use re_protos::external::prost::Message as _;
        Ok(Self::decode(data)?)
    }
}
