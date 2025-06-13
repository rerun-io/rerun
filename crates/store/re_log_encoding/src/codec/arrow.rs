use super::CodecError;

use arrow::array::RecordBatch as ArrowRecordBatch;

// Insert timestamp of when the IPC encoding and decoding took place?
//
// This is used for latency measurements, e.g. in gRPC calls.
// It is slightly wasteful to add it to each chunk, even in an .rrd file,
// but the benefit is that we'll always have them, whether we're streaming
// data through gRPC, piping with stdout/stdin, or writing to a file that a viewer is simultaneously reading.
// However, it messes up our roundtrip unit-tests, so we disable it then.
//
// TODO(emilk, andreas, cmc): This is disabled for now since it messes with our tests and
// (worse) means that checksums of files become unstable on read _and_ write.
const INSERT_TIMING_METADATA: bool = false; // !cfg!(feature = "testing");

/// Helper function that serializes given arrow schema and record batch into bytes
/// using Arrow IPC format.
pub(crate) fn write_arrow_to_bytes<W: std::io::Write>(
    writer: &mut W,
    batch: &ArrowRecordBatch,
) -> Result<(), CodecError> {
    re_tracing::profile_function!();

    let mut schema = (*batch.schema()).clone();

    if INSERT_TIMING_METADATA {
        schema.metadata.insert(
            re_sorbet::timestamp_metadata::KEY_TIMESTAMP_IPC_ENCODED.to_owned(),
            re_sorbet::timestamp_metadata::now_timestamp(),
        );
    }

    let mut sw = arrow::ipc::writer::StreamWriter::try_new(writer, &schema)
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
    re_tracing::profile_function!();

    let mut stream = arrow::ipc::reader::StreamReader::try_new(reader, None)
        .map_err(CodecError::ArrowDeserialization)?;

    let mut record_batch = stream
        .next()
        .ok_or(CodecError::MissingRecordBatch)?
        .map_err(CodecError::ArrowDeserialization)?;

    if INSERT_TIMING_METADATA {
        record_batch = re_arrow_util::insert_metadata(
            record_batch,
            re_sorbet::timestamp_metadata::KEY_TIMESTAMP_IPC_DECODED.to_owned(),
            re_sorbet::timestamp_metadata::now_timestamp(),
        );
    }

    Ok(record_batch)
}

#[cfg(feature = "encoder")]
pub(crate) struct Payload {
    pub uncompressed_size: usize,
    pub data: Vec<u8>,
}

#[cfg(feature = "encoder")]
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn encode_arrow(
    batch: &ArrowRecordBatch,
    compression: crate::Compression,
) -> Result<Payload, crate::encoder::EncodeError> {
    re_tracing::profile_function!();

    let mut uncompressed = Vec::new();
    write_arrow_to_bytes(&mut uncompressed, batch)?;
    let uncompressed_size = uncompressed.len();

    let data = match compression {
        crate::Compression::Off => uncompressed,
        crate::Compression::LZ4 => {
            re_tracing::profile_scope!("lz4::compress");
            lz4_flex::block::compress(&uncompressed)
        }
    };

    Ok(Payload {
        uncompressed_size,
        data,
    })
}

#[cfg(feature = "decoder")]
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn decode_arrow(
    data: &[u8],
    uncompressed_size: usize,
    compression: crate::Compression,
) -> Result<ArrowRecordBatch, crate::decoder::DecodeError> {
    let mut uncompressed = Vec::new();
    let data = match compression {
        crate::Compression::Off => data,
        crate::Compression::LZ4 => {
            re_tracing::profile_scope!("LZ4-decompress");
            uncompressed.resize(uncompressed_size, 0);
            lz4_flex::block::decompress_into(data, &mut uncompressed)?;
            uncompressed.as_slice()
        }
    };

    Ok(read_arrow_from_bytes(&mut &data[..])?)
}
