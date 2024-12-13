use super::MessageHeader;
use super::TransportMessageV0;
use crate::codec::arrow::write_arrow_to_bytes;
use crate::codec::CodecError;
use re_chunk::TransportChunk;

impl MessageHeader {
    pub(crate) fn encode(&self, write: &mut impl std::io::Write) -> Result<(), CodecError> {
        write
            .write_all(&[self.0])
            .map_err(CodecError::HeaderEncoding)?;

        Ok(())
    }
}

impl TransportMessageV0 {
    pub(crate) fn to_bytes(&self) -> Result<Vec<u8>, CodecError> {
        match self {
            Self::NoData => {
                let mut data: Vec<u8> = Vec::new();
                MessageHeader::NO_DATA.encode(&mut data)?;
                Ok(data)
            }
            Self::RecordBatch(chunk) => {
                let mut data: Vec<u8> = Vec::new();
                MessageHeader::RECORD_BATCH.encode(&mut data)?;

                write_arrow_to_bytes(&mut data, &chunk.schema, &chunk.data)?;

                Ok(data)
            }
        }
    }
}

/// Encode a `NoData` message into a byte stream. This can be used by the remote store
/// (i.e. data producer) to signal back to the client that there's no data available.
pub fn no_data(version: re_protos::common::v0::EncoderVersion) -> Result<Vec<u8>, CodecError> {
    match version {
        re_protos::common::v0::EncoderVersion::V0 => TransportMessageV0::NoData.to_bytes(),
    }
}

// TODO(zehiko) add support for separately encoding schema from the record batch to get rid of overhead
// of sending schema in each transport message for the same stream of batches. This will require codec
// to become stateful and keep track if schema was sent / received.
/// Encode a transport chunk into a byte stream.
pub fn encode(
    version: re_protos::common::v0::EncoderVersion,
    chunk: TransportChunk,
) -> Result<Vec<u8>, CodecError> {
    match version {
        re_protos::common::v0::EncoderVersion::V0 => {
            TransportMessageV0::RecordBatch(chunk).to_bytes()
        }
    }
}
