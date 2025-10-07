use std::collections::VecDeque;
use std::io::Cursor;
use std::io::Read as _;

use re_build_info::CrateVersion;
use re_log_types::LogMsg;

use crate::EncodingOptions;
use crate::FileHeader;
use crate::Serializer;
use crate::app_id_injector::CachingApplicationIdInjector;
use crate::decoder::options_from_bytes;

use super::DecodeError;

/// The stream decoder is a state machine which ingests byte chunks and outputs messages once it
/// has enough data to deserialize one.
///
/// Byte chunks are given to the stream via [`StreamDecoder::push_byte_chunk`], and messages are read
/// back via [`StreamDecoder::try_read`].
//
// TODO(cmc): explain when you'd use this over StreamingDecoder and vice-versa.
pub struct StreamDecoder {
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

    /// Stop reading
    Aborted,
}

impl StreamDecoder {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            version: None,
            // Note: `options` are filled in once we read `FileHeader`, so this value does not matter.
            options: EncodingOptions::PROTOBUF_UNCOMPRESSED,
            byte_chunks: ByteChunkBuffer::new(),
            state: State::StreamHeader,
            app_id_cache: CachingApplicationIdInjector::default(),
        }
    }

    /// Feed a bunch of bytes to the decoding state machine.
    pub fn push_byte_chunk(&mut self, byte_chunk: Vec<u8>) {
        self.byte_chunks.push(byte_chunk);
    }

    /// Read the next message in the stream, dropping messages missing application id that cannot
    /// be migrated (because they arrived before `SetStoreInfo`).
    pub fn try_read(&mut self) -> Result<Option<LogMsg>, DecodeError> {
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
    fn try_read_impl(&mut self) -> Result<Option<LogMsg>, DecodeError> {
        match self.state {
            State::StreamHeader => {
                let is_first_header = self.byte_chunks.num_read() == 0;
                if let Some(header) = self.byte_chunks.try_read(FileHeader::SIZE) {
                    re_log::trace!(?header, "Decoding StreamHeader");

                    // header contains version and compression options
                    let (version, options) = match options_from_bytes(header) {
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
                if let Some(bytes) = self
                    .byte_chunks
                    .try_read(crate::codec::file::MessageHeader::SIZE_BYTES)
                {
                    let header = crate::codec::file::MessageHeader::from_bytes(bytes)?;

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

                    let message = match crate::codec::file::decoder::decode_bytes_to_app(
                        &mut self.app_id_cache,
                        header.kind,
                        bytes,
                    ) {
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
                        re_log::trace!(
                            "LogMsg::{}",
                            match message {
                                LogMsg::SetStoreInfo { .. } => "SetStoreInfo",
                                LogMsg::ArrowMsg { .. } => "ArrowMsg",
                                LogMsg::BlueprintActivationCommand { .. } => {
                                    "BlueprintActivationCommand"
                                }
                            }
                        );

                        propagate_version(&mut message, self.version);
                        self.state = State::MessageHeader;
                        return Ok(Some(message));
                    } else {
                        re_log::trace!("End of stream - expecting a new Streamheader");

                        // `None` means end of stream, but there might be concatenated streams,
                        // so try to read another one.
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

fn propagate_version(message: &mut LogMsg, version: Option<CrateVersion>) {
    if let re_log_types::LogMsg::SetStoreInfo(msg) = message {
        // Propagate the protocol version from the header into the `StoreInfo` so that all
        // parts of the app can easily access it.
        msg.info.store_version = version;
    }
}

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
}

fn is_byte_chunk_empty(byte_chunk: &ByteChunk) -> bool {
    byte_chunk.position() >= byte_chunk.get_ref().len() as u64
}

#[cfg(test)]
mod tests {
    use re_chunk::RowId;
    use re_log_types::{SetStoreInfo, StoreInfo};

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

        let mut decoder = StreamDecoder::new();

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

        let mut decoder = StreamDecoder::new();

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

        let mut decoder = StreamDecoder::new();

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

        let mut decoder = StreamDecoder::new();

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

        let mut decoder = StreamDecoder::new();

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

        let mut decoder = StreamDecoder::new();
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

        let mut decoder = StreamDecoder::new();
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
    use re_log_types::{SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};
    use re_protos::log_msg::v1alpha1 as proto;
    use re_protos::log_msg::v1alpha1::LogMsg as LogMsgProto;

    use crate::Compression;
    use crate::Encoder;
    use crate::codec::arrow::encode_arrow;

    use super::*;

    fn decoder_into_iter(mut decoder: StreamDecoder) -> impl Iterator<Item = LogMsg> {
        std::iter::from_fn(move || {
            let msg = decoder.try_read().unwrap();

            if msg.is_none() && decoder.state == State::StreamHeader {
                // We're _really_ done, we're not just filtering out some message lacking an app ID.
                return None;
            }

            Some(msg)
        })
        .flatten()
    }

    pub fn fake_log_messages() -> Vec<LogMsg> {
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
            .map(log_msg_to_proto)
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

    fn log_msg_to_proto(message: LogMsg) -> LogMsgProto {
        use re_protos::log_msg::v1alpha1::{
            ArrowMsg, BlueprintActivationCommand, Encoding, SetStoreInfo,
        };

        let msg: proto::log_msg::Msg = match message {
            LogMsg::SetStoreInfo(set_store_info) => {
                let set_store_info: SetStoreInfo = set_store_info.clone().into();
                proto::log_msg::Msg::SetStoreInfo(set_store_info)
            }
            LogMsg::ArrowMsg(store_id, in_arrow_msg) => {
                let re_log_types::ArrowMsg {
                    chunk_id,
                    batch,
                    on_release: _,
                } = &in_arrow_msg;

                let payload =
                    encode_arrow(batch, Compression::Off).expect("compression should succeed");

                let arrow_msg = ArrowMsg {
                    store_id: Some(store_id.clone().into()),
                    chunk_id: Some((*chunk_id).into()),
                    compression: proto::Compression::None as i32,
                    uncompressed_size: payload.uncompressed_size as i32,
                    encoding: Encoding::ArrowIpc as i32,
                    payload: payload.data.into(),
                    is_static: re_sorbet::is_static_chunk(batch),
                };

                proto::log_msg::Msg::ArrowMsg(arrow_msg)
            }
            LogMsg::BlueprintActivationCommand(blueprint_activation_command) => {
                let blueprint_activation_command: BlueprintActivationCommand =
                    blueprint_activation_command.clone().into();

                proto::log_msg::Msg::BlueprintActivationCommand(blueprint_activation_command)
            }
        };

        LogMsgProto { msg: Some(msg) }
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

        for options in options {
            let mut file = vec![];
            crate::Encoder::encode_into(rrd_version, options, messages.iter().map(Ok), &mut file)
                .unwrap();

            let mut decoder = StreamDecoder::new();
            decoder.push_byte_chunk(file);

            let decoded_messages: Vec<_> = decoder_into_iter(decoder).collect();
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

            let mut decoder = StreamDecoder::new();
            decoder.push_byte_chunk(file);

            let decoded_messages: Vec<_> = decoder_into_iter(decoder).collect();
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

            let mut decoder = StreamDecoder::new();
            decoder.push_byte_chunk(file);

            let decoded_messages: Vec<_> = decoder_into_iter(decoder).collect();
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

            let mut decoder = StreamDecoder::new();
            decoder.push_byte_chunk(file);

            let decoded_messages: Vec<_> = decoder_into_iter(decoder).collect();
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

            // write "2 files" i.e. 2 streams that end with end-of-stream marker
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

            let mut decoder = StreamDecoder::new();
            decoder.push_byte_chunk(data);

            let decoded_messages: Vec<_> = decoder_into_iter(decoder).collect();
            similar_asserts::assert_eq!(decoded_messages, [messages.clone(), messages].concat());
        }
    }
}
