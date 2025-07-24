use re_log_types::{BlueprintActivationCommand, LogMsg, SetStoreInfo};
use re_protos::missing_field;

use super::{MessageHeader, MessageKind};

use crate::ApplicationIdInjector;
use crate::codec::CodecError;
use crate::codec::arrow::decode_arrow;
use crate::decoder::DecodeError;

// ---

/// This decodes all the way from raw bytes to application-level types (i.e. even Arrow layers are decoded).
///
/// See also:
/// * [`decode_to_transport`]
pub(crate) fn decode_to_app(
    app_id_injector: &mut impl ApplicationIdInjector,
    data: &mut impl std::io::Read,
) -> Result<(u64, Option<LogMsg>), DecodeError> {
    let mut read_bytes = 0u64;
    let header = MessageHeader::decode(data)?;
    read_bytes += std::mem::size_of::<MessageHeader>() as u64 + header.len;

    let mut buf = vec![0; header.len as usize];
    data.read_exact(&mut buf[..])?;

    let msg = decode_bytes_to_app(app_id_injector, header.kind, &buf)?;

    Ok((read_bytes, msg))
}

/// This only decodes from raw bytes up to transport-level types (i.e. Protobuf payloads are
/// decoded, but Arrow data is never touched).
///
/// See also:
/// * [`decode_to_app`]
pub(crate) fn decode_to_transport(
    data: &mut impl std::io::Read,
) -> Result<(u64, Option<re_protos::log_msg::v1alpha1::log_msg::Msg>), DecodeError> {
    let mut read_bytes = 0u64;
    let header = MessageHeader::decode(data)?;
    read_bytes += std::mem::size_of::<MessageHeader>() as u64 + header.len;

    let mut buf = vec![0; header.len as usize];
    data.read_exact(&mut buf[..])?;

    let msg = decode_bytes_to_transport(header.kind, &buf)?;

    Ok((read_bytes, msg))
}

/// Decode a message of kind `message_kind` from `buf`.
///
/// This decodes all the way from raw bytes to application-level types (i.e. even Arrow layers are
/// decoded). For more fine-grained control, see:
/// * [`decode_bytes_to_transport`]
/// * [`decode_transport_to_app`]
///
/// `Ok(None)` returned from this function marks the end of the file stream.
#[tracing::instrument(level = "debug", skip_all)]
pub fn decode_bytes_to_app(
    app_id_injector: &mut impl ApplicationIdInjector,
    message_kind: MessageKind,
    buf: &[u8],
) -> Result<Option<LogMsg>, DecodeError> {
    let decoded = decode_bytes_to_transport(message_kind, buf)?;
    let decoded = decoded.map(|msg| decode_transport_to_app(app_id_injector, msg));
    decoded.transpose()
}

/// Decode a message of kind `message_kind` from `buf`.
///
/// This only decodes from raw bytes up to transport-level types (i.e. Protobuf payloads are
/// decoded, but Arrow data is never touched).
///
/// `Ok(None)` returned from this function marks the end of the file stream.
#[tracing::instrument(level = "debug", skip_all)]
pub fn decode_bytes_to_transport(
    message_kind: MessageKind,
    buf: &[u8],
) -> Result<Option<re_protos::log_msg::v1alpha1::log_msg::Msg>, DecodeError> {
    use re_protos::external::prost::Message as _;
    use re_protos::log_msg::v1alpha1::{ArrowMsg, BlueprintActivationCommand, SetStoreInfo};

    let msg = match message_kind {
        MessageKind::SetStoreInfo => {
            let msg = SetStoreInfo::decode(buf)?;
            Some(re_protos::log_msg::v1alpha1::log_msg::Msg::SetStoreInfo(
                msg,
            ))
        }

        MessageKind::ArrowMsg => {
            let msg = ArrowMsg::decode(buf)?;
            Some(re_protos::log_msg::v1alpha1::log_msg::Msg::ArrowMsg(msg))
        }

        MessageKind::BlueprintActivationCommand => {
            let msg = BlueprintActivationCommand::decode(buf)?;
            Some(re_protos::log_msg::v1alpha1::log_msg::Msg::BlueprintActivationCommand(msg))
        }

        MessageKind::End => None,
    };

    Ok(msg)
}

/// Decode a transport-level message.
///
/// This decodes a message from the transport layer (Protobuf) all the way to app layer, i.e. this
/// is where all Arrow data will be decoded.
#[tracing::instrument(level = "debug", skip_all)]
pub fn decode_transport_to_app(
    app_id_injector: &mut impl ApplicationIdInjector,
    msg: re_protos::log_msg::v1alpha1::log_msg::Msg,
) -> Result<LogMsg, DecodeError> {
    use re_protos::log_msg::v1alpha1::Encoding;

    let msg = match msg {
        re_protos::log_msg::v1alpha1::log_msg::Msg::SetStoreInfo(set_store_info) => {
            let set_store_info: SetStoreInfo = set_store_info.try_into()?;
            app_id_injector.store_info_received(&set_store_info.info);
            LogMsg::SetStoreInfo(set_store_info)
        }

        re_protos::log_msg::v1alpha1::log_msg::Msg::ArrowMsg(arrow_msg) => {
            if arrow_msg.encoding() != Encoding::ArrowIpc {
                return Err(DecodeError::Codec(CodecError::UnsupportedEncoding));
            }

            let batch = decode_arrow(
                &arrow_msg.payload,
                arrow_msg.uncompressed_size as usize,
                arrow_msg.compression().into(),
            )?;

            //TODO(#10730): clean that up when removing 0.24 back compat
            let store_id: re_log_types::StoreId = match arrow_msg
                .store_id
                .ok_or_else(|| missing_field!(re_protos::log_msg::v1alpha1::ArrowMsg, "store_id"))?
                .try_into()
            {
                Ok(store_id) => store_id,
                Err(err) => {
                    let Some(store_id) = app_id_injector.recover_store_id(err.clone()) else {
                        return Err(err.into());
                    };

                    store_id
                }
            };

            // TODO(grtlr): In the future, we should be able to rely on the `chunk_id` to be present in the
            // protobuf definitions. For now we have to extract it from the `batch`.
            //
            // let chunk_id = arrow_msg
            //     .chunk_id
            //     .ok_or_else(|| missing_field!(re_protos::log_msg::v1alpha1::ArrowMsg, "chunk_id"))?
            //     .try_from()?;

            // This also ensures that we perform all required migrations from `re_sorbet`.
            // TODO(#10343): Would it make sense to change `re_types_core::ArrowMsg` to contain the
            // `ChunkBatch` directly?
            let chunk_batch = re_sorbet::ChunkBatch::try_from(&batch)?;

            let arrow_msg = re_log_types::ArrowMsg {
                chunk_id: chunk_batch.chunk_schema().chunk_id().as_tuid(),

                batch: chunk_batch.into(),
                on_release: None,
            };

            LogMsg::ArrowMsg(store_id, arrow_msg)
        }

        re_protos::log_msg::v1alpha1::log_msg::Msg::BlueprintActivationCommand(
            blueprint_activation_command,
        ) => {
            //TODO(#10730): clean that up when removing 0.24 back compat
            let blueprint_id: re_log_types::StoreId = match blueprint_activation_command
                .blueprint_id
                .ok_or_else(|| {
                    missing_field!(
                        re_protos::log_msg::v1alpha1::BlueprintActivationCommand,
                        "blueprint_id"
                    )
                })?
                .try_into()
            {
                Ok(store_id) => store_id,
                Err(err) => {
                    let Some(store_id) = app_id_injector.recover_store_id(err.clone()) else {
                        return Err(err.into());
                    };

                    store_id
                }
            };

            LogMsg::BlueprintActivationCommand(BlueprintActivationCommand {
                blueprint_id,
                make_active: blueprint_activation_command.make_active,
                make_default: blueprint_activation_command.make_default,
            })
        }
    };

    Ok(msg)
}
