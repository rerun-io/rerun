use re_protos::common::v1alpha1::RerunChunk;

use arrow::array::RecordBatch as ArrowRecordBatch;

use crate::codec::CodecError;
use crate::codec::arrow::write_arrow_to_bytes;

/// Encode a transport chunk into a byte stream.
//
// TODO: well it doesn't even take a transport chunk, for one.
fn encode(
    version: re_protos::common::v1alpha1::EncoderVersion,
    batch: &ArrowRecordBatch,
) -> Result<Vec<u8>, CodecError> {
    match version {
        re_protos::common::v1alpha1::EncoderVersion::Unspecified => {
            Err(CodecError::UnsupportedEncoding)
        }

        re_protos::common::v1alpha1::EncoderVersion::V0 => {
            let mut data: Vec<u8> = Vec::new();
            write_arrow_to_bytes(&mut data, batch)?;
            Ok(data)
        }
    }
}

/// Encode an object into a wire (protobuf) type.
//
// TODO: okay what on earth is this? okay i dont really have a clue why this is a thing but sure, why not
// TODO: if you're an extension trait, please name yourself as such.
pub trait Encode<O> {
    fn encode(&self) -> Result<O, CodecError>;
}

// TODO: just use the qualified path here, so we know this is in fact a protobuf conversion
impl Encode<RerunChunk> for ArrowRecordBatch {
    fn encode(&self) -> Result<RerunChunk, CodecError> {
        let payload = encode(re_protos::common::v1alpha1::EncoderVersion::V0, self)?;
        Ok(RerunChunk {
            encoder_version: re_protos::common::v1alpha1::EncoderVersion::V0 as i32,
            payload: payload.into(),
        })
    }
}

impl Encode<re_protos::common::v1alpha1::DataframePart> for ArrowRecordBatch {
    fn encode(&self) -> Result<re_protos::common::v1alpha1::DataframePart, CodecError> {
        let payload = encode(re_protos::common::v1alpha1::EncoderVersion::V0, self)?;
        Ok(re_protos::common::v1alpha1::DataframePart {
            encoder_version: re_protos::common::v1alpha1::EncoderVersion::V0 as i32,
            payload: Some(payload.into()),
        })
    }
}
