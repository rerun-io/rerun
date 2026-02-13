use std::collections::VecDeque;
use std::io::{Cursor, Read as _};

use itertools::Itertools as _;
use re_build_info::CrateVersion;

use crate::rrd::MessageHeader;
use crate::{
    CachingApplicationIdInjector, CodecError, Decodable as _, DecodeError, DecoderEntrypoint,
    EncodingOptions, RawRrdManifest, Serializer, StreamFooter, StreamFooterEntry, StreamHeader,
    ToApplication as _,
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

    /// All the RRD manifests accumulated so far by parsing incoming footers in the RRD stream.
    ///
    /// Transport-level types to keep decoding cheap.
    rrd_manifests: Vec<re_protos::log_msg::v1alpha1::RrdManifest>,

    _decodable: std::marker::PhantomData<T>,
}

/// ```text,ignore
///  +--> WaitingForStreamHeader -----> Aborted
///  |        |
///  |        v
///  +--- WaitingForMessageHeader <---+
///  |        |                       |
///  |        v                       |
///  |    WaitingForMessagePayload ---+
///  |        |
///  |        v
///  +--- WaitingForStreamFooter
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderState {
    /// The beginning of a stream.
    ///
    /// The [`StreamHeader`] contains the magic bytes (e.g. `RRF2`), the encoded version, and the
    /// encoding options.
    ///
    /// We can come back to this state at any point because multiple RRD streams might have been
    /// concatenated together (e.g. `cat *.rrd | rerun`).
    WaitingForStreamHeader,

    /// Stream in progress.
    ///
    /// The [`MessageHeader`] indicates what kind of payload this is and how large it is.
    ///
    /// After the [`StreamHeader`] is read once, the state machine will only ever switch between
    /// [`Self::WaitingForMessageHeader`] and [`Self::WaitingForMessagePayload`], until either a
    /// footer or another concatenated RRD stream shows up.
    WaitingForMessageHeader,

    /// A [`MessageHeader`] was parsed, now we're waiting for the associated payload.
    ///
    /// Compression is only applied to individual `ArrowMsg`s, instead of the entire stream.
    WaitingForMessagePayload(MessageHeader),

    /// We hit a message of kind `MessageKind::End`, which means a footer must be following it.
    ///
    /// The [`StreamFooter`] contains information about where the RRD manifests can be found, but we
    /// won't be doing anything with it in this case since we're just going through the data in order.
    ///
    /// Once the footer is parsed, we can wait for a new stream to begin.
    WaitingForStreamFooter,

    /// The stream entered an irrecoverable state and cannot yield data anymore. However, most of the
    /// valuable data was already decoded, so we merely log an error and stop yielding more
    /// messages rather than bubbling up the error all the way to the end user.
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
            state: DecoderState::WaitingForStreamHeader,
            app_id_cache: CachingApplicationIdInjector::default(),
            rrd_manifests: Vec::new(),
            _decodable: std::marker::PhantomData::<T>,
        }
    }

    /// Feed a bunch of bytes to the decoding state machine.
    pub fn push_byte_chunk(&mut self, byte_chunk: Vec<u8>) {
        self.byte_chunks.push(byte_chunk);
    }

    /// Returns all the RRD manifests accumulated _so far_.
    ///
    /// RRD manifests are parsed from footers, of which there might be more than one e.g. in the
    /// case of concatenated streams.
    ///
    /// This is not cheap: it automatically performs the transport to app level conversion.
    pub fn rrd_manifests(&self) -> Result<Vec<RawRrdManifest>, DecodeError> {
        re_tracing::profile_function!();
        self.rrd_manifests
            .iter()
            .map(|m| m.to_application(()).map_err(Into::into))
            .collect()
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
                    "dropping message without application id which arrived before `SetStoreInfo` \
                    (kind: {store_kind}, recording id: {recording_id}."
                );
            } else {
                return result;
            }
        }
    }

    /// Read the next message in the stream.
    fn try_read_impl(&mut self) -> Result<Option<T>, DecodeError> {
        // Enable this for easy debugging of the state machine.
        if false {
            use bytes::Buf as _;
            let num_bytes = self
                .byte_chunks
                .queue
                .iter()
                .map(|v| v.remaining())
                .sum::<usize>();

            eprintln!("state: {:?} (bytes available: {num_bytes})", self.state);

            let mut peeked = [0u8; 32];
            self.byte_chunks.try_peek(&mut peeked);
            let peeked = peeked
                .into_iter()
                .map(|b| match b {
                    b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => String::from(b as char),
                    v => v.to_string(),
                })
                .join(", ");

            eprintln!("upcoming 32 bytes: [{peeked}]");
        }

        match self.state {
            DecoderState::WaitingForStreamHeader => {
                let is_first_header = self.byte_chunks.num_read() == 0;
                let position = self.byte_chunks.num_read();
                if let Some(header_data) =
                    self.byte_chunks.try_read(StreamHeader::ENCODED_SIZE_BYTES)
                {
                    // header contains version and compression options
                    let version_and_options = StreamHeader::from_rrd_bytes(&header_data)
                        .and_then(|h| h.to_version_and_options());
                    let (version, options) = match version_and_options {
                        Ok(ok) => ok,
                        Err(err) => {
                            if is_first_header {
                                // We expected a header, but didn't find one!
                                return Err(err.into());
                            } else {
                                // A bunch of weird trailing bytes means we're now in an irrecoverable state, but
                                // it doesn't change the fact that we've just successfully yielded an entire stream's
                                // worth of data. So, instead of bubbling up errors all the way to the end user,
                                // merely log one here stop the state machine forever.
                                re_log::error!(
                                    is_first_header,
                                    position,
                                    "trailing bytes in rrd stream: {header_data:?} ({err})"
                                );
                                self.state = DecoderState::Aborted;
                                return Ok(None);
                            }
                        }
                    };

                    re_log::trace!(
                        version = version.to_string(),
                        ?options,
                        "found StreamHeader"
                    );

                    self.version = Some(version);
                    self.options = options;

                    match self.options.serializer {
                        Serializer::Protobuf => self.state = DecoderState::WaitingForMessageHeader,
                    }

                    // we might have data left in the current byte chunk, immediately try to read
                    // length of the next message.
                    return self.try_read();
                }

                // Not enough data yet -- wait to be fed and called back once again.
            }

            DecoderState::WaitingForMessageHeader => {
                let mut peeked = [0u8; MessageHeader::ENCODED_SIZE_BYTES];
                if self.byte_chunks.try_peek(&mut peeked) == peeked.len() {
                    let header = match MessageHeader::from_rrd_bytes(&peeked) {
                        Ok(header) => header,

                        Err(crate::rrd::CodecError::FrameDecoding(_)) => {
                            // We failed to decode a `MessageHeader`: it might be because the stream is corrupt,
                            // or it might be because it just switched to a different, concatenated recording
                            // without having the courtesy of announcing it via an EOS marker.
                            //
                            // TODO(cmc): These kinds of peeking shenanigans should never be necessary, need to
                            // write a proposal for RRF3 that addresses these issues and more.
                            self.state = DecoderState::WaitingForStreamHeader;
                            return self.try_read();
                        }

                        err @ Err(_) => err?,
                    };

                    self.byte_chunks
                        .try_read(MessageHeader::ENCODED_SIZE_BYTES)
                        .expect("reading cannot fail if peeking worked");

                    re_log::trace!(?header, "found MessageHeader");

                    self.state = DecoderState::WaitingForMessagePayload(header);
                    return self.try_read();
                }

                // Not enough data yet -- wait to be fed and called back once again.
            }

            DecoderState::WaitingForMessagePayload(header) => {
                let start_offset = self.byte_chunks.num_read() as u64;

                if let Some(bytes) = self.byte_chunks.try_read(header.len as usize) {
                    let bytes_len = bytes.len() as u64;
                    let byte_span = re_chunk::Span {
                        start: start_offset,
                        len: bytes_len,
                    };
                    let message = match T::decode(
                        bytes.clone(),
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
                            self.state = DecoderState::WaitingForMessageHeader;
                            return Err(err.into());
                        }
                    };

                    if let Some(message) = message {
                        self.state = DecoderState::WaitingForMessageHeader;
                        return Ok(Some(message));
                    } else {
                        re_log::trace!(
                            "End of stream - expecting either a StreamFooter or a new Streamheader"
                        );

                        if !bytes.is_empty() {
                            let rrd_footer =
                                re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(&bytes)?;
                            self.rrd_manifests.extend(rrd_footer.manifests);

                            // A non-empty ::End message means there must be a footer ahead, no exception.
                            self.state = DecoderState::WaitingForStreamFooter;
                        } else {
                            // There are 2 possible scenarios where we can end up here:
                            // * The recording doesn't contain any data messages (e.g. it's empty except for
                            //   a `SetStoreInfo` message).
                            // * Backward compatibility: the payload is empty (i.e. `header.len == 0`) because the
                            //   `End` message was written by a legacy encoder that predates the introduction of footers.
                            //
                            // Either way, we have to expect a footer next, since we don't know for sure which scenario
                            // we're in. We could check the encoder version and such, but that's just superfluous complexity
                            // since `WaitingForStreamFooter` already knows how to deal with optional footers anyway.
                            self.state = DecoderState::WaitingForStreamFooter;
                        }

                        return self.try_read();
                    }
                }

                // Not enough data yet -- wait to be fed and called back once again.
            }

            DecoderState::WaitingForStreamFooter => {
                // NOTE: We're not peeking here! If we enter this state, then there must be a footer
                // ahead, no exception, otherwise that's a violation of the framing protocol.
                let position = self.byte_chunks.num_read();
                if let Some(bytes) = self
                    .byte_chunks
                    .try_read(crate::rrd::StreamFooter::ENCODED_SIZE_BYTES)
                {
                    match crate::rrd::StreamFooter::from_rrd_bytes(&bytes) {
                        Ok(footer) => {
                            re_log::trace!(?footer, "found StreamFooter");

                            let StreamFooter {
                                fourcc: _,
                                identifier: _,
                                entries,
                            } = &footer;

                            for entry in entries {
                                let StreamFooterEntry {
                                    rrd_footer_byte_span_from_start_excluding_header,
                                    crc_excluding_header: _,
                                } = entry;

                                let rrd_footer_end =
                                    rrd_footer_byte_span_from_start_excluding_header.end();

                                if rrd_footer_end > position as u64 {
                                    // The RRD footer cannot possibly end after the stream footer starts, since it must
                                    // be part of an ::End message.
                                    re_log::error!(
                                        position,
                                        bytes = ?bytes,
                                        ?footer,
                                        err = "offsets are invalid",
                                        "corrupt footer in rrd stream"
                                    );
                                }
                            }

                            // And now we start all over.
                            self.state = DecoderState::WaitingForStreamHeader;
                            return self.try_read();
                        }

                        Err(err) => {
                            // A corrupt footer means we're now in an irrecoverable state, but it doesn't change the
                            // fact that we've just successfully yielded an entire stream's worth of data. So, instead
                            // of bubbling up errors all the way to the end user, merely log one here stop the state
                            // machine forever.
                            re_log::error!(
                                position,
                                bytes = ?bytes,
                                %err,
                                "corrupt footer in rrd stream"
                            );
                            self.state = DecoderState::Aborted;
                            return Ok(None);
                        }
                    }
                }

                // Not enough data yet -- wait to be fed and called back once again.
            }

            DecoderState::Aborted => return Ok(None),
        }

        Ok(None) // Not enough data yet -- wait to be fed and called back once again.
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
    use crate::Encoder;
    use crate::rrd::EncodingOptions;

    fn fake_log_msg() -> LogMsg {
        LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::ZERO,
            info: StoreInfo {
                store_version: Some(CrateVersion::LOCAL), // Encoder sets the crate version
                ..StoreInfo::testing_with_recording_id("test_recording")
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
