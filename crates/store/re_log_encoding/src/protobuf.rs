use re_log_types::LogMsg;
use re_protos::TypeConversionError;

use crate::codec::CodecError;
use crate::decoder::DecodeError;
use crate::encoder::EncodeError;
use crate::Compression;

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub(crate) enum MessageKind {
    SetStoreInfo = 1,
    ArrowMsg = 2,
    BlueprintActivationCommand = 3,
    End = 255,
}

impl MessageKind {
    pub(crate) fn encode(&self, buf: &mut impl std::io::Write) -> Result<(), EncodeError> {
        let kind: u32 = *self as u32;
        buf.write_all(&kind.to_le_bytes())?;
        Ok(())
    }

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

#[derive(Debug, Clone, Copy)]
pub(crate) struct MessageHeader {
    pub(crate) kind: MessageKind,
    pub(crate) len: u32,
}

impl MessageHeader {
    pub(crate) fn encode(&self, buf: &mut impl std::io::Write) -> Result<(), EncodeError> {
        self.kind.encode(buf)?;
        buf.write_all(&self.len.to_le_bytes())?;
        Ok(())
    }

    pub(crate) fn decode(data: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        let kind = MessageKind::decode(data)?;
        let mut buf = [0; 4];
        data.read_exact(&mut buf)?;
        let len = u32::from_le_bytes(buf);

        Ok(Self { kind, len })
    }
}

pub(crate) fn encode(
    buf: &mut Vec<u8>,
    message: &LogMsg,
    compression: Compression,
) -> Result<(), EncodeError> {
    use re_protos::external::prost::Message;
    use re_protos::log_msg::v0::{
        self as proto, ArrowMsg, BlueprintActivationCommand, Encoding, SetStoreInfo,
    };

    match message {
        LogMsg::SetStoreInfo(set_store_info) => {
            let set_store_info: SetStoreInfo = set_store_info.clone().into();
            let header = MessageHeader {
                kind: MessageKind::SetStoreInfo,
                len: set_store_info.encoded_len() as u32,
            };
            header.encode(buf)?;
            set_store_info.encode(buf)?;
        }
        LogMsg::ArrowMsg(store_id, arrow_msg) => {
            let arrow_msg = ArrowMsg {
                store_id: Some(store_id.clone().into()),
                compression: match compression {
                    Compression::Off => proto::Compression::None as i32,
                    Compression::LZ4 => proto::Compression::Lz4 as i32,
                },
                encoding: Encoding::ArrowIpc as i32,
                payload: encode_arrow(&arrow_msg.schema, &arrow_msg.chunk, compression)?,
            };
            let header = MessageHeader {
                kind: MessageKind::ArrowMsg,
                len: arrow_msg.encoded_len() as u32,
            };
            header.encode(buf)?;
            arrow_msg.encode(buf)?;
        }
        LogMsg::BlueprintActivationCommand(blueprint_activation_command) => {
            let blueprint_activation_command: BlueprintActivationCommand =
                blueprint_activation_command.clone().into();
            let header = MessageHeader {
                kind: MessageKind::BlueprintActivationCommand,
                len: blueprint_activation_command.encoded_len() as u32,
            };
            header.encode(buf)?;
            blueprint_activation_command.encode(buf)?;
        }
    }

    Ok(())
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

fn encode_arrow(
    schema: &arrow2::datatypes::Schema,
    chunk: &arrow2::chunk::Chunk<Box<dyn re_chunk::Arrow2Array>>,
    compression: crate::Compression,
) -> Result<Vec<u8>, EncodeError> {
    let mut uncompressed = Vec::new();
    write_arrow_to_bytes(&mut uncompressed, schema, chunk)?;

    match compression {
        crate::Compression::Off => Ok(uncompressed),
        crate::Compression::LZ4 => Ok(lz4_flex::block::compress(&uncompressed)),
    }
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

/// Helper function that serializes given arrow schema and record batch into bytes
/// using Arrow IPC format.
fn write_arrow_to_bytes<W: std::io::Write>(
    writer: &mut W,
    schema: &arrow2::datatypes::Schema,
    data: &arrow2::chunk::Chunk<Box<dyn re_chunk::Arrow2Array>>,
) -> Result<(), CodecError> {
    use arrow2::io::ipc;

    let options = ipc::write::WriteOptions { compression: None };
    let mut sw = ipc::write::StreamWriter::new(writer, options);

    sw.start(schema, None)
        .map_err(CodecError::ArrowSerialization)?;
    sw.write(data, None)
        .map_err(CodecError::ArrowSerialization)?;
    sw.finish().map_err(CodecError::ArrowSerialization)?;

    Ok(())
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
