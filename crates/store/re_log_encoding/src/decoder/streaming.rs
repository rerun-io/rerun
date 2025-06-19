use std::pin::Pin;

use bytes::{Buf as _, Bytes, BytesMut};
use tokio::io::{AsyncBufRead, AsyncReadExt as _};
use tokio_stream::Stream;

use re_build_info::CrateVersion;
use re_log::external::log::warn;

use crate::{
    EncodingOptions,
    codec::file::{self, MessageKind},
};

use super::{DecodeError, FileHeader, options_from_bytes};

/// A [`LogMsg`] with extra contextual information.
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

    /// How many bytes does one have to go through in the underlying storage resource in order to
    /// find the start of this message?
    ///
    /// Specifically, this points to the beginning of the [`MessageHeader`].
    pub byte_offset: u64,

    /// How many bytes does this message take in the underlying storage resource?
    ///
    /// This covers both the size of the message's header _and_ its body.
    pub byte_len: u64,
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
            byte_offset: 0,
            byte_len,
        })
    }

    /// Returns the decoded `LogMsg`.
    ///
    /// This will only decode at the transport-layer (Protobuf), Arrow data is left untouched.
    ///
    /// Compute cost:
    /// * If [`StreamingDecoderOptions::keep_decoded_protobuf`] was set, this is free.
    /// * Otherwise, if [`StreamingDecoderOptions::keep_encoded_protobuf`] was set, this has to
    ///   perform Protobuf decoding.
    /// * Otherwise, if neither are set, this just returns `None`.
    pub fn decoded(
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
            unprocessed_bytes: BytesMut::new(),
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
            unprocessed_bytes: BytesMut::new(),
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
/// The fact that we can have concatanated file or corrupted file ( / input stream) pushes us to keep
/// the state of the decoder in the struct itself (through `unprocessed_bytes` and `expect_more_data`).
impl<R: AsyncBufRead + Unpin> Stream for StreamingDecoder<R> {
    type Item = Result<StreamingLogMsg, DecodeError>;

    #[tracing::instrument(name = "streaming_decoder", level = "trace", skip_all)]
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
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
            };

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

                        Pin::new(&mut self.reader).consume(buf_length);
                        self.unprocessed_bytes.advance(FileHeader::SIZE);
                        self.num_bytes_read += FileHeader::SIZE as u64;

                        continue;
                    }
                    Err(err) => return std::task::Poll::Ready(Some(Err(err))),
                }
            }

            let (kind, encoded, processed_length) = match serializer {
                crate::Serializer::Protobuf => {
                    let header_size = std::mem::size_of::<file::MessageHeader>();
                    if unprocessed_bytes.len() < header_size {
                        // Not enough data to read the header, need to wait for more
                        self.expect_more_data = true;
                        Pin::new(&mut self.reader).consume(buf_length);

                        continue;
                    }
                    let data = &unprocessed_bytes[..header_size];
                    let header = file::MessageHeader::from_bytes(data)?;

                    if unprocessed_bytes.len() < header.len as usize + header_size {
                        // Not enough data to read the message, need to wait for more
                        self.expect_more_data = true;
                        Pin::new(&mut self.reader).consume(buf_length);

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

                    Pin::new(&mut self.reader).consume(buf_length);
                    continue;
                }

                re_log::trace!("Reached end of stream, iterator complete");
                return std::task::Poll::Ready(None);
            };

            let decoded = if opts.keep_decoded_protobuf {
                file::decoder::decode_bytes_to_transport(kind, encoded)?
            } else {
                None
            };
            let encoded: Option<Bytes> =
                opts.keep_encoded_protobuf.then(|| encoded.to_vec().into());
            let version = self.version;

            Pin::new(&mut self.reader).consume(buf_length);
            self.unprocessed_bytes.advance(processed_length);
            self.expect_more_data = false;

            let msg = StreamingLogMsg {
                kind,
                version,
                encoded,
                decoded,
                byte_offset: self.num_bytes_read,
                byte_len: processed_length as _,
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
            crate::encoder::encode_ref(rrd_version, options, messages.iter().map(Ok), &mut data)
                .unwrap();

            // We cut the input file by one byte to simulate a corrupted file and check that we don't end up in an infinite loop
            // waiting for more data when there's none to be read.
            let data = &data[..data.len() - 1];

            let buf_reader = tokio::io::BufReader::new(std::io::Cursor::new(data));

            let decoder = StreamingDecoder::new(StreamingDecoderOptions::ALL, buf_reader)
                .await
                .unwrap();

            let decoded_messages: Vec<re_log_types::LogMsg> = decoder
                .map(Result::unwrap)
                .filter_map(|msg| msg.decoded().unwrap())
                .map(|msg| file::decoder::decode_transport_to_app(msg).unwrap())
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
            crate::encoder::encode_ref(rrd_version, options, messages.iter().map(Ok), &mut data)
                .unwrap();

            let buf_reader = tokio::io::BufReader::new(std::io::Cursor::new(data));

            let decoder = StreamingDecoder::new(StreamingDecoderOptions::ALL, buf_reader)
                .await
                .unwrap();

            let decoded_messages = decoder
                .map(Result::unwrap)
                .filter_map(|msg| msg.decoded().unwrap())
                .map(|msg| file::decoder::decode_transport_to_app(msg).unwrap())
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
            crate::encoder::encode_ref(rrd_version, options, messages.iter().map(Ok), &mut data)
                .unwrap();

            let buf_reader = tokio::io::BufReader::new(std::io::Cursor::new(data.clone()));

            let decoder = StreamingDecoder::new(StreamingDecoderOptions::ALL, buf_reader)
                .await
                .unwrap();

            let mut decoded_messages = decoder.collect::<Result<Vec<_>, _>>().await.unwrap();
            for msg_expected in &mut decoded_messages {
                let (offset, len) = (
                    msg_expected.byte_offset as usize,
                    msg_expected.byte_len as usize,
                );
                let data = &data[offset..offset + len];

                {
                    use crate::codec::file;

                    let header_size = std::mem::size_of::<file::MessageHeader>();
                    let header_data = &data[..header_size];
                    let header = file::MessageHeader::from_bytes(header_data).unwrap();

                    let data = &data[header_size..];
                    let msg = file::decoder::decode_bytes_to_app(header.kind, data)
                        .unwrap()
                        .unwrap();

                    let msg_expected = file::decoder::decode_transport_to_app(
                        msg_expected.decoded().unwrap().unwrap(),
                    )
                    .unwrap();
                    similar_asserts::assert_eq!(msg_expected, msg);
                }
            }

            let decoded_messages = decoded_messages
                .iter_mut()
                .filter_map(|msg| msg.decoded().unwrap())
                .map(|msg| file::decoder::decode_transport_to_app(msg).unwrap())
                .collect::<Vec<_>>();

            similar_asserts::assert_eq!(decoded_messages, messages);
        }
    }
}
