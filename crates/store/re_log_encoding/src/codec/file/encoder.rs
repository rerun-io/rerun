use super::{MessageHeader, MessageKind};
use crate::codec::arrow::write_arrow_to_bytes;
use crate::encoder::EncodeError;
use crate::Compression;
use re_log_types::LogMsg;

impl MessageKind {
    pub(crate) fn encode(&self, buf: &mut impl std::io::Write) -> Result<(), EncodeError> {
        let kind: u32 = *self as u32;
        buf.write_all(&kind.to_le_bytes())?;
        Ok(())
    }
}

impl MessageHeader {
    pub(crate) fn encode(&self, buf: &mut impl std::io::Write) -> Result<(), EncodeError> {
        self.kind.encode(buf)?;
        buf.write_all(&self.len.to_le_bytes())?;
        Ok(())
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
