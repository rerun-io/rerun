use super::CodecError;

use arrow::array::RecordBatch as ArrowRecordBatch;

/// TODO(#3741): switch to arrow1 once <https://github.com/apache/arrow-rs/issues/6803> is released
const SERIALIZE_WITH_ARROW_1: bool = false; // I _think_ we can use arrow1 here, because we don't encounter the above bug in this context
const DESERIALIZE_WITH_ARROW_1: bool = true; // Both arrow1 and arrow2 should be working fine

/// Helper function that serializes given arrow schema and record batch into bytes
/// using Arrow IPC format.
pub(crate) fn write_arrow_to_bytes<W: std::io::Write>(
    writer: &mut W,
    batch: &ArrowRecordBatch,
) -> Result<(), CodecError> {
    if SERIALIZE_WITH_ARROW_1 {
        #[allow(clippy::disallowed_types)] // it's behind a disabled feature flag
        let mut sw = arrow::ipc::writer::StreamWriter::try_new(writer, batch.schema_ref())
            .map_err(CodecError::ArrowSerialization)?;
        sw.write(batch).map_err(CodecError::ArrowSerialization)?;
        sw.finish().map_err(CodecError::ArrowSerialization)?;
    } else {
        let schema = arrow2::datatypes::Schema::from(batch.schema());
        let chunk = arrow2::chunk::Chunk::new(
            batch
                .columns()
                .iter()
                .map(|c| -> Box<dyn arrow2::array::Array> { c.clone().into() })
                .collect(),
        );

        let mut writer = arrow2::io::ipc::write::StreamWriter::new(writer, Default::default());
        writer
            .start(&schema, None)
            .map_err(CodecError::Arrow2Serialization)?;
        writer
            .write(&chunk, None)
            .map_err(CodecError::Arrow2Serialization)?;
        writer.finish().map_err(CodecError::Arrow2Serialization)?;
    }

    Ok(())
}

/// Helper function that deserializes raw bytes into arrow schema and record batch
/// using Arrow IPC format.
///
/// Returns only the first record batch in the stream.
pub(crate) fn read_arrow_from_bytes<R: std::io::Read>(
    reader: &mut R,
) -> Result<ArrowRecordBatch, CodecError> {
    if DESERIALIZE_WITH_ARROW_1 {
        let mut stream = arrow::ipc::reader::StreamReader::try_new(reader, None)
            .map_err(CodecError::ArrowDeserialization)?;

        stream
            .next()
            .ok_or(CodecError::MissingRecordBatch)?
            .map_err(CodecError::ArrowDeserialization)
    } else {
        use arrow2::io::ipc;

        let metadata =
            ipc::read::read_stream_metadata(reader).map_err(CodecError::Arrow2Serialization)?;
        let mut stream = ipc::read::StreamReader::new(reader, metadata, None);

        let schema = stream.schema().clone();
        // there should be at least one record batch in the stream
        let stream_state = stream
            .next()
            .ok_or(CodecError::MissingRecordBatch)?
            .map_err(CodecError::Arrow2Serialization)?;

        match stream_state {
            ipc::read::StreamState::Waiting => Err(CodecError::UnexpectedStreamState),
            ipc::read::StreamState::Some(chunk) => {
                let batch = ArrowRecordBatch::try_new(
                    schema.into(),
                    chunk.columns().iter().map(|c| c.clone().into()).collect(),
                )
                .map_err(CodecError::ArrowDeserialization)?;
                Ok(batch)
            }
        }
    }
}

#[cfg(feature = "encoder")]
pub(crate) struct Payload {
    pub uncompressed_size: usize,
    pub data: Vec<u8>,
}

#[cfg(feature = "encoder")]
pub(crate) fn encode_arrow(
    batch: &ArrowRecordBatch,
    compression: crate::Compression,
) -> Result<Payload, crate::encoder::EncodeError> {
    let mut uncompressed = Vec::new();
    write_arrow_to_bytes(&mut uncompressed, batch)?;
    let uncompressed_size = uncompressed.len();

    let data = match compression {
        crate::Compression::Off => uncompressed,
        crate::Compression::LZ4 => lz4_flex::block::compress(&uncompressed),
    };

    Ok(Payload {
        uncompressed_size,
        data,
    })
}

#[cfg(feature = "decoder")]
pub(crate) fn decode_arrow(
    data: &[u8],
    uncompressed_size: usize,
    compression: crate::Compression,
) -> Result<ArrowRecordBatch, crate::decoder::DecodeError> {
    let mut uncompressed = Vec::new();
    let data = match compression {
        crate::Compression::Off => data,
        crate::Compression::LZ4 => {
            uncompressed.resize(uncompressed_size, 0);
            lz4_flex::block::decompress_into(data, &mut uncompressed)?;
            uncompressed.as_slice()
        }
    };

    Ok(read_arrow_from_bytes(&mut &data[..])?)
}
