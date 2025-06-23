use super::CodecError;

use arrow::array::RecordBatch as ArrowRecordBatch;

/// Helper function that serializes given arrow schema and record batch into bytes
/// using Arrow IPC format.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn write_arrow_to_bytes<W: std::io::Write>(
    writer: &mut W,
    batch: &ArrowRecordBatch,
) -> Result<(), CodecError> {
    re_tracing::profile_function!();

    let schema = batch.schema_ref().as_ref();

    let mut sw = {
        let _span = tracing::trace_span!("schema").entered();
        arrow::ipc::writer::StreamWriter::try_new(writer, schema)
            .map_err(CodecError::ArrowSerialization)?
    };

    {
        let _span = tracing::trace_span!("data").entered();
        sw.write(batch).map_err(CodecError::ArrowSerialization)?;
    }

    sw.finish().map_err(CodecError::ArrowSerialization)?;

    Ok(())
}

/// Helper function that deserializes raw bytes into arrow schema and record batch
/// using Arrow IPC format.
///
/// Returns only the first record batch in the stream.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn read_arrow_from_bytes<R: std::io::Read>(
    reader: &mut R,
) -> Result<ArrowRecordBatch, CodecError> {
    re_tracing::profile_function!();

    let mut stream = {
        let _span = tracing::trace_span!("schema").entered();
        arrow::ipc::reader::StreamReader::try_new(reader, None)
            .map_err(CodecError::ArrowDeserialization)?
    };

    let _span = tracing::trace_span!("data").entered();
    stream
        .next()
        .ok_or(CodecError::MissingRecordBatch)?
        .map_err(CodecError::ArrowDeserialization)
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
            let _span = tracing::trace_span!("lz4::compress").entered();
            lz4_flex::block::compress(&uncompressed)
        }
    };

    Ok(Payload {
        uncompressed_size,
        data,
    })
}

// TODO(cmc): can we use the File-oriented APIs in order to re-use the transport buffer as backing
// storage for the final RecordBatch?
// See e.g. https://github.com/apache/arrow-rs/blob/b8b2f21f6a8254224d37a1e2d231b6b1e1767648/arrow/examples/zero_copy_ipc.rs
#[cfg(feature = "decoder")]
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn decode_arrow(
    data: &[u8],
    uncompressed_size: usize,
    compression: crate::Compression,
) -> Result<ArrowRecordBatch, crate::decoder::DecodeError> {
    if true {
        let mut uncompressed = Vec::new();
        let data = match compression {
            crate::Compression::Off => data,
            crate::Compression::LZ4 => {
                re_tracing::profile_scope!("LZ4-decompress");
                let _span = tracing::trace_span!("lz4::decompress").entered();
                uncompressed.resize(uncompressed_size, 0);
                lz4_flex::block::decompress_into(data, &mut uncompressed)?;
                uncompressed.as_slice()
            }
        };

        Ok(read_arrow_from_bytes(&mut &data[..])?)
    } else {
        // Zero-alloc path: not used today because allocations are bottlenecked on other things,
        // but I want to keep this around for later.

        use std::cell::RefCell;
        thread_local! {
            static BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
        }

        BUFFER.with_borrow_mut(|uncompressed| {
            let data = match compression {
                crate::Compression::Off => data,
                crate::Compression::LZ4 => {
                    let _span = tracing::trace_span!("lz4::decompress").entered();
                    uncompressed.resize(uncompressed_size, 0);
                    lz4_flex::block::decompress_into(data, uncompressed)?;
                    uncompressed.as_slice()
                }
            };

            Ok(read_arrow_from_bytes(&mut &data[..])?)
        })
    }
}
