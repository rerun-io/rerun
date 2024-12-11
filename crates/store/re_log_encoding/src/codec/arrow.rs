use crate::decoder::DecodeError;
use crate::encoder::EncodeError;

use super::CodecError;

use arrow2::array::Array as Arrow2Array;
use arrow2::datatypes::Schema as Arrow2Schema;
use arrow2::io::ipc;

type Arrow2Chunk = arrow2::chunk::Chunk<Box<dyn Arrow2Array>>;

// TODO(#8412): try using arrow ipc `compression` option instead of doing our own compression

/// Helper function that serializes given arrow schema and record batch into bytes
/// using Arrow IPC format.
pub(crate) fn write_arrow_to_bytes<W: std::io::Write>(
    writer: &mut W,
    schema: &Arrow2Schema,
    data: &Arrow2Chunk,
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
pub(crate) fn read_arrow_from_bytes<R: std::io::Read>(
    reader: &mut R,
) -> Result<(Arrow2Schema, Arrow2Chunk), CodecError> {
    use arrow2::io::ipc;

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

pub(crate) struct Payload {
    pub uncompressed_size: usize,
    pub data: Vec<u8>,
}

pub(crate) fn encode_arrow(
    schema: &Arrow2Schema,
    chunk: &Arrow2Chunk,
    compression: crate::Compression,
) -> Result<Payload, EncodeError> {
    let mut uncompressed = Vec::new();
    write_arrow_to_bytes(&mut uncompressed, schema, chunk)?;
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

pub(crate) fn decode_arrow(
    data: &[u8],
    uncompressed_size: usize,
    compression: crate::Compression,
) -> Result<(Arrow2Schema, Arrow2Chunk), DecodeError> {
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
