use arrow::array::RecordBatch as ArrowRecordBatch;

use re_protos::common::v1alpha1::RerunChunk;
use re_protos::remote_store::v1alpha1::DataframePart;

use crate::codec::arrow::read_arrow_from_bytes;
use crate::codec::CodecError;

/// Decode transport data from a byte stream.
fn decode(
    version: re_protos::common::v1alpha1::EncoderVersion,
    data: &[u8],
) -> Result<ArrowRecordBatch, CodecError> {
    match version {
        re_protos::common::v1alpha1::EncoderVersion::Unspecified => {
            Err(CodecError::UnsupportedEncoding)
        }

        re_protos::common::v1alpha1::EncoderVersion::V0 => {
            let mut reader = std::io::Cursor::new(data);
            let batch = read_arrow_from_bytes(&mut reader)?;
            Ok(batch)
        }
    }
}

/// Decode an object from a its wire (protobuf) representation.
pub trait Decode {
    fn decode(&self) -> Result<ArrowRecordBatch, CodecError>;
}

impl Decode for DataframePart {
    fn decode(&self) -> Result<ArrowRecordBatch, CodecError> {
        decode(self.encoder_version(), &self.payload)
    }
}

impl Decode for RerunChunk {
    fn decode(&self) -> Result<ArrowRecordBatch, CodecError> {
        decode(self.encoder_version(), &self.payload)
    }
}

impl Decode for re_protos::common::v1alpha1::DataframePart {
    fn decode(&self) -> Result<ArrowRecordBatch, CodecError> {
        let payload = self
            .payload
            .as_ref()
            .ok_or(CodecError::MissingRecordBatch)?;
        decode(self.encoder_version(), payload)
    }
}
