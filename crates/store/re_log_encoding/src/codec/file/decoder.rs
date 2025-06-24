use re_log_types::LogMsg;
use re_protos::missing_field;

use crate::codec::CodecError;
use crate::codec::arrow::decode_arrow;
use crate::decoder::DecodeError;

use super::{MessageHeader, MessageKind};

// ---

pub(crate) fn decode(data: &mut impl std::io::Read) -> Result<(u64, Option<LogMsg>), DecodeError> {
    let mut read_bytes = 0u64;
    let header = MessageHeader::decode(data)?;
    read_bytes += std::mem::size_of::<MessageHeader>() as u64 + header.len;

    let mut buf = vec![0; header.len as usize];
    data.read_exact(&mut buf[..])?;

    let msg = decode_bytes_to_app(header.kind, &buf)?;

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
#[tracing::instrument(level = "trace", skip_all)]
pub fn decode_bytes_to_app(
    message_kind: MessageKind,
    buf: &[u8],
) -> Result<Option<LogMsg>, DecodeError> {
    let decoded = decode_bytes_to_transport(message_kind, buf)?;
    let decoded = decoded.map(decode_transport_to_app);
    decoded.transpose()
}

/// Decode a message of kind `message_kind` from `buf`.
///
/// This only decodes from raw bytes up to transport-level types (i.e. Protobuf payloads are
/// decoded, but Arrow data is never touched).
///
/// `Ok(None)` returned from this function marks the end of the file stream.
#[tracing::instrument(level = "trace", skip_all)]
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
#[tracing::instrument(level = "trace", skip_all)]
pub fn decode_transport_to_app(
    msg: re_protos::log_msg::v1alpha1::log_msg::Msg,
) -> Result<LogMsg, DecodeError> {
    use re_protos::log_msg::v1alpha1::Encoding;

    let msg = match msg {
        re_protos::log_msg::v1alpha1::log_msg::Msg::SetStoreInfo(set_store_info) => {
            LogMsg::SetStoreInfo(set_store_info.try_into()?)
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

            let store_id: re_log_types::StoreId = arrow_msg
                .store_id
                .ok_or_else(|| missing_field!(re_protos::log_msg::v1alpha1::ArrowMsg, "store_id"))?
                .into();

            let sorbet_schema = re_sorbet::ChunkSchema::try_from(batch.schema_ref().as_ref())?;

            let arrow_msg = re_log_types::ArrowMsg {
                chunk_id: sorbet_schema.chunk_id().as_tuid(),
                batch,
                on_release: None,
            };

            LogMsg::ArrowMsg(store_id, arrow_msg)
        }

        re_protos::log_msg::v1alpha1::log_msg::Msg::BlueprintActivationCommand(
            blueprint_activation_command,
        ) => LogMsg::BlueprintActivationCommand(blueprint_activation_command.try_into()?),
    };

    Ok(msg)
}
