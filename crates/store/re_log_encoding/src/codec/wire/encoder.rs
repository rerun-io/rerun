use crate::codec::arrow::write_arrow_to_bytes;
use crate::codec::CodecError;
use re_chunk::TransportChunk;
use re_protos::common::v0::RerunChunk;
use re_protos::remote_store::v0::DataframePart;

/// Encode a transport chunk into a byte stream.
fn encode(
    version: re_protos::common::v0::EncoderVersion,
    chunk: &TransportChunk,
) -> Result<Vec<u8>, CodecError> {
    match version {
        re_protos::common::v0::EncoderVersion::V0 => {
            let mut data: Vec<u8> = Vec::new();
            write_arrow_to_bytes(&mut data, chunk.schema_ref(), &chunk.data)?;

            Ok(data)
        }
    }
}

/// Encode an object into a wire (protobuf) type.
pub trait Encode<O> {
    fn encode(&self) -> Result<O, CodecError>;
}

impl Encode<DataframePart> for TransportChunk {
    fn encode(&self) -> Result<DataframePart, CodecError> {
        let payload = encode(re_protos::common::v0::EncoderVersion::V0, self)?;
        Ok(DataframePart {
            encoder_version: re_protos::common::v0::EncoderVersion::V0 as i32,
            payload,
        })
    }
}

impl Encode<RerunChunk> for TransportChunk {
    fn encode(&self) -> Result<RerunChunk, CodecError> {
        let payload = encode(re_protos::common::v0::EncoderVersion::V0, self)?;
        Ok(RerunChunk {
            encoder_version: re_protos::common::v0::EncoderVersion::V0 as i32,
            payload,
        })
    }
}
