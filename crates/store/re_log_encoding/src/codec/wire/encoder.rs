use re_protos::common::v1alpha1::RerunChunk;
use re_protos::remote_store::v1alpha1::DataframePart;

use arrow::array::RecordBatch as ArrowRecordBatch;

use crate::codec::arrow::write_arrow_to_bytes;
use crate::codec::CodecError;

/// Encode a transport chunk into a byte stream.
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
pub trait Encode<O> {
    fn encode(&self) -> Result<O, CodecError>;
}

impl Encode<DataframePart> for ArrowRecordBatch {
    fn encode(&self) -> Result<DataframePart, CodecError> {
        let payload = encode(re_protos::common::v1alpha1::EncoderVersion::V0, self)?;
        Ok(DataframePart {
            encoder_version: re_protos::common::v1alpha1::EncoderVersion::V0 as i32,
            payload,
        })
    }
}

impl Encode<RerunChunk> for ArrowRecordBatch {
    fn encode(&self) -> Result<RerunChunk, CodecError> {
        let payload = encode(re_protos::common::v1alpha1::EncoderVersion::V0, self)?;
        Ok(RerunChunk {
            encoder_version: re_protos::common::v1alpha1::EncoderVersion::V0 as i32,
            payload,
        })
    }
}
