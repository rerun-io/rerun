use super::MessageHeader;
use super::TransportMessageV0;
use crate::codec::arrow::read_arrow_from_bytes;
use crate::codec::CodecError;
use re_chunk::TransportChunk;

impl MessageHeader {
    pub(crate) fn decode(read: &mut impl std::io::Read) -> Result<Self, CodecError> {
        let mut buffer = [0_u8; Self::SIZE_BYTES];
        read.read_exact(&mut buffer)
            .map_err(CodecError::HeaderDecoding)?;

        let header = u8::from_le(buffer[0]);

        Ok(Self(header))
    }
}

impl TransportMessageV0 {
    pub(crate) fn from_bytes(data: &[u8]) -> Result<Self, CodecError> {
        let mut reader = std::io::Cursor::new(data);
        let header = MessageHeader::decode(&mut reader)?;

        match header {
            MessageHeader::NO_DATA => Ok(Self::NoData),
            MessageHeader::RECORD_BATCH => {
                let (schema, data) = read_arrow_from_bytes(&mut reader)?;

                let tc = TransportChunk {
                    schema: schema.clone(),
                    data,
                };

                Ok(Self::RecordBatch(tc))
            }
            _ => Err(CodecError::UnknownMessageHeader),
        }
    }
}

/// Decode transport data from a byte stream - if there's a record batch present, return it, otherwise return `None`.
pub fn decode(
    version: re_protos::common::v0::EncoderVersion,
    data: &[u8],
) -> Result<Option<TransportChunk>, CodecError> {
    match version {
        re_protos::common::v0::EncoderVersion::V0 => {
            let msg = TransportMessageV0::from_bytes(data)?;
            match msg {
                TransportMessageV0::RecordBatch(chunk) => Ok(Some(chunk)),
                TransportMessageV0::NoData => Ok(None),
            }
        }
    }
}
