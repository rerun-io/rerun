use arrow2::chunk::Chunk as Arrow2Chunk;
use arrow2::datatypes::Schema as Arrow2Schema;
use arrow2::io::ipc;
use re_chunk::Arrow2Array;
use re_chunk::TransportChunk;

use super::CodecError;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MessageHeader(pub u8);

impl MessageHeader {
    pub const NO_DATA: Self = Self(1);
    pub const RECORD_BATCH: Self = Self(2);

    pub const SIZE_BYTES: usize = 1;
}

impl MessageHeader {
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

    fn from_bytes(data: &[u8]) -> Result<Self, CodecError> {
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

/// Encode a `NoData` message into a byte stream. This can be used by the remote store
/// (i.e. data producer) to signal back to the client that there's no data available.
pub fn no_data(version: re_protos::common::v0::EncoderVersion) -> Result<Vec<u8>, CodecError> {
    match version {
        re_protos::common::v0::EncoderVersion::V0 => TransportMessageV0::NoData.to_bytes(),
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

/// Helper function that serializes given arrow schema and record batch into bytes
/// using Arrow IPC format.
pub fn write_arrow_to_bytes<W: std::io::Write>(
    writer: &mut W,
    schema: &Arrow2Schema,
    data: &Arrow2Chunk<Box<dyn Arrow2Array>>,
) -> Result<(), CodecError> {
    let options = ipc::write::WriteOptions { compression: None };
    let mut sw = ipc::write::StreamWriter::new(writer, options);

    sw.start(schema, None)
        .map_err(CodecError::ArrowSerialization)?;
    sw.write(data, None)
        .map_err(CodecError::ArrowSerialization)?;
    sw.finish().map_err(CodecError::ArrowSerialization)?;

    Ok(())
}

/// Helper function that deserializes raw bytes into arrow schema and record batch
/// using Arrow IPC format.
pub fn read_arrow_from_bytes<R: std::io::Read>(
    reader: &mut R,
) -> Result<(Arrow2Schema, Arrow2Chunk<Box<dyn Arrow2Array>>), CodecError> {
    let metadata =
        ipc::read::read_stream_metadata(reader).map_err(CodecError::ArrowSerialization)?;
    let mut stream = ipc::read::StreamReader::new(reader, metadata, None);

    let schema = stream.schema().clone();
    // there should be at least one record batch in the stream
    let stream_state = stream
        .next()
        .ok_or(CodecError::MissingRecordBatch)?
        .map_err(CodecError::ArrowSerialization)?;

    match stream_state {
        ipc::read::StreamState::Waiting => Err(CodecError::UnexpectedStreamState),
        ipc::read::StreamState::Some(chunk) => Ok((schema, chunk)),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        codec::wire::{decode, encode, TransportMessageV0},
        codec::CodecError,
    };
    use re_chunk::{Chunk, RowId};
    use re_log_types::{example_components::MyPoint, Timeline};
    use re_protos::common::v0::EncoderVersion;

    fn get_test_chunk() -> Chunk {
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

        Chunk::builder("mypoints".into())
            .with_component_batches(row_id1, timepoint1, [points1 as _])
            .with_component_batches(row_id2, timepoint2, [points2 as _])
            .build()
            .unwrap()
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
        let expected_chunk = get_test_chunk();

        let msg = TransportMessageV0::RecordBatch(expected_chunk.clone().to_transport().unwrap());
        let data = msg.to_bytes().unwrap();
        let decoded = TransportMessageV0::from_bytes(&data).unwrap();

        #[allow(clippy::match_wildcard_for_single_variants)]
        match decoded {
            TransportMessageV0::RecordBatch(transport) => {
                let decoded_chunk = Chunk::from_transport(&transport).unwrap();
                assert_eq!(expected_chunk, decoded_chunk);
            }
            _ => panic!("unexpected message type"),
        }
    }

    #[test]
    fn test_invalid_batch_data() {
        let data = vec![2, 3, 4]; // '1' is NO_DATA message header
        let decoded = TransportMessageV0::from_bytes(&data);

        assert!(matches!(
            decoded.err().unwrap(),
            CodecError::ArrowSerialization(_)
        ));
    }

    #[test]
    fn test_unknown_header() {
        let data = vec![3];
        let decoded = TransportMessageV0::from_bytes(&data);
        assert!(decoded.is_err());

        assert!(matches!(
            decoded.err().unwrap(),
            CodecError::UnknownMessageHeader
        ));
    }

    #[test]
    fn test_v0_codec() {
        let expected_chunk = get_test_chunk();

        let encoded = encode(
            EncoderVersion::V0,
            expected_chunk.clone().to_transport().unwrap(),
        )
        .unwrap();
        let decoded = decode(EncoderVersion::V0, &encoded).unwrap().unwrap();
        let decoded_chunk = Chunk::from_transport(&decoded).unwrap();

        assert_eq!(expected_chunk, decoded_chunk);
    }
}
