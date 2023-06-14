use std::collections::VecDeque;
use std::io::Cursor;
use std::io::Read;

use re_log_types::LogMsg;

use crate::decoder::read_options;
use crate::decoder::STREAM_HEADER_SIZE;

use super::DecodeError;
use super::Decompressor;
use super::MESSAGE_HEADER_SIZE;

pub struct StreamDecoder {
    decompressor: Decompressor<Chunk>,
    buffer: ChunkBuffer,
    state: State,
}

#[allow(clippy::large_enum_variant)]
enum State {
    Header,
    MessageLength,
    MessageContent(u64),
}

impl StreamDecoder {
    pub fn new() -> Self {
        Self {
            decompressor: Decompressor::Uncompressed(Chunk::new(Vec::new())),
            buffer: ChunkBuffer::new(),
            state: State::Header,
        }
    }

    pub fn push_chunk(&mut self, chunk: Vec<u8>) {
        self.buffer.push(chunk);
    }

    pub fn try_read(&mut self) -> Result<Option<LogMsg>, DecodeError> {
        match self.state {
            State::Header => {
                if let Some(header) = self.buffer.try_read(STREAM_HEADER_SIZE)? {
                    // header contains version and compression options
                    let options = read_options(header)?;
                    self.decompressor =
                        Decompressor::new(options.compression, Chunk::new(Vec::new()));

                    // we might have data left in the current chunk,
                    // immediately try to read length of the next message
                    self.state = State::MessageLength;
                    return self.try_read();
                }
            }
            State::MessageLength => {
                if let Some(len) = self.buffer.try_read(MESSAGE_HEADER_SIZE)? {
                    self.state = State::MessageContent(u64_from_le_slice(len));
                    // we might have data left in the current chunk,
                    // immediately try to read the message content
                    return self.try_read();
                }
            }
            State::MessageContent(len) => {
                if self.buffer.try_read(len as usize)?.is_some() {
                    *self.decompressor.get_mut() =
                        Cursor::new(std::mem::take(&mut self.buffer.buffer));
                    let message = rmp_serde::from_read(&mut self.decompressor)
                        .map_err(DecodeError::MsgPack)?;
                    self.buffer.buffer = std::mem::take(self.decompressor.get_mut().get_mut());

                    self.state = State::MessageLength;
                    return Ok(Some(message));
                }
            }
        }

        Ok(None)
    }
}

fn u64_from_le_slice(bytes: &[u8]) -> u64 {
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

impl Default for StreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

type Chunk = Cursor<Vec<u8>>;

struct ChunkBuffer {
    queue: VecDeque<Chunk>,
    buffer: Vec<u8>,
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

    fn try_read(&mut self, n: usize) -> Result<Option<&[u8]>, DecodeError> {
        if self.buffer.len() != n {
            self.buffer.resize(n, 0);
            self.cursor = 0;
        }

        while self.cursor != n {
            if let Some(chunk) = self.queue.front_mut() {
                self.cursor += chunk
                    .read(&mut self.buffer[self.cursor..])
                    .map_err(DecodeError::Read)?;
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

    fn test_data(n: usize) -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut encoder = Encoder::new(EncodingOptions::UNCOMPRESSED, &mut buffer).unwrap();
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
                Err(e) => {
                    panic!("failed to read message: {e}");
                }
            }
        }};
    }

    #[test]
    fn stream_whole_chunks() {
        let data = test_data(16);

        let mut decoder = StreamDecoder::new();
        decoder.push_chunk(data);

        for _ in 0..16 {
            assert_message_ok!(decoder.try_read());
        }
    }

    #[test]
    fn stream_byte_chunks() {
        let data = test_data(16);

        let mut decoder = StreamDecoder::new();
        for chunk in data.chunks(1) {
            decoder.push_chunk(chunk.to_vec());
        }

        for _ in 0..16 {
            assert_message_ok!(decoder.try_read());
        }
    }
}
