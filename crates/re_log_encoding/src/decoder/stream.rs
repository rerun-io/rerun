use std::collections::VecDeque;
use std::io::Cursor;
use std::io::Read;

use re_log_types::LogMsg;

use crate::decoder::read_options;
use crate::Compression;
use crate::FileHeader;
use crate::MessageHeader;

use super::DecodeError;

/// The stream decoder is a state machine which ingests byte chunks
/// and outputs messages once it has enough data to deserialize one.
///
/// Chunks are given to the stream via `StreamDecoder::push_chunk`,
/// and messages are read back via `StreamDecoder::try_read`.
pub struct StreamDecoder {
    /// Compression options
    compression: Compression,

    /// Incoming chunks are stored here
    chunks: ChunkBuffer,

    /// The uncompressed bytes are stored in this buffer before being read by `rmp_serde`
    uncompressed: Vec<u8>,

    /// The stream state
    state: State,
}

///
/// ```text,ignore
/// StreamHeader
///      |
///      v
/// MessageHeader
/// ^           |
/// |           |
/// ---Message<--
/// ```
#[derive(Clone, Copy)]
enum State {
    /// The beginning of the stream.
    ///
    /// The stream header contains the magic bytes (e.g. `RRF2`),
    /// the encoded version, and the encoding options.
    ///
    /// After the stream header is read once, the state machine
    /// will only ever switch between `MessageHeader` and `Message`
    StreamHeader,
    /// The beginning of a message.
    ///
    /// The message header contains the number of bytes in the
    /// compressed message, and the number of bytes in the
    /// uncompressed message.
    MessageHeader,
    /// The message content.
    ///
    /// We need to know the full length of the message before attempting
    /// to read it, otherwise the call to `decompress_into` or the
    /// MessagePack deserialization may block or even fail.
    Message(MessageHeader),
}

impl StreamDecoder {
    pub fn new() -> Self {
        Self {
            compression: Compression::Off,
            chunks: ChunkBuffer::new(),
            uncompressed: Vec::with_capacity(1024),
            state: State::StreamHeader,
        }
    }

    pub fn push_chunk(&mut self, chunk: Vec<u8>) {
        self.chunks.push(chunk);
    }

    pub fn try_read(&mut self) -> Result<Option<LogMsg>, DecodeError> {
        match self.state {
            State::StreamHeader => {
                if let Some(header) = self.chunks.try_read(FileHeader::SIZE)? {
                    // header contains version and compression options
                    self.compression = read_options(header)?.compression;

                    // we might have data left in the current chunk,
                    // immediately try to read length of the next message
                    self.state = State::MessageHeader;
                    return self.try_read();
                }
            }
            State::MessageHeader => {
                if let Some(mut len) = self.chunks.try_read(MessageHeader::SIZE)? {
                    let header = MessageHeader::decode(&mut len)?;
                    self.state = State::Message(header);
                    // we might have data left in the current chunk,
                    // immediately try to read the message content
                    return self.try_read();
                }
            }
            State::Message(header) => {
                if let Some(bytes) = self.chunks.try_read(header.compressed_len as usize)? {
                    let bytes = match self.compression {
                        Compression::Off => bytes,
                        Compression::LZ4 => {
                            self.uncompressed
                                .resize(header.uncompressed_len as usize, 0);
                            lz4_flex::block::decompress_into(bytes, &mut self.uncompressed)
                                .map_err(DecodeError::Lz4)?;
                            &self.uncompressed
                        }
                    };

                    // read the message from the uncompressed bytes
                    let message = rmp_serde::from_slice(bytes).map_err(DecodeError::MsgPack)?;

                    self.state = State::MessageHeader;
                    return Ok(Some(message));
                }
            }
        }

        Ok(None)
    }
}

impl Default for StreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

type Chunk = Cursor<Vec<u8>>;

struct ChunkBuffer {
    /// Any incoming chunks are queued until they are emptied
    queue: VecDeque<Chunk>,

    /// When `try_read` is called and we don't have enough bytes yet,
    /// we store whatever we do have in this buffer
    buffer: Vec<u8>,

    /// The cursor points to the end of the used range in `buffer`
    cursor: usize,
}

impl ChunkBuffer {
    fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(16),
            buffer: Vec::with_capacity(1024),
            cursor: 0,
        }
    }

    fn push(&mut self, chunk: Vec<u8>) {
        self.queue.push_back(Chunk::new(chunk));
    }

    /// Attempt to read exactly `n` bytes out of the queued chunks.
    ///
    /// Returns `Ok(None)` if there is not enough data to return a slice of `n` bytes.
    fn try_read(&mut self, n: usize) -> Result<Option<&[u8]>, DecodeError> {
        // resize the buffer if the target has changed
        if self.buffer.len() != n {
            self.buffer.resize(n, 0);
            self.cursor = 0;
        }

        // try to read some bytes from the front of the queue,
        // until either:
        // - we've read enough to return a slice of `n` bytes
        // - we run out of chunks to read
        // while also discarding any empty chunks
        while self.cursor != n {
            if let Some(chunk) = self.queue.front_mut() {
                let remainder = &mut self.buffer[self.cursor..];
                self.cursor += chunk.read(remainder).map_err(DecodeError::Read)?;
                if is_chunk_empty(chunk) {
                    self.queue.pop_front();
                }
            } else {
                break;
            }
        }

        if self.cursor == n {
            Ok(Some(&self.buffer[..]))
        } else {
            Ok(None)
        }
    }
}

fn is_chunk_empty(chunk: &Chunk) -> bool {
    chunk.position() >= chunk.get_ref().len() as u64
}

#[cfg(test)]
mod tests {
    use re_log_types::ApplicationId;
    use re_log_types::RowId;
    use re_log_types::SetStoreInfo;
    use re_log_types::StoreId;
    use re_log_types::StoreInfo;
    use re_log_types::StoreKind;
    use re_log_types::StoreSource;
    use re_log_types::Time;

    use crate::encoder::Encoder;
    use crate::EncodingOptions;

    use super::*;

    fn fake_log_msg() -> LogMsg {
        LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: RowId::ZERO,
            info: StoreInfo {
                application_id: ApplicationId::unknown(),
                store_id: StoreId::from_string(StoreKind::Recording, "test".into()),
                is_official_example: false,
                started: Time::from_ns_since_epoch(0),
                store_source: StoreSource::Unknown,
                store_kind: StoreKind::Recording,
            },
        })
    }

    fn test_data(options: EncodingOptions, n: usize) -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut encoder = Encoder::new(options, &mut buffer).unwrap();
            for _ in 0..n {
                encoder.append(&fake_log_msg()).unwrap();
            }
        }
        buffer
    }

    macro_rules! assert_message_ok {
        ($message:expr) => {{
            match $message {
                Ok(Some(message)) => {
                    assert_eq!(&fake_log_msg(), &message)
                }
                Ok(None) => {
                    panic!("failed to read message: message could not be read in full");
                }
                Err(err) => {
                    panic!("failed to read message: {err}");
                }
            }
        }};
    }

    #[test]
    fn stream_whole_chunks_uncompressed() {
        let data = test_data(EncodingOptions::UNCOMPRESSED, 16);

        let mut decoder = StreamDecoder::new();
        decoder.push_chunk(data);

        for _ in 0..16 {
            assert_message_ok!(decoder.try_read());
        }
    }

    #[test]
    fn stream_byte_chunks_uncompressed() {
        let data = test_data(EncodingOptions::UNCOMPRESSED, 16);

        let mut decoder = StreamDecoder::new();
        for chunk in data.chunks(1) {
            decoder.push_chunk(chunk.to_vec());
        }

        for _ in 0..16 {
            assert_message_ok!(decoder.try_read());
        }
    }

    #[test]
    fn stream_whole_chunks_compressed() {
        let data = test_data(EncodingOptions::COMPRESSED, 16);

        let mut decoder = StreamDecoder::new();
        decoder.push_chunk(data);

        for _ in 0..16 {
            assert_message_ok!(decoder.try_read());
        }
    }

    #[test]
    fn stream_byte_chunks_compressed() {
        let data = test_data(EncodingOptions::COMPRESSED, 16);

        let mut decoder = StreamDecoder::new();
        for chunk in data.chunks(1) {
            decoder.push_chunk(chunk.to_vec());
        }

        for _ in 0..16 {
            assert_message_ok!(decoder.try_read());
        }
    }
}
