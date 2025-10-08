use std::pin::Pin;

use bytes::{Buf as _, Bytes, BytesMut};
use tokio::io::{AsyncBufRead, AsyncReadExt as _};
use tokio_stream::Stream;

use re_build_info::CrateVersion;
use re_chunk::Span;
use re_log::external::log::warn;

use crate::{
    EncodingOptions,
    codec::file::{self, MessageKind},
};

use super::{DecodeError, FileHeader, options_from_bytes};

/// A transport-level `LogMsg` with extra contextual information.
///
/// Yielded by the [`StreamingDecoder`].
#[derive(Debug, Clone)]
pub struct StreamingLogMsg {
    pub kind: MessageKind,

    /// The [`CrateVersion`] of the RRD stream from which this `LogMsg` was taken from.
    pub version: CrateVersion,

    /// The original Protobuf-encoded bytes of this `LogMsg`, *including headers*.
    ///
    /// Only set if [`StreamingDecoderOptions::keep_encoded_protobuf`] is true.
    pub encoded: Option<bytes::Bytes>,

    /// The decoded Protobuf `LogMsg`.
    ///
    /// Arrow data contained within is never decoded.
    ///
    /// Only set if [`StreamingDecoderOptions::keep_decoded_protobuf`] is true.
    decoded: Option<re_protos::log_msg::v1alpha1::log_msg::Msg>,

    /// Where in the underlying storage resource is this message (in bytes)?
    ///
    /// Specifically, the start of this range points to the beginning of the `MessageHeader`.
    ///
    /// The full range covers both the message's header _and_ its body.
    pub byte_span: Span<u64>,
}

impl StreamingLogMsg {
    /// Create a new [`StreamingLogMsg`] from a [`Chunk`].
    ///
    /// This is only really useful for testing purposes, and makes no effort at all to be efficient.
    ///
    /// [`Chunk`]: [`re_chunk::Chunk`]
    #[cfg(feature = "encoder")]
    pub fn from_chunk(
        store_id: re_log_types::StoreId,
        chunk: &re_chunk::Chunk,
    ) -> Result<Self, crate::encoder::EncodeError> {
        let compression = crate::Compression::Off;

        let arrow_msg = re_log_types::ArrowMsg {
            chunk_id: *chunk.id(),
            batch: chunk.to_record_batch()?,
            on_release: None,
        };

        let log_msg = re_log_types::LogMsg::ArrowMsg(store_id, arrow_msg);
        let log_msg_proto =
            crate::protobuf_conversions::log_msg_to_proto(log_msg.clone(), compression)?;

        let mut log_msg_encoded = Vec::new();
        crate::codec::file::encoder::encode(&mut log_msg_encoded, &log_msg, compression)?;

        let byte_len = log_msg_encoded.len() as _;

        Ok(Self {
            kind: MessageKind::ArrowMsg,
            version: CrateVersion::LOCAL,
            encoded: Some(log_msg_encoded.into()),
            decoded: log_msg_proto.msg,
            byte_span: Span {
                start: 0,
                len: byte_len,
            },
        })
    }

    /// Returns the decoded transport-level `LogMsg`.
    ///
    /// This will only decode at the transport-layer (Protobuf), Arrow data is left untouched.
    ///
    /// Compute cost:
    /// * If [`StreamingDecoderOptions::keep_decoded_protobuf`] was set on the [`StreamingDecoder`]
    ///   when producing the `StreamingLogMsg`, this is free.
    /// * Otherwise, if [`StreamingDecoderOptions::keep_encoded_protobuf`] was set, this has to
    ///   perform Protobuf decoding.
    /// * Otherwise, if neither are set, this just returns `None`.
    pub fn decoded_transport(
        &self,
    ) -> Result<Option<re_protos::log_msg::v1alpha1::log_msg::Msg>, DecodeError> {
        if let Some(mut decoded) = self.decoded.clone() {
            if let re_protos::log_msg::v1alpha1::log_msg::Msg::SetStoreInfo(msg) = &mut decoded {
                // Propagate the protocol version from the header into the `StoreInfo` so that all
                // parts of the app can easily access it.
                if let Some(info) = msg.info.as_mut() {
                    info.store_version = Some(re_protos::log_msg::v1alpha1::StoreVersion {
                        crate_version_bits: i32::from_le_bytes(self.version.to_bytes()),
                    });
                }
            }

            return Ok(Some(decoded.clone()));
        }

        if let Some(encoded) = self.encoded.as_ref() {
            return file::decoder::decode_bytes_to_transport(self.kind, encoded);
        }

        Ok(None)
    }
}

pub struct StreamingDecoder<R: AsyncBufRead> {
    version: CrateVersion,
    opts: StreamingDecoderOptions,
    encoding_opts: EncodingOptions,
    reader: R,

    /// internal buffer for unprocessed bytes
    unprocessed_bytes: BytesMut,

    /// flag to indicate if we're expecting more data to be read.
    expect_more_data: bool,

    // total number of bytes read until now
    num_bytes_read: u64,
}

#[derive(Default, Clone)]
pub struct StreamingDecoderOptions {
    pub keep_encoded_protobuf: bool,
    pub keep_decoded_protobuf: bool,
}

impl StreamingDecoderOptions {
    pub const NONE: Self = Self {
        keep_encoded_protobuf: false,
        keep_decoded_protobuf: false,
    };

    pub const ALL: Self = Self {
        keep_encoded_protobuf: true,
        keep_decoded_protobuf: true,
    };

    pub const ENCODED: Self = Self {
        keep_encoded_protobuf: true,
        keep_decoded_protobuf: false,
    };

    pub const DECODED: Self = Self {
        keep_encoded_protobuf: false,
        keep_decoded_protobuf: true,
    };
}

impl<R: AsyncBufRead + Unpin> StreamingDecoder<R> {
    pub async fn new(opts: StreamingDecoderOptions, mut reader: R) -> Result<Self, DecodeError> {
        let mut data = [0_u8; FileHeader::SIZE];

        reader
            .read_exact(&mut data)
            .await
            .map_err(DecodeError::Read)?;

        let (version, encoding_opts) = options_from_bytes(&data)?;

        Ok(Self {
            version,
            opts,
            encoding_opts,
            reader,
            unprocessed_bytes: BytesMut::default(),
            expect_more_data: false,
            num_bytes_read: FileHeader::SIZE as _,
        })
    }

    pub fn new_with_options(
        version: CrateVersion,
        opts: StreamingDecoderOptions,
        encoding_opts: EncodingOptions,
        reader: R,
    ) -> Self {
        Self {
            version,
            opts,
            encoding_opts,
            reader,
            unprocessed_bytes: BytesMut::default(),
            expect_more_data: false,
            num_bytes_read: FileHeader::SIZE as _,
        }
    }

    /// Returns true if `data` can be successfully decoded into a `FileHeader`.
    fn peek_file_header(data: &[u8]) -> bool {
        let mut read = std::io::Cursor::new(data);
        FileHeader::decode(&mut read).is_ok()
    }
}

/// `StreamingDecoder` relies on the underlying reader for the wakeup mechanism.
/// The fact that we can have concatenated file or corrupted file ( / input stream) pushes us to keep
/// the state of the decoder in the struct itself (through `unprocessed_bytes` and `expect_more_data`).
impl<R: AsyncBufRead + Unpin> Stream for StreamingDecoder<R> {
    type Item = Result<StreamingLogMsg, DecodeError>;

    #[tracing::instrument(name = "streaming_decoder", level = "trace", skip_all)]
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // It is super important that we don't needlessly poll on the underlying async reader.
        //
        // In most real-world scenarios, the underlying async reader will be some kind of object
        // store client, which generally tries to fetch 1MiB ranges on every poll.
        //
        // 1MiB is the upper-boundary that we target for the size of a single, perfectly compacted chunk.
        // In practice, chunks very rarely come anywhere close to that size, so you can expect
        // anywhere between 10 to 100 chunks to fit in that 1MiB.
        // If we were to blindly poll on the underlying reader every time we poll on the decoder,
        // we would therefore be accumulating unprocessed data way faster than we can yield chunks
        // (since by definition we can only yield a single chunk per call to `poll_next`).
        //
        // Normally this would just be somewhat bad: the `unprocessed_bytes` buffer would grow
        // unnecessarily large and we'd be sad for it.
        // In practice, it's much worse than that because it prevents some very specific
        // optimizations from the `bytes` crate from triggering. Specifically, the `bytes` crate has a
        // bunch of heuristics that will try and re-use buffer space, even going through the motions of
        // shifting/copying the data if it's deemed worth it, when `extend()` and `advance()` are interleaved
        // repeatedly (like we do here). These heuristics are based on the difference between the
        // head capacity of the internal buffer versus the current position of the cursor.
        //
        // When polling too aggressively, this internal head capacity and the internal cursor end up
        // completely out of sync, and these heuristics are never hit.
        // The end result is an internal buffer that grows indefinitely large, instead of consistently
        // maintaining a size that is a single-digit factor away from the size of the biggest chunk
        // in the stream (or 1MiB).
        //
        // ---
        //
        //     let mut buf = BytesMut::new();
        //     for _ in 0..1_000_000 {
        //         buf.extend_from_slice(&[0; 100][..]);
        //     }
        //     eprintln!("cap={} len={} mem={}", buf.capacity(), buf.len(), re_memory::MemoryUse::capture().counted);
        //
        // Outputs: cap=104,857,600 len=100,000,000 mem=104,857,600
        //
        //     let mut buf = BytesMut::new();
        //     for _ in 0..1_000_000 {
        //         buf.extend_from_slice(&[0; 100][..]);
        //         buf.advance(100);
        //     }
        //     eprintln!("cap={} len={} mem={}", buf.capacity(), buf.len(), re_memory::MemoryUse::capture().counted);
        //
        // Outputs: cap=0 len=0 mem=100
        //
        //     let mut buf = BytesMut::new();
        //     for _ in 0..1_000_000 {
        //         buf.extend_from_slice(&[0; 100][..]);
        //         buf.advance(90);
        //     }
        //     eprintln!("cap={} len={} mem={}", buf.capacity(), buf.len(), re_memory::MemoryUse::capture().counted);
        //
        // Outputs: cap=24,056,110 len=10,000,000 mem=26,214,400
        // ```
        let mut should_read_more_data = self.unprocessed_bytes.is_empty();

        loop {
            let Self {
                opts,
                encoding_opts,
                reader,
                unprocessed_bytes,
                expect_more_data,
                ..
            } = &mut *self;

            let serializer = encoding_opts.serializer;
            let mut buf_length = 0;

            fn consume_if_needed<R: AsyncBufRead + Unpin>(reader: &mut R, buf_length: usize) {
                // Many implementers of `AsyncBufRead` panic when trying to poll them unexpectedly
                // (and this can be unexpected in this case because we might have bypassed polling,
                // see `should_read_more_data`).
                if buf_length > 0 {
                    Pin::new(reader).consume(buf_length);
                }
            }

            if should_read_more_data {
                // poll_fill_buf() implicitly handles the EOF case, so we don't need to check for it
                match Pin::new(reader).poll_fill_buf(cx) {
                    std::task::Poll::Ready(Ok([])) => {
                        if unprocessed_bytes.is_empty() {
                            return std::task::Poll::Ready(None);
                        }
                        // there's more unprocessed data, but there's nothing in the underlying
                        // bytes stream - this indicates a corrupted stream
                        if *expect_more_data {
                            warn!(
                                "There's {} unprocessed data, but not enough for decoding a full message",
                                unprocessed_bytes.len()
                            );
                            return std::task::Poll::Ready(None);
                        }
                    }

                    std::task::Poll::Ready(Ok(buf)) => {
                        unprocessed_bytes.extend_from_slice(buf);
                        buf_length = buf.len();
                    }

                    std::task::Poll::Ready(Err(err)) => {
                        return std::task::Poll::Ready(Some(Err(DecodeError::Read(err))));
                    }

                    std::task::Poll::Pending => return std::task::Poll::Pending,
                }
            }

            // Now that we've tried at least once to get a chunk out without reading any data, life
            // can go on as usual.
            should_read_more_data = true;

            // check if this is a start of a new concatenated file
            if unprocessed_bytes.len() >= FileHeader::SIZE
                && Self::peek_file_header(&unprocessed_bytes[..FileHeader::SIZE])
            {
                let data = &unprocessed_bytes[..FileHeader::SIZE];
                // We've found another file header in the middle of the stream, it's time to switch
                // gears and start over on this new file.
                match options_from_bytes(data) {
                    Ok((version, options)) => {
                        self.version = CrateVersion::max(self.version, version);
                        self.encoding_opts = options;

                        consume_if_needed(&mut self.reader, buf_length);
                        self.unprocessed_bytes.advance(FileHeader::SIZE);
                        self.num_bytes_read += FileHeader::SIZE as u64;

                        continue;
                    }
                    Err(err) => {
                        return std::task::Poll::Ready(Some(Err(err)));
                    }
                }
            }

            let (kind, encoded, processed_length) = match serializer {
                crate::Serializer::Protobuf => {
                    let header_size = std::mem::size_of::<file::MessageHeader>();
                    if unprocessed_bytes.len() < header_size {
                        // Not enough data to read the header, need to wait for more
                        self.expect_more_data = true;
                        consume_if_needed(&mut self.reader, buf_length);

                        continue;
                    }
                    let data = &unprocessed_bytes[..header_size];
                    let header = file::MessageHeader::from_bytes(data)?;

                    if unprocessed_bytes.len() < header.len as usize + header_size {
                        // Not enough data to read the message, need to wait for more
                        self.expect_more_data = true;
                        consume_if_needed(&mut self.reader, buf_length);

                        continue;
                    }

                    // decode the message
                    let data = &unprocessed_bytes[header_size..header_size + header.len as usize];

                    (header.kind, data, header.len as usize + header_size)
                }
            };

            if kind == MessageKind::End {
                // we've reached the end of the stream (i.e. read the EoS header), we check if there's another file concatenated
                if unprocessed_bytes.len() < processed_length + FileHeader::SIZE {
                    return std::task::Poll::Ready(None);
                }

                let data =
                    &unprocessed_bytes[processed_length..processed_length + FileHeader::SIZE];
                if Self::peek_file_header(data) {
                    re_log::debug!(
                        "Reached end of stream, but it seems we have a concatenated file, continuing"
                    );

                    consume_if_needed(&mut self.reader, buf_length);
                    continue;
                }

                re_log::trace!("Reached end of stream, iterator complete");
                return std::task::Poll::Ready(None);
            }

            let decoded = if opts.keep_decoded_protobuf {
                file::decoder::decode_bytes_to_transport(kind, encoded)?
            } else {
                None
            };
            let encoded: Option<Bytes> =
                opts.keep_encoded_protobuf.then(|| encoded.to_vec().into());
            let version = self.version;

            consume_if_needed(&mut self.reader, buf_length);
            self.unprocessed_bytes.advance(processed_length);
            self.expect_more_data = false;

            let msg = StreamingLogMsg {
                kind,
                version,
                encoded,
                decoded,
                byte_span: Span {
                    start: self.num_bytes_read,
                    len: processed_length as _,
                },
            };

            self.num_bytes_read += processed_length as u64;

            return std::task::Poll::Ready(Some(Ok(msg)));
        }
    }
}

#[cfg(feature = "testing")]
#[cfg(all(test, feature = "decoder", feature = "encoder"))]
mod tests {
    use re_build_info::CrateVersion;
    use tokio_stream::StreamExt as _;

    use crate::{
        Compression, EncodingOptions, Serializer,
        codec::file,
        decoder::{
            streaming::{StreamingDecoder, StreamingDecoderOptions},
            tests::fake_log_messages,
        },
    };

    #[tokio::test]
    async fn test_streaming_decoder_handles_corrupted_input_file() {
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
            let mut data = vec![];
            crate::Encoder::encode_into(rrd_version, options, messages.iter().map(Ok), &mut data)
                .unwrap();

            // We cut the input file by one byte to simulate a corrupted file and check that we don't end up in an infinite loop
            // waiting for more data when there's none to be read.
            let data = &data[..data.len() - 1];

            let buf_reader = tokio::io::BufReader::new(std::io::Cursor::new(data));

            let decoder = StreamingDecoder::new(StreamingDecoderOptions::ALL, buf_reader)
                .await
                .unwrap();

            let mut app_id_cache = crate::app_id_injector::CachingApplicationIdInjector::default();
            let decoded_messages: Vec<re_log_types::LogMsg> = decoder
                .map(Result::unwrap)
                .filter_map(|msg| msg.decoded_transport().unwrap())
                .map(|msg| file::decoder::decode_transport_to_app(&mut app_id_cache, msg).unwrap())
                .collect::<Vec<_>>()
                .await;

            similar_asserts::assert_eq!(decoded_messages, messages);
        }
    }

    #[tokio::test]
    async fn test_streaming_decoder_happy_paths() {
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
            let mut data = vec![];
            crate::Encoder::encode_into(rrd_version, options, messages.iter().map(Ok), &mut data)
                .unwrap();

            let buf_reader = tokio::io::BufReader::new(std::io::Cursor::new(data));

            let decoder = StreamingDecoder::new(StreamingDecoderOptions::ALL, buf_reader)
                .await
                .unwrap();

            let mut app_id_cache = crate::app_id_injector::CachingApplicationIdInjector::default();
            let decoded_messages = decoder
                .map(Result::unwrap)
                .filter_map(|msg| msg.decoded_transport().unwrap())
                .map(|msg| file::decoder::decode_transport_to_app(&mut app_id_cache, msg).unwrap())
                .collect::<Vec<_>>()
                .await;

            similar_asserts::assert_eq!(decoded_messages, messages);
        }
    }

    #[tokio::test]
    async fn test_streaming_decoder_byte_offsets() {
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
            let mut data = vec![];
            crate::Encoder::encode_into(rrd_version, options, messages.iter().map(Ok), &mut data)
                .unwrap();

            let buf_reader = tokio::io::BufReader::new(std::io::Cursor::new(data.clone()));

            let decoder = StreamingDecoder::new(StreamingDecoderOptions::ALL, buf_reader)
                .await
                .unwrap();

            let mut decoded_messages = decoder.collect::<Result<Vec<_>, _>>().await.unwrap();

            let mut app_id_cache = crate::app_id_injector::CachingApplicationIdInjector::default();
            for msg_expected in &mut decoded_messages {
                let data = &data[msg_expected.byte_span.try_cast::<usize>().unwrap().range()];

                {
                    use crate::codec::file;

                    let header_size = std::mem::size_of::<file::MessageHeader>();
                    let header_data = &data[..header_size];
                    let header = file::MessageHeader::from_bytes(header_data).unwrap();

                    let data = &data[header_size..];
                    let msg =
                        file::decoder::decode_bytes_to_app(&mut app_id_cache, header.kind, data)
                            .unwrap()
                            .unwrap();

                    let msg_expected = file::decoder::decode_transport_to_app(
                        &mut app_id_cache,
                        msg_expected.decoded_transport().unwrap().unwrap(),
                    )
                    .unwrap();
                    similar_asserts::assert_eq!(msg_expected, msg);
                }
            }

            let mut app_id_cache = crate::app_id_injector::CachingApplicationIdInjector::default();
            let decoded_messages = decoded_messages
                .iter_mut()
                .filter_map(|msg| msg.decoded_transport().unwrap())
                .map(|msg| file::decoder::decode_transport_to_app(&mut app_id_cache, msg).unwrap())
                .collect::<Vec<_>>();

            similar_asserts::assert_eq!(decoded_messages, messages);
        }
    }
}
