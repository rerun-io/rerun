use crate::codec::arrow::read_arrow_from_bytes;
use crate::codec::CodecError;
use re_chunk::TransportChunk;

/// Decode transport data from a byte stream - if there's a record batch present, return it, otherwise return `None`.
pub fn decode(
    version: re_protos::common::v0::EncoderVersion,
    data: &[u8],
) -> Result<TransportChunk, CodecError> {
    match version {
        re_protos::common::v0::EncoderVersion::V0 => {
            let mut reader = std::io::Cursor::new(data);
            let (schema, data) = read_arrow_from_bytes(&mut reader)?;

            let tc = TransportChunk {
                schema: schema.clone(),
                data,
            };

            Ok(tc)
        }
    }
}
