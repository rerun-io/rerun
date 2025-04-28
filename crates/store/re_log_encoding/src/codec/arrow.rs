use super::CodecError;

use arrow::array::RecordBatch as ArrowRecordBatch;

/// Helper function that serializes given arrow schema and record batch into bytes
/// using Arrow IPC format.
pub(crate) fn write_arrow_to_bytes<W: std::io::Write>(
    writer: &mut W,
    batch: &ArrowRecordBatch,
) -> Result<(), CodecError> {
    let mut sw = arrow::ipc::writer::StreamWriter::try_new(writer, batch.schema_ref())
        .map_err(CodecError::ArrowSerialization)?;
    sw.write(batch).map_err(CodecError::ArrowSerialization)?;
    sw.finish().map_err(CodecError::ArrowSerialization)?;

    Ok(())
}

/// Helper function that deserializes raw bytes into arrow schema and record batch
/// using Arrow IPC format.
///
/// Returns only the first record batch in the stream.
pub(crate) fn read_arrow_from_bytes<R: std::io::Read>(
    reader: &mut R,
) -> Result<ArrowRecordBatch, CodecError> {
    let mut stream = arrow::ipc::reader::StreamReader::try_new(reader, None)
        .map_err(CodecError::ArrowDeserialization)?;

    stream
        .next()
        .ok_or(CodecError::MissingRecordBatch)?
        .map_err(CodecError::ArrowDeserialization)
}

#[cfg(feature = "encoder")]
pub(crate) struct Payload<'a> {
    pub uncompressed_size: usize,
    // NOTE: This is a `&mut` to ensure exclusivity
    pub data: &'a mut Vec<u8>,
}

#[cfg(feature = "encoder")]
pub(crate) struct ArrowEncodingContext {
    uncompressed: Vec<u8>,
    compressed: Vec<u8>,
}

#[cfg(feature = "encoder")]
impl ArrowEncodingContext {
    pub fn new() -> Self {
        Self {
            uncompressed: Vec::new(),
            compressed: Vec::new(),
        }
    }
}

#[cfg(feature = "encoder")]
pub(crate) fn encode_arrow_with_ctx<'a>(
    arrow_ctx: &'a mut ArrowEncodingContext,
    batch: &ArrowRecordBatch,
    compression: crate::Compression,
) -> Result<Payload<'a>, crate::encoder::EncodeError> {
    let ArrowEncodingContext {
        uncompressed,
        compressed,
    } = arrow_ctx;

    uncompressed.clear();
    write_arrow_to_bytes(uncompressed, batch)?;
    let uncompressed_size = uncompressed.len();

    let data = match compression {
        crate::Compression::Off => &mut *uncompressed,
        crate::Compression::LZ4 => {
            compressed.resize(
                lz4_flex::block::get_maximum_output_size(uncompressed.len()),
                0,
            );
            lz4_flex::block::compress_into(&uncompressed, compressed)?;
            &mut *compressed
        }
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
