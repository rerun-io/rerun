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
    data: &'a [u8],
    capacity: usize,
}

#[cfg(feature = "encoder")]
impl Payload<'_> {
    /// This version does not copy the data, but instead produces the `Vec`
    /// from raw parts.
    ///
    /// The safe version of this is [`Payload::to_vec`].
    ///
    /// # Safety
    ///
    /// The returned `Vec` must NOT be dropped! Pass it to `mem::forget` before that happens.
    #[allow(unsafe_code)]
    pub(crate) unsafe fn to_fake_temp_vec(&self) -> Vec<u8> {
        // SAFETY: User is required to uphold safety invariants.
        unsafe {
            Vec::from_raw_parts(
                self.data.as_ptr().cast_mut(),
                self.data.len(),
                self.capacity,
            )
        }
    }

    /// Copy the data to a `Vec`.
    pub(crate) fn to_vec(&self) -> Vec<u8> {
        self.data.to_vec()
    }
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

// NOTE: Externally, `Payload`'s borrow of `'a` is treated as if it was `&'a mut`.
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

    let (capacity, data) = match compression {
        crate::Compression::Off => (uncompressed.capacity(), &uncompressed[..]),
        crate::Compression::LZ4 => {
            let max_len = lz4_flex::block::get_maximum_output_size(uncompressed.len());
            compressed.resize(max_len, 0);
            let written_bytes = lz4_flex::block::compress_into(uncompressed, compressed)?;
            (compressed.capacity(), &compressed[..written_bytes])
        }
    };

    Ok(Payload {
        uncompressed_size,
        data,
        capacity,
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
