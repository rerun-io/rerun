use std::{
    collections::VecDeque,
    io::{Cursor, Read as _},
};

use re_build_info::CrateVersion;

use crate::{
    CachingApplicationIdInjector,
    rrd::{
        CodecError, Decodable as _, DecodeError, DecoderEntrypoint, EncodingOptions, Serializer,
        StreamHeader,
    },
};

// ---

/// A type alias for a [`Decoder`] that only decodes from raw bytes up to transport-level
/// types (i.e. Protobuf payloads are decoded, but Arrow data is never touched).
///
/// See also [`DecoderTransport`].
pub type DecoderTransport = Decoder<re_protos::log_msg::v1alpha1::log_msg::Msg>;

/// A type alias for a [`Decoder`] that decodes all the way from raw bytes to
/// application-level types (i.e. even Arrow layers are decoded).
///
/// See also [`DecoderApp`].
pub type DecoderApp = Decoder<re_log_types::LogMsg>;

/// A push-based state machine that ingests byte chunks and outputs messages once it has enough
/// data to decode one.
///
/// Byte chunks are given to the stream via [`DecoderApp::push_byte_chunk`], and messages are read
/// back via [`DecoderApp::try_read`].
///
/// The unorthodox push-based model is what allows us to run this in all the weird environments
/// that Rerun support (web, async, HTTP fetches, etc):
/// * [`DecoderIterator`] implements a poll-based synchronous iterator on top of it.
/// * [`DecoderStream`] implements a poll-based asynchronous stream on top of it.
///
/// [`DecoderIterator`]: [`crate::DecoderIterator`]
/// [`DecoderStream`]: [`crate::DecoderStream`]
pub struct Decoder<T> {
    /// The Rerun version used to encode the RRD data.
    ///
    /// `None` until a Rerun header has been processed.
    pub(crate) version: Option<CrateVersion>,

    pub(crate) options: EncodingOptions,

    /// Incoming byte chunks are stored here.
    pub(crate) byte_chunks: ByteChunkBuffer,

    /// The stream state.
    pub(crate) state: DecoderState,

    /// The application id cache used for migrating old data.
    pub(crate) app_id_cache: CachingApplicationIdInjector,

    pub(crate) _decodable: std::marker::PhantomData<T>,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderState {
    /// The beginning of the stream.
    ///
    /// The stream header contains the magic bytes (e.g. `RRF2`), the encoded version, and the
    /// encoding options.
    ///
    /// After the stream header is read once, the state machine will only ever switch between
    /// `MessageHeader` and `Message`
    StreamHeader,

    /// The beginning of a Protobuf message.
    MessageHeader,

    /// The message content, serialized using `Protobuf`.
    ///
    /// Compression is only applied to individual `ArrowMsg`s, instead of the entire stream.
    Message(crate::rrd::MessageHeader),

    /// Stop reading.
    Aborted,
}

impl<T: DecoderEntrypoint> Decoder<T> {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            version: None,
            // Note: `options` are filled in once we read `FileHeader`, so this value does not matter.
            options: EncodingOptions::PROTOBUF_UNCOMPRESSED,
            byte_chunks: ByteChunkBuffer::new(),
            state: DecoderState::StreamHeader,
            app_id_cache: CachingApplicationIdInjector::default(),
            _decodable: std::marker::PhantomData::<T>,
        }
    }

    /// Feed a bunch of bytes to the decoding state machine.
    pub fn push_byte_chunk(&mut self, byte_chunk: Vec<u8>) {
        self.byte_chunks.push(byte_chunk);
    }

    /// Read the next message in the stream, dropping messages missing application id that cannot
    /// be migrated (because they arrived before `SetStoreInfo`).
    pub fn try_read(&mut self) -> Result<Option<T>, DecodeError> {
        //TODO(#10730): remove this if/when we remove the legacy `StoreId` migration.
        loop {
            let result = self.try_read_impl();
            if let Err(DecodeError::Codec(CodecError::StoreIdMissingApplicationId {
                store_kind,
                recording_id,
            })) = result
            {
                re_log::warn_once!(
                    "Dropping message without application id which arrived before `SetStoreInfo` \
                    (kind: {store_kind}, recording id: {recording_id}."
                );
            } else {
                return result;
            }
        }
    }

    /// Read the next message in the stream.
    fn try_read_impl(&mut self) -> Result<Option<T>, DecodeError> {
        match self.state {
            DecoderState::StreamHeader => {
                let is_first_header = self.byte_chunks.num_read() == 0;
                let position = self.byte_chunks.num_read();
                if let Some(header_data) =
                    self.byte_chunks.try_read(StreamHeader::ENCODED_SIZE_BYTES)
                {
                    re_log::trace!(?header_data, "Decoding StreamHeader");

                    // header contains version and compression options
                    let version_and_options = StreamHeader::from_rrd_bytes(&header_data)
                        .and_then(|h| h.to_version_and_options());
                    let (version, options) = match version_and_options {
                        Ok(ok) => ok,
                        Err(err) => {
                            // We expected a header, but didn't find one!
                            if is_first_header {
                                return Err(err.into());
                            } else {
                                re_log::error!(
                                    is_first_header,
                                    position,
                                    "Trailing bytes in rrd stream: {header_data:?} ({err})"
                                );
                                self.state = DecoderState::Aborted;
                                return Ok(None);
                            }
                        }
                    };

                    re_log::trace!(
                        version = version.to_string(),
                        ?options,
                        "Found Stream Header"
                    );

                    self.version = Some(version);
                    self.options = options;

                    match self.options.serializer {
                        Serializer::Protobuf => self.state = DecoderState::MessageHeader,
                    }

                    // we might have data left in the current byte chunk, immediately try to read
                    // length of the next message.
                    return self.try_read();
                }
            }

            DecoderState::MessageHeader => {
                let mut peeked = [0u8; crate::rrd::MessageHeader::ENCODED_SIZE_BYTES];
                if self.byte_chunks.try_peek(&mut peeked) == peeked.len() {
                    let header = match crate::rrd::MessageHeader::from_rrd_bytes(&peeked) {
                        Ok(header) => header,

                        Err(crate::rrd::CodecError::HeaderDecoding(_)) => {
                            // We failed to decode a `MessageHeader`: it might be because the
                            // stream is corrupt, or it might be because it just switched to a
                            // different, concatenated recording without having the courtesy of
                            // announcing it via an EOS marker.
                            self.state = DecoderState::StreamHeader;
                            return self.try_read();
                        }

                        err @ Err(_) => err?,
                    };

                    self.byte_chunks
                        .try_read(crate::rrd::MessageHeader::ENCODED_SIZE_BYTES)
                        .expect("reading cannot fail if peeking worked");

                    re_log::trace!(?header, "MessageHeader");

                    self.state = DecoderState::Message(header);
                    // we might have data left in the current byte chunk, immediately try to read
                    // the message content.
                    return self.try_read();
                }
            }

            DecoderState::Message(header) => {
                let start_offset = self.byte_chunks.num_read() as u64;

                if let Some(bytes) = self.byte_chunks.try_read(header.len as usize) {
                    re_log::trace!(?header, "Read message");

                    let bytes_len = bytes.len() as u64;
                    let byte_span = re_chunk::Span {
                        start: start_offset,
                        len: bytes_len,
                    };
                    let message = match T::decode(
                        bytes,
                        byte_span,
                        header.kind,
                        &mut self.app_id_cache,
                        self.version,
                    ) {
                        Ok(msg) => msg,
                        Err(err) => {
                            // We successfully parsed a header, but decided to drop the message altogether.
                            // We must go back to looking for headers, or the decoder will just be stuck in a dead
                            // state forever.
                            self.state = DecoderState::MessageHeader;
                            return Err(err.into());
                        }
                    };

                    if let Some(message) = message {
                        re_log::trace!("Decoded new message");

                        self.state = DecoderState::MessageHeader;
                        return Ok(Some(message));
                    } else {
                        re_log::trace!("End of stream - expecting a new Streamheader");

                        // `None` means an end-of-stream marker was hit, but there might be another concatenated
                        // stream behind, so try to start all over again.
                        self.state = DecoderState::StreamHeader;
                        return self.try_read();
                    }
                }
            }

            DecoderState::Aborted => {
                return Ok(None);
            }
        }

        Ok(None)
    }
}

// ---

/// A bunch of contiguous bytes.
type ByteChunk = Cursor<Vec<u8>>;

pub struct ByteChunkBuffer {
    /// Any incoming byte chunks are queued until they are emptied.
    queue: VecDeque<ByteChunk>,

    /// This buffer is used as scratch space for any read bytes, so that we can return a contiguous
    /// slice from `try_read`.
    buffer: Vec<u8>,

    /// How many bytes of valid data are currently in `self.buffer`.
    buffer_fill: usize,

    /// How many bytes have been read with [`Self::try_read`] so far?
    num_read: usize,
}

impl ByteChunkBuffer {
    fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(16),
            buffer: Vec::with_capacity(1024),
            buffer_fill: 0,
            num_read: 0,
        }
    }

    fn push(&mut self, byte_chunk: Vec<u8>) {
        if byte_chunk.is_empty() {
            return;
        }
        self.queue.push_back(ByteChunk::new(byte_chunk));
    }

    /// How many bytes have been read with [`Self::try_read`] so far?
    pub fn num_read(&self) -> usize {
        self.num_read
    }

    /// Attempt to read exactly `n` bytes out of the queued byte chunks.
    ///
    /// Returns `None` if there is not enough data to return a slice of `n` bytes.
    ///
    /// NOTE: `try_read` *must* be called with the same `n` until it returns `Some`,
    /// otherwise this will discard any previously buffered data.
    fn try_read(&mut self, n: usize) -> Option<bytes::Bytes> {
        // resize the buffer if the target has changed
        if self.buffer.len() != n {
            assert_eq!(
                self.buffer_fill, 0,
                "`try_read` called with different `n` for incomplete read"
            );
            self.buffer.resize(n, 0);
            self.buffer_fill = 0;
        }

        // try to read some bytes from the front of the queue,
        // until either:
        // - we've read enough to return a slice of `n` bytes
        // - we run out of byte chunks to read
        // while also discarding any empty byte chunks
        while self.buffer_fill != n {
            if let Some(byte_chunk) = self.queue.front_mut() {
                let remainder = &mut self.buffer[self.buffer_fill..];
                self.buffer_fill += byte_chunk
                    .read(remainder)
                    .expect("failed to read from byte chunk");
                if is_byte_chunk_empty(byte_chunk) {
                    self.queue.pop_front();
                }
            } else {
                break;
            }
        }

        if self.buffer_fill == n {
            // ensure that a successful call to `try_read(N)`
            // followed by another call to `try_read(N)` with the same `N`
            // won't erroneously return the same bytes
            self.buffer_fill = 0;
            self.num_read += n;
            Some(std::mem::take(&mut self.buffer).into())
        } else {
            None
        }
    }

    /// Attempt to peek exactly `n` bytes from of the queued byte chunks.
    ///
    /// Returns the number of bytes that could successfully be peeked, and therefore copied into `out`.
    /// The returned value is guaranteed to never exceed `out.len`.
    fn try_peek(&self, out: &mut [u8]) -> usize {
        use std::io::Write as _;

        let target_len = out.len();

        let mut out = std::io::Cursor::new(out);
        let mut n = 0;

        // `try_read` will never read from the active buffer if `n` changes, so we must emulate the
        // same behavior.
        if target_len == self.buffer.len() {
            n += out
                .write(&self.buffer[..self.buffer_fill])
                .expect("memcpy, cannot fail");
        }

        for byte_chunk in &self.queue {
            if n == target_len {
                return n;
            }

            let pos = byte_chunk.position() as usize;
            n += out
                .write(&byte_chunk.get_ref()[pos..])
                .expect("memcpy, cannot fail");
        }

        n
    }
}

fn is_byte_chunk_empty(byte_chunk: &ByteChunk) -> bool {
    byte_chunk.position() >= byte_chunk.get_ref().len() as u64
}

// ---

#[cfg(test)]
mod tests {
    use re_chunk::RowId;
    use re_log_types::{LogMsg, SetStoreInfo, StoreInfo};

    use super::*;
    use crate::{Encoder, rrd::EncodingOptions};

    fn fake_log_msg() -> LogMsg {
        LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::ZERO,
            info: StoreInfo {
                store_version: Some(CrateVersion::LOCAL), // Encoder sets the crate version
                ..StoreInfo::testing()
            },
        })
    }

    fn test_data(options: EncodingOptions, n: usize) -> (Vec<LogMsg>, Vec<u8>) {
        let messages: Vec<_> = (0..n).map(|_| fake_log_msg()).collect();

        let mut data = Vec::new();
        Encoder::encode_into(
            CrateVersion::LOCAL,
            options,
            messages.clone().into_iter().map(Ok),
            &mut data,
        )
        .unwrap();

        (messages, data)
    }

    macro_rules! assert_message_ok {
        ($message:expr) => {{
            match $message {
                Ok(Some(message)) => {
                    assert_eq!(&fake_log_msg(), &message);
                    message
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

    macro_rules! assert_message_incomplete {
        ($message:expr) => {{
            match $message {
                Ok(None) => {}
                Ok(Some(message)) => {
                    panic!("expected message to be incomplete, instead received: {message:?}");
                }
                Err(err) => {
                    panic!("failed to read message: {err}");
                }
            }
        }};
    }

    #[test]
    fn stream_whole_chunks_uncompressed_protobuf() {
        let (input, data) = test_data(EncodingOptions::PROTOBUF_UNCOMPRESSED, 16);

        let mut decoder = DecoderApp::new();

        assert_message_incomplete!(decoder.try_read());

        decoder.push_byte_chunk(data);

        let decoded_messages: Vec<_> = (0..16)
            .map(|_| assert_message_ok!(decoder.try_read()))
            .collect();

        assert_eq!(input, decoded_messages);
    }

    #[test]
    fn stream_byte_chunks_uncompressed_protobuf() {
        let (input, data) = test_data(EncodingOptions::PROTOBUF_UNCOMPRESSED, 16);

        let mut decoder = DecoderApp::new();

        assert_message_incomplete!(decoder.try_read());

        for byte_chunk in data.chunks(1) {
            decoder.push_byte_chunk(byte_chunk.to_vec());
        }

        let decoded_messages: Vec<_> = (0..16)
            .map(|_| assert_message_ok!(decoder.try_read()))
            .collect();

        assert_eq!(input, decoded_messages);
    }

    #[test]
    fn two_concatenated_streams_protobuf() {
        let (input1, data1) = test_data(EncodingOptions::PROTOBUF_UNCOMPRESSED, 16);
        let (input2, data2) = test_data(EncodingOptions::PROTOBUF_UNCOMPRESSED, 16);
        let input = input1.into_iter().chain(input2).collect::<Vec<_>>();

        let mut decoder = DecoderApp::new();

        assert_message_incomplete!(decoder.try_read());

        decoder.push_byte_chunk(data1);
        decoder.push_byte_chunk(data2);

        let decoded_messages: Vec<_> = (0..32)
            .map(|_| assert_message_ok!(decoder.try_read()))
            .collect();

        assert_eq!(input, decoded_messages);
    }

    #[test]
    fn stream_whole_chunks_compressed_protobuf() {
        let (input, data) = test_data(EncodingOptions::PROTOBUF_COMPRESSED, 16);

        let mut decoder = DecoderApp::new();

        assert_message_incomplete!(decoder.try_read());

        decoder.push_byte_chunk(data);

        let decoded_messages: Vec<_> = (0..16)
            .map(|_| assert_message_ok!(decoder.try_read()))
            .collect();

        assert_eq!(input, decoded_messages);
    }

    #[test]
    fn stream_byte_chunks_compressed_protobuf() {
        let (input, data) = test_data(EncodingOptions::PROTOBUF_COMPRESSED, 16);

        let mut decoder = DecoderApp::new();

        assert_message_incomplete!(decoder.try_read());

        for byte_chunk in data.chunks(1) {
            decoder.push_byte_chunk(byte_chunk.to_vec());
        }

        let decoded_messages: Vec<_> = (0..16)
            .map(|_| assert_message_ok!(decoder.try_read()))
            .collect();

        assert_eq!(input, decoded_messages);
    }

    #[test]
    fn stream_3x16_chunks_protobuf() {
        let (input, data) = test_data(EncodingOptions::PROTOBUF_COMPRESSED, 16);

        let mut decoder = DecoderApp::new();
        let mut decoded_messages = vec![];

        // keep pushing 3 chunks of 16 bytes at a time, and attempting to read messages
        // until there are no more chunks
        let mut byte_chunks = data.chunks(16).peekable();
        while byte_chunks.peek().is_some() {
            for _ in 0..3 {
                if let Some(byte_chunk) = byte_chunks.next() {
                    decoder.push_byte_chunk(byte_chunk.to_vec());
                } else {
                    break;
                }
            }

            if let Some(message) = decoder.try_read().unwrap() {
                decoded_messages.push(message);
            }
        }

        assert_eq!(input, decoded_messages);
    }

    #[test]
    fn stream_irregular_chunks_protobuf() {
        // this attempts to stress-test `try_read` with byte chunks of various sizes

        let (input, data) = test_data(EncodingOptions::PROTOBUF_COMPRESSED, 16);
        let mut data = Cursor::new(data);

        let mut decoder = DecoderApp::new();
        let mut decoded_messages = vec![];

        // read byte chunks 2xN bytes at a time, where `N` comes from a regular pattern
        // this is slightly closer to using random numbers while still being
        // fully deterministic

        let pattern = [0, 3, 4, 70, 31];
        let mut pattern_index = 0;
        let mut temp = [0_u8; 71];

        while data.position() < data.get_ref().len() as u64 {
            for _ in 0..2 {
                let n = data.read(&mut temp[..pattern[pattern_index]]).unwrap();
                pattern_index = (pattern_index + 1) % pattern.len();
                decoder.push_byte_chunk(temp[..n].to_vec());
            }

            while let Some(message) = decoder.try_read().unwrap() {
                decoded_messages.push(message);
            }
        }

        assert_eq!(input, decoded_messages);
    }

    #[test]
    fn chunk_buffer_read_single_chunk() {
        // reading smaller `n` from multiple larger byte chunks

        let mut buffer = ByteChunkBuffer::new();

        let data = &[0, 1, 2, 3, 4];
        assert_eq!(None, buffer.try_read(1));
        buffer.push(data.to_vec());
        assert_eq!(Some(&data[..3]), buffer.try_read(3).as_deref());
        assert_eq!(Some(&data[3..]), buffer.try_read(2).as_deref());
        assert_eq!(None, buffer.try_read(1));
    }

    #[test]
    fn chunk_buffer_read_multi_chunk() {
        // reading a large `n` from multiple smaller byte chunks

        let mut buffer = ByteChunkBuffer::new();

        let byte_chunks: &[&[u8]] = &[&[0, 1, 2], &[3, 4]];

        assert_eq!(None, buffer.try_read(1));
        buffer.push(byte_chunks[0].to_vec());
        assert_eq!(None, buffer.try_read(5));
        buffer.push(byte_chunks[1].to_vec());
        assert_eq!(Some(&[0, 1, 2, 3, 4][..]), buffer.try_read(5).as_deref());
        assert_eq!(None, buffer.try_read(1));
    }

    #[test]
    fn chunk_buffer_read_same_n() {
        // reading the same `n` multiple times should not return the same bytes

        let mut buffer = ByteChunkBuffer::new();

        let data = &[0, 1, 2, 3];
        buffer.push(data.to_vec());
        assert_eq!(data, buffer.try_read(4).as_deref().unwrap());
        assert_eq!(None, buffer.try_read(4));
        let data = &[4, 5, 6, 7];
        buffer.push(data.to_vec());
        assert_eq!(data, buffer.try_read(4).as_deref().unwrap());
        assert_eq!(None, buffer.try_read(4));
    }
}
