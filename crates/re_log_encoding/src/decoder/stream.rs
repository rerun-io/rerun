use std::collections::VecDeque;
use std::io;
use std::io::Cursor;
use std::io::Read;

use re_log_types::LogMsg;

use crate::decoder::read_options;
use crate::decoder::STREAM_HEADER_SIZE;

use super::DecodeError;
use super::Decompressor;
use super::MESSAGE_HEADER_SIZE;

pub struct StreamDecoder {
    state: Option<StreamState>,
}

#[allow(clippy::large_enum_variant)]
enum StreamState {
    Header(Header),
    Message(Message),
}

struct Header {
    chunks: ChunkBuffer,
    buffer: [u8; STREAM_HEADER_SIZE],
    cursor: usize,
}

struct Message {
    decompressor: Decompressor<ChunkBuffer>,
    buffer: Vec<u8>,
    cursor: usize,
    state: DataState,
}

enum DataState {
    Header { buffer: [u8; 8], cursor: usize },
    Content,
}

impl DataState {
    fn empty_header() -> Self {
        Self::Header {
            buffer: [0_u8; 8],
            cursor: 0,
        }
    }
}

impl StreamDecoder {
    pub fn new() -> Self {
        Self {
            state: Some(StreamState::Header(Header {
                chunks: ChunkBuffer::new(),
                buffer: [0_u8; STREAM_HEADER_SIZE],
                cursor: 0,
            })),
        }
    }

    pub fn push_chunk(&mut self, chunk: Vec<u8>) {
        match self.state.as_mut().take().unwrap() {
            StreamState::Header(inner) => inner.chunks.push(chunk),
            StreamState::Message(inner) => inner.decompressor.get_mut().push(chunk),
        }
    }

    pub fn try_read(&mut self) -> Result<Option<LogMsg>, DecodeError> {
        match self.state.take().unwrap() {
            StreamState::Header(mut inner) => {
                println!("header");
                // we need at least 12 bytes to initialize the reader
                inner.cursor += inner
                    .chunks
                    .read(&mut inner.buffer[inner.cursor..])
                    .map_err(DecodeError::Read)?;
                if STREAM_HEADER_SIZE - inner.cursor == 0 {
                    // we have enough data to initialize the decoder
                    let options = read_options(&inner.buffer)?;

                    self.state = Some(StreamState::Message(Message {
                        decompressor: Decompressor::new(options.compression, inner.chunks),
                        cursor: 0,
                        buffer: Vec::with_capacity(1024),
                        state: DataState::empty_header(),
                    }));

                    // immediately try to read a message
                    self.try_read()
                } else {
                    // not done yet
                    self.state = Some(StreamState::Header(inner));
                    Ok(None)
                }
            }
            StreamState::Message(mut inner) => match &mut inner.state {
                DataState::Header { buffer, cursor } => {
                    println!("data header {cursor}");
                    *cursor += inner
                        .decompressor
                        .get_mut()
                        .read(&mut buffer[*cursor..])
                        .map_err(DecodeError::Read)?;
                    if MESSAGE_HEADER_SIZE - *cursor == 0 {
                        // we know how large the incoming message is
                        let len = u64::from_le_bytes(*buffer) as usize;
                        println!("{len}"); // <- incorrect
                        inner.buffer.resize(len, 0);
                        self.state = Some(StreamState::Message(Message {
                            decompressor: inner.decompressor,
                            buffer: inner.buffer,
                            cursor: 0,
                            state: DataState::Content,
                        }));

                        // immediately try to read a message
                        self.try_read()
                    } else {
                        // not done yet
                        self.state = Some(StreamState::Message(inner));
                        Ok(None)
                    }
                }
                DataState::Content => {
                    println!("data content");
                    let expected_message_size = inner.buffer.len();
                    println!("expected size {expected_message_size}");
                    println!("before cursor{{{}}}", inner.cursor);
                    inner.cursor += inner
                        .decompressor
                        .read(&mut inner.buffer[inner.cursor..])
                        .map_err(DecodeError::Read)?;
                    println!("after before{{{}}}", inner.cursor);
                    if expected_message_size - inner.cursor == 0 {
                        println!("can read message");
                        // we can read a full message
                        let message = rmp_serde::decode::from_read(&inner.buffer[..])?;
                        self.state = Some(StreamState::Message(Message {
                            decompressor: inner.decompressor,
                            cursor: 0,
                            buffer: inner.buffer,
                            state: DataState::empty_header(),
                        }));

                        Ok(Some(message))
                    } else {
                        println!("not enough data yet");
                        // not done yet
                        self.state = Some(StreamState::Message(inner));
                        Ok(None)
                    }
                }
            },
        }
    }
}

impl Default for StreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

type Chunk = Cursor<Vec<u8>>;

struct ChunkBuffer {
    queue: VecDeque<Chunk>,
}

impl ChunkBuffer {
    fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(16),
        }
    }

    fn push(&mut self, chunk: Vec<u8>) {
        self.queue.push_back(Chunk::new(chunk));
    }
}

fn is_chunk_empty(chunk: &Chunk) -> bool {
    chunk.position() >= chunk.get_ref().len() as u64
}

impl Read for ChunkBuffer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut cursor = 0;
        while cursor != buf.len() {
            println!(
                "before buf needs{{{}/{}}}",
                buf.len() - buf[cursor..].len(),
                buf.len()
            );
            if let Some(chunk) = self.queue.front_mut() {
                println!(
                    "before chunk remaining{{{}/{}}}",
                    chunk.get_ref().len() as u64 - chunk.position(),
                    chunk.get_ref().len(),
                );
                cursor += chunk.read(&mut buf[cursor..])?;
                println!(
                    "after chunk remaining{{{}/{}}}",
                    chunk.get_ref().len() as u64 - chunk.position(),
                    chunk.get_ref().len(),
                );
                // pop the chunk if it is now empty
                if is_chunk_empty(chunk) {
                    println!("chunk now empty");
                    self.queue.pop_front();
                }
            } else {
                println!("no chunk, break");
                break;
            }
            println!(
                "after buf needs{{{}/{}}}",
                buf.len() - buf[cursor..].len(),
                buf.len()
            );
        }
        Ok(cursor)
    }
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

    fn to_debug_string(message: &LogMsg) -> String {
        format!("{message:?}")
    }

    fn test_data(n: usize) -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut encoder = Encoder::new(EncodingOptions::UNCOMPRESSED, &mut buffer).unwrap();
            for _ in 0..n {
                encoder.append(&fake_log_msg()).unwrap();
            }
            encoder.finish().unwrap();
        }
        buffer
    }

    macro_rules! assert_message_ok {
        ($message:expr) => {{
            match $message {
                Ok(Some(message)) => {
                    assert_eq!(to_debug_string(&fake_log_msg()), to_debug_string(&message),)
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
