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
    let mut sw = arrow::ipc::writer::StreamWriter::try_new(writer, schema)
        .map_err(CodecError::ArrowSerialization)?;
    sw.write(batch).map_err(CodecError::ArrowSerialization)?;
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

    let mut stream = arrow::ipc::reader::StreamReader::try_new(reader, None)
        .map_err(CodecError::ArrowDeserialization)?;

    stream
        .next()
        .ok_or(CodecError::MissingRecordBatch)?
        .map_err(CodecError::ArrowDeserialization)
}

/// Helper function that deserializes raw bytes into arrow schema and record batch
/// using Arrow IPC format.
///
/// The original buffer will be used as the backing storage for the decoded Arrow data.
///
/// Returns only the first record batch in the stream.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn load_arrow_from_bytes(data: bytes::Bytes) -> Result<ArrowRecordBatch, CodecError> {
    re_tracing::profile_function!();

    let buffer = Buffer::from(data);
    let decoder = IPCBufferDecoder::new(buffer);
    assert_eq!(decoder.num_batches(), 1); // TODO

    Ok(decoder.get_batch(0).unwrap())
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

#[cfg(feature = "decoder")]
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn decode_arrow(
    data: &[u8],
    uncompressed_size: usize,
    compression: crate::Compression,
) -> Result<ArrowRecordBatch, crate::decoder::DecodeError> {
    let mut uncompressed = bytes::BytesMut::new();
    let data: bytes::Bytes = match compression {
        crate::Compression::Off => data.to_vec().into(),
        crate::Compression::LZ4 => {
            re_tracing::profile_scope!("LZ4-decompress");
            let _span = tracing::trace_span!("lz4::decompress").entered();
            uncompressed.resize(uncompressed_size, 0);
            lz4_flex::block::decompress_into(data, &mut uncompressed)?;
            uncompressed.freeze()
        }
    };

    Ok(load_arrow_from_bytes(data)?)
}

// ---

// This code is taken from [1].
//
// Licensed to the Apache Software Foundation (ASF) under one or more contributor license agreements.
// The ASF licenses this file to you under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of the License at
// <http://www.apache.org/licenses/LICENSE-2.0>.
//
// [1]: https://github.com/apache/arrow-rs/blob/b8b2f21f6a8254224d37a1e2d231b6b1e1767648/arrow/examples/zero_copy_ipc.rs

use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::buffer::Buffer;
use arrow::datatypes::Schema;
use arrow::error::ArrowError;
use arrow::ipc::convert::fb_to_schema;
use arrow::ipc::reader::{FileDecoder, read_footer_length};
use arrow::ipc::{Block, root_as_footer};

/// Incrementally decodes [`RecordBatch`]es from an IPC file stored in a Arrow [`Buffer`] using the
/// [`FileDecoder`] API.
///
/// This is a wrapper around the example in the `FileDecoder` which handles the low level
/// interaction with the Arrow IPC format.
struct IPCBufferDecoder {
    /// Memory (or memory mapped) Buffer with the data.
    buffer: Buffer,

    /// Decoder that reads Arrays that refers to the underlying buffers.
    decoder: FileDecoder,

    /// Location of the batches within the buffer.
    batches: Vec<Block>,
}

impl IPCBufferDecoder {
    fn new(buffer: Buffer) -> Self {
        let trailer_start = buffer.len() - 10;
        let footer_len = read_footer_length(buffer[trailer_start..].try_into().unwrap()).unwrap();
        let footer = root_as_footer(&buffer[trailer_start - footer_len..trailer_start]).unwrap();

        let schema = fb_to_schema(footer.schema().unwrap());

        let mut decoder = FileDecoder::new(Arc::new(schema), footer.version());

        // Read dictionaries
        for block in footer.dictionaries().iter().flatten() {
            let block_len = block.bodyLength() as usize + block.metaDataLength() as usize;
            let data = buffer.slice_with_length(block.offset() as _, block_len);
            decoder.read_dictionary(block, &data).unwrap();
        }

        // convert to Vec from the flatbuffers Vector to avoid having a direct dependency on flatbuffers
        let batches = footer
            .recordBatches()
            .map(|b| b.iter().copied().collect())
            .unwrap_or_default();

        Self {
            buffer,
            decoder,
            batches,
        }
    }

    /// Return the number of [`RecordBatch`]es in this buffer
    fn num_batches(&self) -> usize {
        self.batches.len()
    }

    /// Return the [`RecordBatch`] at message index `i`.
    ///
    /// This may return `None` if the IPC message was None
    fn get_batch(&self, i: usize) -> Option<RecordBatch> {
        let block = &self.batches[i];
        let block_len = block.bodyLength() as usize + block.metaDataLength() as usize;
        let data = self
            .buffer
            .slice_with_length(block.offset() as _, block_len);
        self.decoder.read_record_batch(block, &data).unwrap()
    }
}

const ARROW_MAGIC: [u8; 6] = [b'A', b'R', b'R', b'O', b'W', b'1'];
const CONTINUATION_MARKER: [u8; 4] = [0xff; 4];

/// Try deserialize the IPC format bytes into a schema
pub fn try_schema_from_ipc_buffer(buffer: &[u8]) -> Result<Schema, ArrowError> {
    // There are two protocol types: https://issues.apache.org/jira/browse/ARROW-6313
    // The original protocol is:
    //   4 bytes - the byte length of the payload
    //   a flatbuffer Message whose header is the Schema
    // The latest version of protocol is:
    // The schema of the dataset in its IPC form:
    //   4 bytes - an optional IPC_CONTINUATION_TOKEN prefix
    //   4 bytes - the byte length of the payload
    //   a flatbuffer Message whose header is the Schema

    if buffer.len() < 4 {
        return Err(ArrowError::ParseError(
            "The buffer length is less than 4 and missing the continuation marker or length of buffer".to_string()
        ));
    }

    let (len, buffer) = if buffer[..4] == CONTINUATION_MARKER {
        if buffer.len() < 8 {
            return Err(ArrowError::ParseError(
                "The buffer length is less than 8 and missing the length of buffer".to_string(),
            ));
        }

        buffer[4..].split_at(4)
    } else {
        buffer.split_at(4)
    };

    let len = <i32>::from_le_bytes(len.try_into().unwrap());

    if len < 0 {
        return Err(ArrowError::ParseError(format!(
            "The encapsulated message's reported length is negative ({len})"
        )));
    }

    if buffer.len() < len as usize {
        let actual_len = buffer.len();

        return Err(ArrowError::ParseError(format!(
            "The buffer length ({actual_len}) is less than the encapsulated message's reported length ({len})"
        )));
    }

    let msg = arrow::ipc::root_as_message(buffer)
        .map_err(|err| ArrowError::ParseError(format!("Unable to get root as message: {err:?}")))?;

    let ipc_schema = msg.header_as_schema().ok_or_else(|| {
        ArrowError::ParseError("Unable to convert flight info to a schema".to_string())
    })?;

    Ok(fb_to_schema(ipc_schema))
}
