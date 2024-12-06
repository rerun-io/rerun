use super::{MessageHeader, MessageKind};
use crate::decoder::DecodeError;
use crate::{codec::CodecError, Compression};
use re_log_types::LogMsg;
use re_protos::TypeConversionError;

impl MessageKind {
    pub(crate) fn decode(data: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        let mut buf = [0; 4];
        data.read_exact(&mut buf)?;

        match u32::from_le_bytes(buf) {
            1 => Ok(Self::SetStoreInfo),
            2 => Ok(Self::ArrowMsg),
            3 => Ok(Self::BlueprintActivationCommand),
            255 => Ok(Self::End),
            _ => Err(DecodeError::Codec(CodecError::UnknownMessageHeader)),
        }
    }
}

impl MessageHeader {
    pub(crate) fn decode(data: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        let kind = MessageKind::decode(data)?;
        let mut buf = [0; 4];
        data.read_exact(&mut buf)?;
        let len = u32::from_le_bytes(buf);

        Ok(Self { kind, len })
    }
}

pub(crate) fn decode(
    data: &mut impl std::io::Read,
    compression: Compression,
) -> Result<(u64, Option<LogMsg>), DecodeError> {
    use re_protos::external::prost::Message;
    use re_protos::log_msg::v0::{ArrowMsg, BlueprintActivationCommand, Encoding, SetStoreInfo};

    let mut read_bytes = 0u64;
    let header = MessageHeader::decode(data)?;
    read_bytes += std::mem::size_of::<MessageHeader>() as u64 + header.len as u64;

    let mut buf = vec![0; header.len as usize];
    data.read_exact(&mut buf[..])?;

    let msg = match header.kind {
        MessageKind::SetStoreInfo => {
            let set_store_info = SetStoreInfo::decode(&buf[..])?;
            Some(LogMsg::SetStoreInfo(set_store_info.try_into()?))
        }
        MessageKind::ArrowMsg => {
            let arrow_msg = ArrowMsg::decode(&buf[..])?;
            if arrow_msg.encoding() != Encoding::ArrowIpc {
                return Err(DecodeError::Codec(CodecError::UnsupportedEncoding));
            }

            let (schema, chunk) = decode_arrow(&arrow_msg.payload, compression)?;

            let store_id: re_log_types::StoreId = arrow_msg
                .store_id
                .ok_or_else(|| {
                    TypeConversionError::missing_field("rerun.log_msg.v0.ArrowMsg", "store_id")
                })?
                .into();

            let chunk = re_chunk::Chunk::from_transport(&re_chunk::TransportChunk {
                schema,
                data: chunk,
            })?;

            Some(LogMsg::ArrowMsg(store_id, chunk.to_arrow_msg()?))
        }
        MessageKind::BlueprintActivationCommand => {
            let blueprint_activation_command = BlueprintActivationCommand::decode(&buf[..])?;
            Some(LogMsg::BlueprintActivationCommand(
                blueprint_activation_command.try_into()?,
            ))
        }
        MessageKind::End => None,
    };

    Ok((read_bytes, msg))
}

fn decode_arrow(
    data: &[u8],
    compression: crate::Compression,
) -> Result<
    (
        arrow2::datatypes::Schema,
        arrow2::chunk::Chunk<Box<dyn re_chunk::Arrow2Array>>,
    ),
    DecodeError,
> {
    let mut uncompressed = Vec::new();
    let data = match compression {
        crate::Compression::Off => data,
        crate::Compression::LZ4 => {
            lz4_flex::block::decompress_into(data, &mut uncompressed)?;
            uncompressed.as_slice()
        }
    };

    Ok(read_arrow_from_bytes(&mut &data[..])?)
}

/// Helper function that deserializes raw bytes into arrow schema and record batch
/// using Arrow IPC format.
fn read_arrow_from_bytes<R: std::io::Read>(
    reader: &mut R,
) -> Result<
    (
        arrow2::datatypes::Schema,
        arrow2::chunk::Chunk<Box<dyn re_chunk::Arrow2Array>>,
    ),
    CodecError,
> {
    use arrow2::io::ipc;

    let metadata =
        ipc::read::read_stream_metadata(reader).map_err(CodecError::ArrowSerialization)?;
    let mut stream = ipc::read::StreamReader::new(reader, metadata, None);

    let schema = stream.schema().clone();
    // there should be at least one record batch in the stream
    let stream_state = stream
        .next()
        .ok_or(CodecError::MissingRecordBatch)?
        .map_err(CodecError::ArrowSerialization)?;

    match stream_state {
        ipc::read::StreamState::Waiting => Err(CodecError::UnexpectedStreamState),
        ipc::read::StreamState::Some(chunk) => Ok((schema, chunk)),
    }
}
