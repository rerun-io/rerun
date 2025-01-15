impl From<re_protos::log_msg::v0::Compression> for crate::Compression {
    fn from(value: re_protos::log_msg::v0::Compression) -> Self {
        match value {
            re_protos::log_msg::v0::Compression::None => Self::Off,
            re_protos::log_msg::v0::Compression::Lz4 => Self::LZ4,
        }
    }
}

impl From<crate::Compression> for re_protos::log_msg::v0::Compression {
    fn from(value: crate::Compression) -> Self {
        match value {
            crate::Compression::Off => Self::None,
            crate::Compression::LZ4 => Self::Lz4,
        }
    }
}

#[cfg(feature = "decoder")]
pub fn log_msg_from_proto(
    message: re_protos::log_msg::v0::LogMsg,
) -> Result<re_log_types::LogMsg, crate::decoder::DecodeError> {
    use crate::codec::{arrow::decode_arrow, CodecError};
    use crate::decoder::DecodeError;
    use re_protos::{
        log_msg::v0::{log_msg::Msg, Encoding},
        missing_field,
    };

    match message.msg {
        Some(Msg::SetStoreInfo(set_store_info)) => Ok(re_log_types::LogMsg::SetStoreInfo(
            set_store_info.try_into()?,
        )),
        Some(Msg::ArrowMsg(arrow_msg)) => {
            if arrow_msg.encoding() != Encoding::ArrowIpc {
                return Err(DecodeError::Codec(CodecError::UnsupportedEncoding));
            }

            let batch = decode_arrow(
                &arrow_msg.payload,
                arrow_msg.uncompressed_size as usize,
                arrow_msg.compression().into(),
            )?;

            let store_id: re_log_types::StoreId = arrow_msg
                .store_id
                .ok_or_else(|| missing_field!(re_protos::log_msg::v0::ArrowMsg, "store_id"))?
                .into();

            let chunk = re_chunk::Chunk::from_record_batch(batch)?;

            Ok(re_log_types::LogMsg::ArrowMsg(
                store_id,
                chunk.to_arrow_msg()?,
            ))
        }
        Some(Msg::BlueprintActivationCommand(blueprint_activation_command)) => {
            Ok(re_log_types::LogMsg::BlueprintActivationCommand(
                blueprint_activation_command.try_into()?,
            ))
        }
        None => Err(missing_field!(re_protos::log_msg::v0::LogMsg, "msg").into()),
    }
}

#[cfg(feature = "encoder")]
pub fn log_msg_to_proto(
    message: re_log_types::LogMsg,
    compression: crate::Compression,
) -> Result<re_protos::log_msg::v0::LogMsg, crate::encoder::EncodeError> {
    use crate::codec::arrow::encode_arrow;
    use re_protos::log_msg::v0::{
        ArrowMsg, BlueprintActivationCommand, LogMsg as ProtoLogMsg, SetStoreInfo,
    };

    let proto_msg = match message {
        re_log_types::LogMsg::SetStoreInfo(set_store_info) => {
            let set_store_info: SetStoreInfo = set_store_info.into();
            ProtoLogMsg {
                msg: Some(re_protos::log_msg::v0::log_msg::Msg::SetStoreInfo(
                    set_store_info,
                )),
            }
        }
        re_log_types::LogMsg::ArrowMsg(store_id, arrow_msg) => {
            let payload = encode_arrow(&arrow_msg.batch, compression)?;
            let arrow_msg = ArrowMsg {
                store_id: Some(store_id.into()),
                compression: match compression {
                    crate::Compression::Off => re_protos::log_msg::v0::Compression::None as i32,
                    crate::Compression::LZ4 => re_protos::log_msg::v0::Compression::Lz4 as i32,
                },
                uncompressed_size: payload.uncompressed_size as i32,
                encoding: re_protos::log_msg::v0::Encoding::ArrowIpc as i32,
                payload: payload.data,
            };
            ProtoLogMsg {
                msg: Some(re_protos::log_msg::v0::log_msg::Msg::ArrowMsg(arrow_msg)),
            }
        }
        re_log_types::LogMsg::BlueprintActivationCommand(blueprint_activation_command) => {
            let blueprint_activation_command: BlueprintActivationCommand =
                blueprint_activation_command.into();
            ProtoLogMsg {
                msg: Some(
                    re_protos::log_msg::v0::log_msg::Msg::BlueprintActivationCommand(
                        blueprint_activation_command,
                    ),
                ),
            }
        }
    };

    Ok(proto_msg)
}
