use re_log_types::LogMsg;
use re_protos::log_msg::v1alpha1 as proto;

use crate::codec::Compression;
use crate::codec::arrow::encode_arrow;
use crate::encoder::EncodeError;

use super::{MessageHeader, MessageKind};

pub(crate) fn encode(
    buf: &mut Vec<u8>,
    message: &LogMsg,
    compression: Compression,
) -> Result<(), EncodeError> {
    use re_protos::external::prost::Message as _;
    use re_protos::log_msg::v1alpha1::{
        ArrowMsg, BlueprintActivationCommand, Encoding, SetStoreInfo,
    };

    match message {
        LogMsg::SetStoreInfo(set_store_info) => {
            let set_store_info: SetStoreInfo = set_store_info.clone().into();
            let header = MessageHeader {
                kind: MessageKind::SetStoreInfo,
                len: set_store_info.encoded_len() as u64,
            };
            header.encode(buf)?;
            set_store_info.encode(buf)?;
        }
        LogMsg::ArrowMsg(
            store_id,
            re_log_types::ArrowMsg {
                chunk_id,
                batch,
                on_release: _,
            },
        ) => {
            let payload = encode_arrow(batch, compression)?;
            let arrow_msg = ArrowMsg {
                store_id: Some(store_id.clone().into()),
                chunk_id: Some((*chunk_id).into()),
                compression: match compression {
                    Compression::Off => proto::Compression::None as i32,
                    Compression::LZ4 => proto::Compression::Lz4 as i32,
                },
                uncompressed_size: payload.uncompressed_size,
                encoding: Encoding::ArrowIpc as i32,
                payload: payload.data.into(),
                is_static: re_sorbet::is_static_chunk(batch),
            };
            let header = MessageHeader {
                kind: MessageKind::ArrowMsg,
                len: arrow_msg.encoded_len() as u64,
            };
            header.encode(buf)?;
            arrow_msg.encode(buf)?;
        }
        LogMsg::BlueprintActivationCommand(blueprint_activation_command) => {
            let blueprint_activation_command: BlueprintActivationCommand =
                blueprint_activation_command.clone().into();
            let header = MessageHeader {
                kind: MessageKind::BlueprintActivationCommand,
                len: blueprint_activation_command.encoded_len() as u64,
            };
            header.encode(buf)?;
            blueprint_activation_command.encode(buf)?;
        }
    }

    Ok(())
}

pub(crate) fn encode_proto(buf: &mut Vec<u8>, message: proto::LogMsg) -> Result<(), EncodeError> {
    use re_protos::external::prost::Message as _;
    use re_protos::log_msg::v1alpha1 as proto;

    match message.msg.ok_or(EncodeError::MissingField("msg"))? {
        proto::log_msg::Msg::SetStoreInfo(set_store_info) => {
            let header = MessageHeader {
                kind: MessageKind::SetStoreInfo,
                len: set_store_info.encoded_len() as u64,
            };
            header.encode(buf)?;
            set_store_info.encode(buf)?;
        }
        proto::log_msg::Msg::ArrowMsg(arrow_msg) => {
            let header = MessageHeader {
                kind: MessageKind::ArrowMsg,
                len: arrow_msg.encoded_len() as u64,
            };
            header.encode(buf)?;
            arrow_msg.encode(buf)?;
        }
        proto::log_msg::Msg::BlueprintActivationCommand(blueprint_activation_command) => {
            let header = MessageHeader {
                kind: MessageKind::BlueprintActivationCommand,
                len: blueprint_activation_command.encoded_len() as u64,
            };
            header.encode(buf)?;
            blueprint_activation_command.encode(buf)?;
        }
    }

    Ok(())
}
