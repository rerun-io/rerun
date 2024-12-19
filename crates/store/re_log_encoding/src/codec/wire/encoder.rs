use crate::codec::arrow::write_arrow_to_bytes;
use crate::codec::CodecError;
use re_chunk::TransportChunk;

/// Encode a transport chunk into a byte stream.
pub fn encode(
    version: re_protos::common::v0::EncoderVersion,
    chunk: &TransportChunk,
) -> Result<Vec<u8>, CodecError> {
    match version {
        re_protos::common::v0::EncoderVersion::V0 => {
            let mut data: Vec<u8> = Vec::new();
            write_arrow_to_bytes(&mut data, &chunk.schema, &chunk.data)?;

            Ok(data)
        }
    }
}
