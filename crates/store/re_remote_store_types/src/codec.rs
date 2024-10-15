use re_dataframe::external::re_chunk::TransportChunk;
use re_log_types::external::arrow2::error::Error as ArrowError;
use re_log_types::external::arrow2::io::ipc::{read, write};

use crate::v0::EncoderVersion;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("Arrow serialization error: {0}")]
    ArrowSerialization(ArrowError),

    #[error("Failed to decode message header {0}")]
    HeaderDecoding(std::io::Error),

    #[error("Failed to encode message header {0}")]
    HeaderEncoding(std::io::Error),

    #[error("Missing record batch")]
    MissingRecordBatch,

    #[error("Unexpected stream state")]
    UnexpectedStreamState,

    #[error("Unknown message header")]
    UnknownMessageHeader,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MessageHader(pub u8);

impl MessageHader {
    pub const NO_DATA: Self = Self(0);
    pub const RECORD_BATCH: Self = Self(1);

    pub const SIZE_BYTES: usize = 1;
}

impl MessageHader {
    fn decode(read: &mut impl std::io::Read) -> Result<Self, CodecError> {
        let mut buffer = [0_u8; Self::SIZE_BYTES];
        read.read_exact(&mut buffer)
            .map_err(CodecError::HeaderDecoding)?;

        let header = u8::from_le(buffer[0]);

        Ok(Self(header))
    }

    fn encode(&self, write: &mut impl std::io::Write) -> Result<(), CodecError> {
        write
            .write_all(&[self.0])
            .map_err(CodecError::HeaderEncoding)?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum TransportMessageV0 {
    NoData,
    RecordBatch(TransportChunk),
}

impl TransportMessageV0 {
    fn to_bytes(&self) -> Result<Vec<u8>, CodecError> {
        match self {
            Self::NoData => {
                let mut data: Vec<u8> = Vec::new();
                MessageHader::NO_DATA.encode(&mut data)?;
                Ok(data)
            }
            Self::RecordBatch(chunk) => {
                let mut data: Vec<u8> = Vec::new();
                MessageHader::RECORD_BATCH.encode(&mut data)?;

                let options = write::WriteOptions { compression: None };
                let mut sw = write::StreamWriter::new(&mut data, options);

                sw.start(&chunk.schema, None)
                    .map_err(CodecError::ArrowSerialization)?;
                sw.write(&chunk.data, None)
                    .map_err(CodecError::ArrowSerialization)?;
                sw.finish().map_err(CodecError::ArrowSerialization)?;

                Ok(data)
            }
        }
    }

    fn from_bytes(data: &[u8]) -> Result<Self, CodecError> {
        let mut reader = std::io::Cursor::new(data);
        let header = MessageHader::decode(&mut reader)?;

        match header {
            MessageHader::NO_DATA => Ok(Self::NoData),
            MessageHader::RECORD_BATCH => {
                let metadata = read::read_stream_metadata(&mut reader)
                    .map_err(CodecError::ArrowSerialization)?;
                let mut sr = read::StreamReader::new(&mut reader, metadata, None);

                let schema = sr.schema().clone();
                // there should be at least one record batch in the stream
                // TODO(zehiko) isn't there a "read one record batch from bytes" function??
                let stream_state = sr
                    .next()
                    .ok_or(CodecError::MissingRecordBatch)?
                    .map_err(CodecError::ArrowSerialization)?;

                match stream_state {
                    read::StreamState::Waiting => Err(CodecError::UnexpectedStreamState),
                    read::StreamState::Some(chunk) => {
                        let tc = TransportChunk {
                            schema: schema.clone(),
                            data: chunk,
                        };

                        Ok(Self::RecordBatch(tc))
                    }
                }
            }
            _ => Err(CodecError::UnknownMessageHeader),
        }
    }
}

/// Encode a transport chunk into a byte stream.
pub fn encode(version: EncoderVersion, chunk: TransportChunk) -> Result<Vec<u8>, CodecError> {
    match version {
        EncoderVersion::V0 => TransportMessageV0::RecordBatch(chunk).to_bytes(),
    }
}

/// Encode a `NoData` message into a byte stream.
pub fn no_data(version: EncoderVersion) -> Result<Vec<u8>, CodecError> {
    match version {
        EncoderVersion::V0 => TransportMessageV0::NoData.to_bytes(),
    }
}

/// Decode transport data from a byte stream - if there's a record batch present, return it, otherwise return `None`.
pub fn decode(version: EncoderVersion, data: &[u8]) -> Result<Option<TransportChunk>, CodecError> {
    match version {
        EncoderVersion::V0 => {
            let msg = TransportMessageV0::from_bytes(data)?;
            match msg {
                TransportMessageV0::RecordBatch(chunk) => Ok(Some(chunk)),
                TransportMessageV0::NoData => Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use re_dataframe::external::re_chunk::{Chunk, RowId, TransportChunk};
    use re_log_types::{example_components::MyPoint, Timeline};

    use crate::{
        codec::{decode, encode, CodecError, TransportMessageV0},
        v0::EncoderVersion,
    };

    fn get_test_chunk() -> TransportChunk {
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();

        let timepoint1 = [
            (Timeline::log_time(), 100),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint2 = [
            (Timeline::log_time(), 104),
            (Timeline::new_sequence("frame"), 1),
        ];

        let points1 = &[MyPoint::new(1.0, 1.0)];
        let points2 = &[MyPoint::new(2.0, 2.0)];

        let chunk = Chunk::builder("mypoints".into())
            .with_component_batches(row_id1, timepoint1, [points1 as _])
            .with_component_batches(row_id2, timepoint2, [points2 as _])
            .build()
            .unwrap();

        chunk.to_transport().unwrap()
    }

    #[test]
    fn test_message_v0_no_data() {
        let msg = TransportMessageV0::NoData;
        let data = msg.to_bytes().unwrap();
        let decoded = TransportMessageV0::from_bytes(&data).unwrap();
        assert!(matches!(decoded, TransportMessageV0::NoData));
    }

    #[test]
    fn test_message_v0_record_batch() {
        let transport = get_test_chunk();
        let expected_chunk = Chunk::from_transport(&transport).unwrap();

        let msg = TransportMessageV0::RecordBatch(transport);
        let data = msg.to_bytes().unwrap();
        let decoded = TransportMessageV0::from_bytes(&data).unwrap();

        match decoded {
            TransportMessageV0::RecordBatch(transport) => {
                let decoded_chunk = Chunk::from_transport(&transport).unwrap();
                assert_eq!(expected_chunk, decoded_chunk);
            }
            #[allow(clippy::match_wildcard_for_single_variants)]
            _ => panic!("unexpected message type"),
        }
    }

    #[test]
    fn test_invalid_batch_data() {
        let data = vec![1, 2, 3];
        let decoded = TransportMessageV0::from_bytes(&data);

        assert!(matches!(
            decoded.err().unwrap(),
            CodecError::ArrowSerialization(_)
        ));
    }

    #[test]
    fn test_unknown_header() {
        let data = vec![2];
        let decoded = TransportMessageV0::from_bytes(&data);
        assert!(decoded.is_err());

        assert!(matches!(
            decoded.err().unwrap(),
            CodecError::UnknownMessageHeader
        ));
    }

    #[test]
    fn test_v0_codec() {
        let transport_chunk = get_test_chunk();
        let expected_chunk = Chunk::from_transport(&transport_chunk).unwrap();

        let encoded = encode(EncoderVersion::V0, transport_chunk.clone()).unwrap();
        let decoded = decode(EncoderVersion::V0, &encoded).unwrap().unwrap();
        let decoded_chunk = Chunk::from_transport(&decoded).unwrap();

        assert_eq!(expected_chunk, decoded_chunk);
    }
}
