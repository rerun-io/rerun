use std::collections::VecDeque;
use std::io::Cursor;
use std::io::Read as _;

use re_build_info::CrateVersion;

use crate::codec::Serializer;
use crate::codec::file::EncodingOptions;
use crate::codec::file::FileHeader;
use crate::decoder::{ApplicationIdInjector, CachingApplicationIdInjector, DecodeError};

// ---

// TODO(cmc): This trait will be vastly improved and documented in a follow up that completely
// revamps how codecs are defined and organized. For now, it's just undocumented bare minimum to
// make `Decoder` work everywhere.

pub trait FileEncoded: Sized {
    fn decode(
        app_id_injector: &mut impl ApplicationIdInjector,
        message_kind: crate::codec::file::MessageKind,
        buf: &[u8],
    ) -> Result<Option<Self>, DecodeError>;

    fn propagate_version(&mut self, version: Option<CrateVersion>) {
        _ = self;
        _ = version;
    }
}

impl FileEncoded for re_log_types::LogMsg {
    fn decode(
        app_id_injector: &mut impl ApplicationIdInjector,
        message_kind: crate::codec::file::MessageKind,
        buf: &[u8],
    ) -> Result<Option<Self>, DecodeError> {
        crate::codec::file::decoder::decode_bytes_to_app(app_id_injector, message_kind, buf)
    }

    fn propagate_version(&mut self, version: Option<CrateVersion>) {
        if let Self::SetStoreInfo(msg) = self {
            // Propagate the protocol version from the header into the `StoreInfo` so that all
            // parts of the app can easily access it.
            msg.info.store_version = version;
        }
    }
}

impl FileEncoded for re_protos::log_msg::v1alpha1::log_msg::Msg {
    fn decode(
        _app_id_injector: &mut impl ApplicationIdInjector,
        message_kind: crate::codec::file::MessageKind,
        buf: &[u8],
    ) -> Result<Option<Self>, DecodeError> {
        crate::codec::file::decoder::decode_bytes_to_transport(message_kind, buf)
    }
}

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

/// The stream decoder is a state machine which ingests byte chunks and outputs messages once it
/// has enough data to deserialize one.
///
/// Byte chunks are given to the stream via [`DecoderApp::push_byte_chunk`], and messages are read
/// back via [`DecoderApp::try_read`].
//
// TODO(cmc): explain when you'd use this over StreamingDecoder and vice-versa.
pub struct Decoder<T: FileEncoded> {
    /// The Rerun version used to encode the RRD data.
    ///
    /// `None` until a Rerun header has been processed.
    version: Option<CrateVersion>,

    options: EncodingOptions,

    /// Incoming byte chunks are stored here.
    byte_chunks: ByteChunkBuffer,

    /// The stream state.
    state: State,

    /// The application id cache used for migrating old data.
    app_id_cache: CachingApplicationIdInjector,

    _decodable: std::marker::PhantomData<T>,
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
enum State {
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
    Message(crate::codec::file::MessageHeader),

    /// Stop reading.
    Aborted,
}

impl<T: FileEncoded> Decoder<T> {
    /// Instantiates a new lazy decoding iterator on top of the given buffered reader.
    ///
    /// This does not perform any IO until the returned iterator is polled. I.e. this will not
    /// fail if the reader doesn't even contain valid RRD data.
    ///
    /// This takes a `BufRead` instead of a `Read` because:
    /// * This guarantees this will never run on non-buffered input.
    /// * This lets the end-user in control of the buffering, which prevents unfortunately stacked
    ///   buffers (and thus exploding memory usage and copies).
    ///
    /// See also [`Self::decode_lazy_with_opts`].
    pub fn decode_lazy<R: std::io::BufRead>(reader: R) -> DecoderIterator<T, R> {
        let wait_for_eos = false;
        Self::decode_lazy_with_opts(reader, wait_for_eos)
    }

    /// Same as [`Self::decode_lazy`], with extra options.
    ///
    /// * `wait_for_eos`: if true, the decoder will always wait for an end-of-stream marker before
    ///   calling it a day, even if the underlying reader has already reached its EOF state (…for now).
    ///   This only really makes sense when running in tail mode (see `RetryableFileReader`), otherwise
    ///   we'd rather terminate early when a potentially short-circuited (and therefore lacking a proper
    ///   end-of-stream marker) RRD stream indicates EOF.
    pub fn decode_lazy_with_opts<R: std::io::BufRead>(
        reader: R,
        wait_for_eos: bool,
    ) -> DecoderIterator<T, R> {
        let decoder = Self::new();
        DecoderIterator {
            decoder,
            reader,
            wait_for_eos,
            first_msg: None,
        }
    }

    /// Instantiates a new eager decoding iterator on top of the given buffered reader.
    ///
    /// This will perform a first decoding pass immediately. This allows this constructor to fail
    /// synchronously if the underlying reader doesn't even contain valid RRD data at all (e.g. magic
    /// bytes are not present).
    ///
    /// This takes a `BufRead` instead of a `Read` because:
    /// * This guarantees this will never run on non-buffered input.
    /// * This lets the end-user in control of the buffering, which prevents unfortunately stacked
    ///   buffers (and thus exploding memory usage and copies).
    ///
    /// See also [`Self::decode_eager_with_opts`].
    pub fn decode_eager<R: std::io::BufRead>(
        reader: R,
    ) -> Result<DecoderIterator<T, R>, DecodeError> {
        let wait_for_eos = false;
        Self::decode_eager_with_opts(reader, wait_for_eos)
    }

    /// Same as [`Self::decode_eager`], with extra options.
    ///
    /// * `wait_for_eos`: if true, the decoder will always wait for an end-of-stream marker before
    ///   calling it a day, even if the underlying reader has already reached its EOF state (…for now).
    ///   This only really makes sense when running in tail mode (see `RetryableFileReader`), otherwise
    ///   we'd rather terminate early when a potentially short-circuited (and therefore lacking a proper
    ///   end-of-stream marker) RRD stream indicates EOF.
    pub fn decode_eager_with_opts<R: std::io::BufRead>(
        reader: R,
        wait_for_eos: bool,
    ) -> Result<DecoderIterator<T, R>, DecodeError> {
        let decoder = Self::new();
        let mut it = DecoderIterator {
            decoder,
            reader,
            wait_for_eos,
            first_msg: None,
        };

        it.first_msg = it.next().transpose()?;

        Ok(it)
    }
}

impl<T: FileEncoded> Decoder<T> {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            version: None,
            // Note: `options` are filled in once we read `FileHeader`, so this value does not matter.
            options: EncodingOptions::PROTOBUF_UNCOMPRESSED,
            byte_chunks: ByteChunkBuffer::new(),
            state: State::StreamHeader,
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
            if let Err(DecodeError::StoreIdMissingApplicationId {
                store_kind,
                recording_id,
            }) = result
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
            State::StreamHeader => {
                let is_first_header = self.byte_chunks.num_read() == 0;
                if let Some(header) = self.byte_chunks.try_read(FileHeader::SIZE) {
                    re_log::trace!(?header, "Decoding StreamHeader");

                    // header contains version and compression options
                    let (version, options) = match FileHeader::options_from_bytes(header) {
                        Ok(ok) => ok,
                        Err(err) => {
                            // We expected a header, but didn't find one!
                            if is_first_header {
                                return Err(err);
                            } else {
                                re_log::error!("Trailing bytes in rrd stream: {header:?} ({err})");
                                self.state = State::Aborted;
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
                        Serializer::Protobuf => self.state = State::MessageHeader,
                    }

                    // we might have data left in the current byte chunk, immediately try to read
                    // length of the next message.
                    return self.try_read();
                }
            }

            State::MessageHeader => {
                let mut peeked = [0u8; crate::codec::file::MessageHeader::SIZE_BYTES];
                if self.byte_chunks.try_peek(&mut peeked) == peeked.len() {
                    let header = match crate::codec::file::MessageHeader::from_bytes(&peeked) {
                        Ok(header) => header,

                        Err(DecodeError::Codec(crate::codec::CodecError::UnknownMessageHeader)) => {
                            // We failed to decode a `MessageHeader`: it might be because the
                            // stream is corrupt, or it might be because it just switched to a
                            // different, concatenated recording without having the courtesy of
                            // announcing it via an EOS marker.
                            self.state = State::StreamHeader;
                            return self.try_read();
                        }

                        err @ Err(_) => err?,
                    };

                    self.byte_chunks
                        .try_read(crate::codec::file::MessageHeader::SIZE_BYTES)
                        .expect("reading cannot fail if peeking worked");

                    re_log::trace!(?header, "MessageHeader");

                    self.state = State::Message(header);
                    // we might have data left in the current byte chunk, immediately try to read
                    // the message content.
                    return self.try_read();
                }
            }

            State::Message(header) => {
                if let Some(bytes) = self.byte_chunks.try_read(header.len as usize) {
                    re_log::trace!(?header, "Read message");

                    let message = match T::decode(&mut self.app_id_cache, header.kind, bytes) {
                        Ok(msg) => msg,
                        Err(err) => {
                            // We successfully parsed a header, but decided to drop the message altogether.
                            // We must go back to looking for headers, or the decoder will just be stuck in a dead
                            // state forever.
                            self.state = State::MessageHeader;
                            return Err(err);
                        }
                    };

                    if let Some(mut message) = message {
                        re_log::trace!("Decoded new message");

                        message.propagate_version(self.version);
                        self.state = State::MessageHeader;
                        return Ok(Some(message));
                    } else {
                        re_log::trace!("End of stream - expecting a new Streamheader");

                        // `None` means an end-of-stream marker was hit, but there might be another concatenated
                        // stream behind, so try to start all over again.
                        self.state = State::StreamHeader;
                        return self.try_read();
                    }
                }
            }

            State::Aborted => {
                return Ok(None);
            }
        }

        Ok(None)
    }
}

// ---

/// Iteratively decodes the contents of an arbitrary buffered reader.
pub struct DecoderIterator<T: FileEncoded, R: std::io::BufRead> {
    decoder: Decoder<T>,
    reader: R,

    /// If true, the decoder will always wait for an end-of-stream marker before calling it a day,
    /// even if the underlying reader has already reached its EOF state (…for now).
    ///
    /// This only really makes sense when running in tail mode (see `RetryableFileReader`),
    /// otherwise we'd rather terminate early when a potentially short-circuited (and therefore
    /// lacking a proper end-of-stream marker) RRD stream indicates EOF.
    wait_for_eos: bool,

    /// See [`Decoder::decode_eager`] for more information.
    first_msg: Option<T>,
}

impl<T: FileEncoded, R: std::io::BufRead> DecoderIterator<T, R> {
    pub fn num_bytes_processed(&self) -> u64 {
        self.decoder.byte_chunks.num_read() as _
    }
}

impl<T: FileEncoded, R: std::io::BufRead> std::iter::Iterator for DecoderIterator<T, R> {
    type Item = Result<T, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(first_msg) = self.first_msg.take() {
            // The iterator was eagerly initialized so make sure to return the first message if there's any.
            return Some(Ok(first_msg));
        }

        loop {
            match self.decoder.try_read() {
                Ok(Some(msg)) => return Some(Ok(msg)),
                Ok(None) => {}
                Err(err) => return Some(Err(err)),
            }

            match self.reader.fill_buf() {
                // EOF
                Ok([]) => {
                    // There's nothing more to read…
                    match self.decoder.try_read() {
                        // …but we still have enough buffered that we can still manage to decode
                        // more messages, so go on for now.
                        Ok(Some(msg)) => return Some(Ok(msg)),

                        // …and we don't want to explicitly wait around for more to come, so just leave.
                        Ok(None) if !self.wait_for_eos => return None,

                        // …and the underlying decoder already considers that it's done (i.e. it's
                        // waiting for a whole new stream to begin): time to stop.
                        Ok(None) if self.decoder.state == State::StreamHeader => {
                            return None;
                        }

                        // …but the underlying decoder doesn't believe it's done yet (i.e. it's still
                        // waiting for an EOS marker to show up): we continue.
                        Ok(None) => {}

                        Err(err) => return Some(Err(err)),
                    }
                }

                Ok(buf) => {
                    self.decoder.push_byte_chunk(buf.to_vec());
                    let len = buf.len(); // borrowck limitation
                    self.reader.consume(len);
                }

                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}

                Err(err) => return Some(Err(err.into())),
            }
        }
    }
}

// ---

/// A bunch of contiguous bytes.
type ByteChunk = Cursor<Vec<u8>>;

struct ByteChunkBuffer {
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
    fn num_read(&self) -> usize {
        self.num_read
    }

    /// Attempt to read exactly `n` bytes out of the queued byte chunks.
    ///
    /// Returns `None` if there is not enough data to return a slice of `n` bytes.
    ///
    /// NOTE: `try_read` *must* be called with the same `n` until it returns `Some`,
    /// otherwise this will discard any previously buffered data.
    fn try_read(&mut self, n: usize) -> Option<&[u8]> {
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
            Some(&self.buffer[..])
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

    use crate::Encoder;
    use crate::EncodingOptions;

    use super::*;

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
        assert_eq!(Some(&data[..3]), buffer.try_read(3));
        assert_eq!(Some(&data[3..]), buffer.try_read(2));
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
        assert_eq!(Some(&[0, 1, 2, 3, 4][..]), buffer.try_read(5));
        assert_eq!(None, buffer.try_read(1));
    }

    #[test]
    fn chunk_buffer_read_same_n() {
        // reading the same `n` multiple times should not return the same bytes

        let mut buffer = ByteChunkBuffer::new();

        let data = &[0, 1, 2, 3];
        buffer.push(data.to_vec());
        assert_eq!(data, buffer.try_read(4).unwrap());
        assert_eq!(None, buffer.try_read(4));
        let data = &[4, 5, 6, 7];
        buffer.push(data.to_vec());
        assert_eq!(data, buffer.try_read(4).unwrap());
        assert_eq!(None, buffer.try_read(4));
    }
}

// Legacy tests from the old decoder implementation.
#[cfg(all(test, feature = "decoder", feature = "encoder"))]
mod tests_legacy {
    #![allow(clippy::unwrap_used)] // acceptable for tests

    use re_build_info::CrateVersion;
    use re_chunk::RowId;
    use re_log_types::{LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};
    use re_protos::log_msg::v1alpha1 as proto;
    use re_protos::log_msg::v1alpha1::LogMsg as LogMsgProto;

    use crate::Encoder;
    use crate::codec::Compression;

    use super::*;

    fn fake_log_messages() -> Vec<LogMsg> {
        let store_id = StoreId::random(StoreKind::Blueprint, "test_app");

        let arrow_msg = re_chunk::Chunk::builder("test_entity")
            .with_archetype(
                re_chunk::RowId::new(),
                re_log_types::TimePoint::default().with(
                    re_log_types::Timeline::new_sequence("blueprint"),
                    re_log_types::TimeInt::from_millis(re_log_types::NonMinI64::MIN),
                ),
                &re_types::blueprint::archetypes::Background::new(
                    re_types::blueprint::components::BackgroundKind::SolidColor,
                )
                .with_color([255, 0, 0]),
            )
            .build()
            .unwrap()
            .to_arrow_msg()
            .unwrap();

        vec![
            LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::new(),
                info: StoreInfo::new(
                    store_id.clone(),
                    StoreSource::RustSdk {
                        rustc_version: String::new(),
                        llvm_version: String::new(),
                    },
                ),
            }),
            LogMsg::ArrowMsg(store_id.clone(), arrow_msg),
            LogMsg::BlueprintActivationCommand(re_log_types::BlueprintActivationCommand {
                blueprint_id: store_id,
                make_active: true,
                make_default: true,
            }),
        ]
    }

    /// Convert the test log message to their proto version and tweak them so that:
    /// - `StoreId` do not have an `ApplicationId`
    /// - `StoreInfo` does have an `ApplicationId`
    #[expect(deprecated)]
    fn legacy_fake_log_messages() -> Vec<LogMsgProto> {
        fake_log_messages()
            .into_iter()
            .map(|msg| {
                crate::protobuf_conversions::log_msg_to_proto(msg, Compression::Off).unwrap()
            })
            .map(|mut log_msg| {
                match &mut log_msg.msg {
                    None => panic!("Unexpected `LogMsg` without payload"),

                    Some(proto::log_msg::Msg::SetStoreInfo(set_store_info)) => {
                        if let Some(store_info) = &mut set_store_info.info {
                            let Some(mut store_id) = store_info.store_id.clone() else {
                                panic!("Unexpected missing `StoreId`");
                            };

                            // this should be a non-legacy proto
                            assert_eq!(store_info.application_id, None);
                            assert!(store_id.application_id.is_some());

                            // turn this into a legacy proto
                            store_info.application_id = store_id.application_id;
                            store_id.application_id = None;
                            store_info.store_id = Some(store_id);
                        } else {
                            panic!("Unexpected missing `store_info`")
                        }
                    }
                    Some(
                        proto::log_msg::Msg::ArrowMsg(proto::ArrowMsg { store_id, .. })
                        | proto::log_msg::Msg::BlueprintActivationCommand(
                            proto::BlueprintActivationCommand {
                                blueprint_id: store_id,
                                ..
                            },
                        ),
                    ) => {
                        let mut legacy_store_id =
                            store_id.clone().expect("messages should have store ids");
                        assert!(legacy_store_id.application_id.is_some());

                        // make legacy
                        legacy_store_id.application_id = None;
                        *store_id = Some(legacy_store_id);
                    }
                }

                log_msg
            })
            .collect()
    }

    impl<W: std::io::Write> Encoder<W> {
        /// Like [`Self::encode_into`], but intentionally omits the end-of-stream marker, for
        /// testing purposes.
        fn encode_into_without_eos(
            version: CrateVersion,
            options: EncodingOptions,
            messages: impl IntoIterator<Item = re_chunk::ChunkResult<impl std::borrow::Borrow<LogMsg>>>,
            write: &mut W,
        ) -> Result<u64, crate::EncodeError> {
            re_tracing::profile_function!();
            let mut encoder = Encoder::new(version, options, write)?;
            let mut size_bytes = 0;
            for message in messages {
                size_bytes += encoder.append(message?.borrow())?;
            }

            {
                encoder.flush_blocking()?;

                // Intentionally leak it so we don't include the EOS marker on drop.
                #[expect(clippy::mem_forget)]
                std::mem::forget(encoder);
            }

            Ok(size_bytes)
        }
    }

    #[test]
    fn test_encode_decode() {
        let rrd_version = CrateVersion::LOCAL;

        let messages = fake_log_messages();

        let options = [
            EncodingOptions {
                compression: Compression::Off,
                serializer: Serializer::Protobuf,
            },
            EncodingOptions {
                compression: Compression::LZ4,
                serializer: Serializer::Protobuf,
            },
        ];

        // Low-level
        for options in options {
            let mut file = vec![];
            crate::Encoder::encode_into(rrd_version, options, messages.iter().map(Ok), &mut file)
                .unwrap();

            let decoded_messages: Vec<_> = DecoderApp::decode_lazy(file.as_slice())
                .map(Result::unwrap)
                .collect();
            similar_asserts::assert_eq!(decoded_messages, messages);
        }

        // Iterator
        for options in options {
            let mut file = vec![];
            crate::Encoder::encode_into(rrd_version, options, messages.iter().map(Ok), &mut file)
                .unwrap();

            let reader = std::io::BufReader::new(file.as_slice());
            let decoded_messages: Vec<_> = DecoderApp::decode_lazy(reader)
                .map(Result::unwrap)
                .collect();
            similar_asserts::assert_eq!(decoded_messages, messages);
        }

        // Iterator: no EOS marker
        for options in options {
            let mut file = vec![];
            crate::Encoder::encode_into_without_eos(
                rrd_version,
                options,
                messages.iter().map(Ok),
                &mut file,
            )
            .unwrap();

            let reader = std::io::BufReader::new(file.as_slice());
            let decoded_messages: Vec<_> = DecoderApp::decode_lazy(reader)
                .map(Result::unwrap)
                .collect();
            similar_asserts::assert_eq!(decoded_messages, messages);
        }
    }

    /// Test that legacy messages (aka `StoreId` without an application id) are properly decoded.
    #[test]
    fn test_decode_legacy() {
        let rrd_version = CrateVersion::LOCAL;

        let messages = legacy_fake_log_messages();

        let options = [
            EncodingOptions {
                compression: Compression::Off,
                serializer: Serializer::Protobuf,
            },
            EncodingOptions {
                compression: Compression::LZ4,
                serializer: Serializer::Protobuf,
            },
        ];

        for options in options {
            let mut file = vec![];

            let mut encoder = Encoder::new(rrd_version, options, &mut file).unwrap();
            for message in messages.clone() {
                encoder
                    .append_proto(message)
                    .expect("encoding should succeed");
            }
            drop(encoder);

            let decoded_messages: Vec<_> = DecoderApp::decode_lazy(file.as_slice())
                .map(Result::unwrap)
                .collect();
            assert_eq!(decoded_messages.len(), messages.len());
        }
    }

    /// Test that legacy messages (aka `StoreId` without an application id) that arrive _before_
    /// a `SetStoreInfo` are dropped without failing.
    #[test]
    fn test_decode_legacy_out_of_order() {
        let rrd_version = CrateVersion::LOCAL;

        let messages = legacy_fake_log_messages();

        // ensure the test data is as we expect
        let orig_message_count = messages.len();
        assert_eq!(orig_message_count, 3);
        assert!(matches!(
            messages[0].msg,
            Some(proto::log_msg::Msg::SetStoreInfo(..))
        ));
        assert!(matches!(
            messages[1].msg,
            Some(proto::log_msg::Msg::ArrowMsg(..))
        ));
        assert!(matches!(
            messages[2].msg,
            Some(proto::log_msg::Msg::BlueprintActivationCommand(..))
        ));

        let options = [
            EncodingOptions {
                compression: Compression::Off,
                serializer: Serializer::Protobuf,
            },
            EncodingOptions {
                compression: Compression::LZ4,
                serializer: Serializer::Protobuf,
            },
        ];

        // make out-of-order messages
        let mut out_of_order_messages = vec![messages[1].clone(), messages[2].clone()];
        out_of_order_messages.extend(messages);

        for options in options {
            let mut file = vec![];

            let mut encoder = Encoder::new(rrd_version, options, &mut file).unwrap();
            for message in out_of_order_messages.clone() {
                encoder
                    .append_proto(message)
                    .expect("encoding should succeed");
            }
            drop(encoder);

            let decoded_messages: Vec<_> = DecoderApp::decode_lazy(file.as_slice())
                .map(Result::unwrap)
                .collect();
            assert_eq!(decoded_messages.len(), orig_message_count);
        }
    }

    /// Test that non-legacy message streams do not rely on the `SetStoreInfo` message to arrive first.
    #[test]
    fn test_decode_out_of_order() {
        let rrd_version = CrateVersion::LOCAL;

        let messages = fake_log_messages();

        // ensure the test data is as we expect
        let orig_message_count = messages.len();
        assert_eq!(orig_message_count, 3);
        assert!(matches!(messages[0], LogMsg::SetStoreInfo { .. }));
        assert!(matches!(messages[1], LogMsg::ArrowMsg { .. }));
        assert!(matches!(
            messages[2],
            LogMsg::BlueprintActivationCommand { .. }
        ));

        let options = [
            EncodingOptions {
                compression: Compression::Off,
                serializer: Serializer::Protobuf,
            },
            EncodingOptions {
                compression: Compression::LZ4,
                serializer: Serializer::Protobuf,
            },
        ];

        // make out-of-order messages
        let mut out_of_order_messages = vec![messages[1].clone(), messages[2].clone()];
        out_of_order_messages.extend(messages);

        for options in options {
            let mut file = vec![];
            crate::Encoder::encode_into(
                rrd_version,
                options,
                out_of_order_messages.iter().map(Ok),
                &mut file,
            )
            .unwrap();

            let decoded_messages: Vec<_> = DecoderApp::decode_lazy(file.as_slice())
                .map(Result::unwrap)
                .collect();
            similar_asserts::assert_eq!(decoded_messages, out_of_order_messages);
        }
    }

    #[test]
    fn test_concatenated_streams() {
        let options = [
            EncodingOptions {
                compression: Compression::Off,
                serializer: Serializer::Protobuf,
            },
            EncodingOptions {
                compression: Compression::LZ4,
                serializer: Serializer::Protobuf,
            },
        ];

        for options in options {
            let mut data = vec![];

            // write "2 files" i.e. 2 streams that end with end-of-stream markers
            let messages = fake_log_messages();

            // (2 encoders as each encoder writes a file header)
            {
                let writer = std::io::Cursor::new(&mut data);
                let mut encoder1 =
                    crate::Encoder::new(CrateVersion::LOCAL, options, writer).unwrap();
                for message in &messages {
                    encoder1.append(message).unwrap();
                }
                encoder1.finish().unwrap();
            }

            let written = data.len() as u64;

            {
                let mut writer = std::io::Cursor::new(&mut data);
                writer.set_position(written);
                let mut encoder2 =
                    crate::Encoder::new(CrateVersion::LOCAL, options, writer).unwrap();
                for message in &messages {
                    encoder2.append(message).unwrap();
                }
                encoder2.finish().unwrap();
            }

            let decoded_messages: Vec<_> = DecoderApp::decode_lazy(data.as_slice())
                .map(Result::unwrap)
                .collect();
            similar_asserts::assert_eq!(decoded_messages, [messages.clone(), messages].concat());
        }

        // Same thing, but this time without EOS markers.
        for options in options {
            let mut data = vec![];

            // write "2 files" i.e. 2 streams that do not end with end-of-stream markers
            let messages = fake_log_messages();

            // (2 encoders as each encoder writes a file header)
            {
                let writer = std::io::Cursor::new(&mut data);
                let mut encoder1 =
                    crate::Encoder::new(CrateVersion::LOCAL, options, writer).unwrap();
                for message in &messages {
                    encoder1.append(message).unwrap();
                }

                // Intentionally leak it so we don't include the EOS marker on drop.
                #[expect(clippy::mem_forget)]
                std::mem::forget(encoder1);
            }

            let written = data.len() as u64;

            {
                let mut writer = std::io::Cursor::new(&mut data);
                writer.set_position(written);
                let mut encoder2 =
                    crate::Encoder::new(CrateVersion::LOCAL, options, writer).unwrap();
                for message in &messages {
                    encoder2.append(message).unwrap();
                }

                // Intentionally leak it so we don't include the EOS marker on drop.
                #[expect(clippy::mem_forget)]
                std::mem::forget(encoder2);
            }

            let decoded_messages: Vec<_> = DecoderApp::decode_lazy(data.as_slice())
                .map(Result::unwrap)
                .collect();
            assert_eq!(messages.len() * 2, decoded_messages.len());
            similar_asserts::assert_eq!(decoded_messages, [messages.clone(), messages].concat());
        }
    }
}
