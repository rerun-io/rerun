use super::{MessageHeader, MessageKind};
use crate::codec::CodecError;
use crate::codec::arrow::decode_arrow;
use crate::decoder::DecodeError;
use re_log_types::LogMsg;
use re_protos::missing_field;

pub(crate) fn decode(data: &mut impl std::io::Read) -> Result<(u64, Option<LogMsg>), DecodeError> {
    let mut read_bytes = 0u64;
    let header = MessageHeader::decode(data)?;
    read_bytes += std::mem::size_of::<MessageHeader>() as u64 + header.len;

    let mut buf = vec![0; header.len as usize];
    data.read_exact(&mut buf[..])?;

    let msg = decode_bytes(header.kind, &buf)?;

    Ok((read_bytes, msg))
}

/// Decode a message of kind `message_kind` from `buf`.
///
/// `Ok(None)` returned from this function marks the end of the file stream.
#[tracing::instrument(level = "trace", skip_all)]
pub fn decode_bytes(message_kind: MessageKind, buf: &[u8]) -> Result<Option<LogMsg>, DecodeError> {
    use re_protos::external::prost::Message as _;
    use re_protos::log_msg::v1alpha1::{
        ArrowMsg, BlueprintActivationCommand, Encoding, SetStoreInfo,
    };

    let msg = match message_kind {
        MessageKind::SetStoreInfo => {
            let set_store_info = SetStoreInfo::decode(buf)?;
            Some(LogMsg::SetStoreInfo(set_store_info.try_into()?))
        }
        MessageKind::ArrowMsg => {
            let arrow_msg = ArrowMsg::decode(buf)?;
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

            let chunk = re_chunk::Chunk::from_record_batch(&batch)?;

            Some(LogMsg::ArrowMsg(store_id, chunk.to_arrow_msg()?))
        }
        MessageKind::BlueprintActivationCommand => {
            let blueprint_activation_command = BlueprintActivationCommand::decode(buf)?;
            Some(LogMsg::BlueprintActivationCommand(
                blueprint_activation_command.try_into()?,
            ))
        }
        MessageKind::End => None,
    };

    Ok(msg)
}
