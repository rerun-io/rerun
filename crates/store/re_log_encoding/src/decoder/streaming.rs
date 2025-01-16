use std::pin::Pin;

use re_build_info::CrateVersion;
use re_log::external::log::warn;
use re_log_types::LogMsg;
use tokio::io::{AsyncBufRead, AsyncReadExt};
use tokio_stream::Stream;

use crate::{
    codec::file::{self},
    Compression, EncodingOptions, VersionPolicy,
};

use super::{read_options, DecodeError, FileHeader};

pub struct StreamingDecoder<R: AsyncBufRead> {
    version: CrateVersion,
    options: EncodingOptions,
    reader: R,
    // buffer used for uncompressing data. This is a tiny optimization
    // to (potentially) avoid allocation for each (compressed) message
    uncompressed: Vec<u8>,
    // there are some interesting cases (like corrupted files or concatanated files) where we might
    // need to know how much unprocessed bytes we have left from the last read
    bytes_read: usize,
}

/// `StreamingDecoder` relies on the underlying reader for the wakeup mechanism.
impl<R: AsyncBufRead + Unpin> StreamingDecoder<R> {
    pub async fn new(version_policy: VersionPolicy, mut reader: R) -> Result<Self, DecodeError> {
        let mut data = [0_u8; FileHeader::SIZE];

        reader
            .read_exact(&mut data)
            .await
            .map_err(DecodeError::Read)?;

        let (version, options) = read_options(version_policy, &data)?;

        Ok(Self {
            version,
            options,
            reader,
            uncompressed: Vec::new(),
            bytes_read: 0,
        })
    }

    fn peek_file_header(data: &[u8]) -> bool {
        let mut read = std::io::Cursor::new(data);
        FileHeader::decode(&mut read).is_ok()
    }
}

impl<R: AsyncBufRead + Unpin> Stream for StreamingDecoder<R> {
    type Item = Result<LogMsg, DecodeError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            let Self {
                options,
                reader,
                uncompressed,
                bytes_read,
                ..
            } = &mut *self;

            let serializer = options.serializer;
            let compression = options.compression;

            // poll_fill_buf() implicitly handles the EOF case, so we don't need to check for it
            let buf = match Pin::new(reader).poll_fill_buf(cx) {
                std::task::Poll::Ready(Ok([])) => return std::task::Poll::Ready(None),
                std::task::Poll::Ready(Ok(buf)) => buf,
                std::task::Poll::Ready(Err(err)) => {
                    return std::task::Poll::Ready(Some(Err(DecodeError::Read(err))));
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            };

            // no new data to read, but still some unprocessed bytes left
            // this can happen in the case of a corrupted file
            if *bytes_read == buf.len() {
                warn!("we have more bytes in the stream but not enough to process a message");
                return std::task::Poll::Ready(None);
            }

            // check if this is a start of a new concatenated file
            if buf.len() >= FileHeader::SIZE && Self::peek_file_header(&buf[..FileHeader::SIZE]) {
                let data = &buf[..FileHeader::SIZE];
                // We've found another file header in the middle of the stream, it's time to switch
                // gears and start over on this new file.
                match read_options(VersionPolicy::Warn, data) {
                    Ok((version, options)) => {
                        self.version = CrateVersion::max(self.version, version);
                        self.options = options;

                        // Consume the bytes we've processed
                        Pin::new(&mut self.reader).consume(FileHeader::SIZE);
                        self.bytes_read = 0;

                        // Continue the loop to process more data
                        continue;
                    }
                    Err(err) => return std::task::Poll::Ready(Some(Err(err))),
                }
            }

            let (msg, read_bytes) = match serializer {
                crate::Serializer::MsgPack => {
                    let header_size = super::MessageHeader::SIZE;

                    if buf.len() < header_size {
                        self.bytes_read = buf.len();
                        // Not enough data to read the message, need to wait for more
                        continue;
                    }
                    let data = &buf[..header_size];
                    let header = super::MessageHeader::from_bytes(data);

                    match header {
                        super::MessageHeader::Data {
                            compressed_len,
                            uncompressed_len,
                        } => {
                            let uncompressed_len = uncompressed_len as usize;
                            let compressed_len = compressed_len as usize;
                            uncompressed.resize(uncompressed.len().max(uncompressed_len), 0);

                            // read the data
                            let (data, length) = match compression {
                                Compression::Off => {
                                    if buf.len() < header_size + uncompressed_len {
                                        self.bytes_read = buf.len();
                                        // Not enough data to read the message, need to wait for more
                                        continue;
                                    }

                                    (
                                        &buf[header_size..header_size + uncompressed_len],
                                        uncompressed_len,
                                    )
                                }

                                Compression::LZ4 => {
                                    if buf.len() < header_size + compressed_len {
                                        self.bytes_read = buf.len();
                                        // Not enough data to read the message, need to wait for more
                                        continue;
                                    }

                                    let data = &buf[header_size..header_size + compressed_len];
                                    if let Err(err) =
                                        lz4_flex::block::decompress_into(data, uncompressed)
                                    {
                                        return std::task::Poll::Ready(Some(Err(
                                            DecodeError::Lz4(err),
                                        )));
                                    }

                                    (&uncompressed[..], compressed_len)
                                }
                            };

                            // decode the message
                            let msg = rmp_serde::from_slice::<LogMsg>(data);
                            let read_bytes = header_size + length;

                            match msg {
                                Ok(msg) => (Some(msg), read_bytes),
                                Err(err) => {
                                    return std::task::Poll::Ready(Some(Err(
                                        DecodeError::MsgPack(err),
                                    )));
                                }
                            }
                        }
                        super::MessageHeader::EndOfStream => return std::task::Poll::Ready(None),
                    }
                }
                crate::Serializer::Protobuf => {
                    let header_size = std::mem::size_of::<file::MessageHeader>();

                    if buf.len() < header_size {
                        self.bytes_read = buf.len();
                        // Not enough data to read the message, need to wait for more
                        continue;
                    }
                    let data = &buf[..header_size];
                    let header = file::MessageHeader::from_bytes(data)?;

                    if buf.len() < header_size + header.len as usize {
                        self.bytes_read = buf.len();
                        // Not enough data to read the message, need to wait for more
                        continue;
                    }

                    // decode the message
                    let data = &buf[header_size..header.len as usize + header_size];
                    let msg = file::decoder::decode_bytes(header.kind, data)?;

                    let read_bytes = header_size + header.len as usize;

                    (msg, read_bytes)
                }
            };

            // when is msg None? when we've reached the end of the stream
            let Some(mut msg) = msg else {
                // check if there's another file concatenated
                if buf.len() < read_bytes + FileHeader::SIZE {
                    return std::task::Poll::Ready(None);
                }

                let data = &buf[read_bytes..read_bytes + FileHeader::SIZE];
                if Self::peek_file_header(data) {
                    re_log::debug!(
                            "Reached end of stream, but it seems we have a concatenated file, continuing"
                        );

                    // Consume the bytes we've processed
                    Pin::new(&mut self.reader).consume(read_bytes);
                    self.bytes_read = 0;

                    continue;
                }

                re_log::debug!("Reached end of stream, iterator complete");
                return std::task::Poll::Ready(None);
            };

            if let LogMsg::SetStoreInfo(msg) = &mut msg {
                // Propagate the protocol version from the header into the `StoreInfo` so that all
                // parts of the app can easily access it.
                msg.info.store_version = Some(self.version);
            }

            // Consume the bytes we've processed
            Pin::new(&mut self.reader).consume(read_bytes);
            self.bytes_read = 0;

            return std::task::Poll::Ready(Some(Ok(msg)));
        }
    }
}

#[cfg(all(test, feature = "decoder", feature = "encoder"))]
mod tests {
    use re_build_info::CrateVersion;
    use tokio_stream::StreamExt;

    use crate::{
        decoder::{
            streaming::StreamingDecoder,
            tests::{fake_log_messages, strip_arrow_extensions_from_log_messages},
        },
        Compression, EncodingOptions, Serializer, VersionPolicy,
    };

    #[tokio::test]
    async fn test_streaming_decoder_handles_corrupted_input_file() {
        let rrd_version = CrateVersion::LOCAL;

        let messages = fake_log_messages();

        let options = [
            EncodingOptions {
                compression: Compression::Off,
                serializer: Serializer::MsgPack,
            },
            EncodingOptions {
                compression: Compression::LZ4,
                serializer: Serializer::MsgPack,
            },
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

            let decoder = StreamingDecoder::new(VersionPolicy::Error, buf_reader)
                .await
                .unwrap();

            let decoded_messages = strip_arrow_extensions_from_log_messages(
                decoder.collect::<Result<Vec<_>, _>>().await.unwrap(),
            );

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
                serializer: Serializer::MsgPack,
            },
            EncodingOptions {
                compression: Compression::LZ4,
                serializer: Serializer::MsgPack,
            },
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

            let decoder = StreamingDecoder::new(VersionPolicy::Error, buf_reader)
                .await
                .unwrap();

            let decoded_messages = strip_arrow_extensions_from_log_messages(
                decoder.collect::<Result<Vec<_>, _>>().await.unwrap(),
            );

            similar_asserts::assert_eq!(decoded_messages, messages);
        }
    }
}
