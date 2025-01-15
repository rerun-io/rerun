use std::pin::Pin;

use re_build_info::CrateVersion;
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
    /// The size in bytes of the data that has been decoded up to now.
    size_bytes: u64,
}

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
            size_bytes: 0,
        })
    }

    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    fn peek_file_header() -> bool {
        // TODO(zehiko) support concatenated files
        false
    }
}

impl<R: AsyncBufRead + Unpin> Stream for StreamingDecoder<R> {
    type Item = Result<LogMsg, DecodeError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            let serializer = self.options.serializer;
            let compression = self.options.compression;

            let buf = match Pin::new(&mut self.reader).poll_fill_buf(cx) {
                std::task::Poll::Ready(Ok(buf)) => buf,
                std::task::Poll::Ready(Err(err)) => {
                    return std::task::Poll::Ready(Some(Err(DecodeError::Read(err))));
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            };

            if buf.is_empty() {
                return std::task::Poll::Ready(None);
            }

            if Self::peek_file_header() {
                // We've found another file header in the middle of the stream, it's time to switch
                // gears and start over on this new file.

                if buf.len() < FileHeader::SIZE {
                    // Not enough data yet, need to wait for more
                    return std::task::Poll::Pending;
                }

                let data = &buf[..FileHeader::SIZE];
                match read_options(VersionPolicy::Warn, data) {
                    Ok((version, options)) => {
                        self.version = CrateVersion::max(self.version, version);
                        self.options = options;
                        self.size_bytes += FileHeader::SIZE as u64;

                        // Consume the bytes we've processed
                        Pin::new(&mut self.reader).consume(FileHeader::SIZE);

                        // Continue the loop to process more data
                        continue;
                    }
                    Err(err) => return std::task::Poll::Ready(Some(Err(err))),
                }
            }

            let msg = match serializer {
                crate::Serializer::MsgPack => {
                    let header_size = super::MessageHeader::SIZE;
                    if buf.len() < header_size {
                        // Not enough data to read the header, need to wait for more
                        return std::task::Poll::Pending;
                    }

                    // decode the header
                    let data = &buf[..header_size];
                    let header = super::MessageHeader::from_bytes(data);

                    match header {
                        super::MessageHeader::Data {
                            compressed_len,
                            uncompressed_len,
                        } => {
                            let uncompressed_len = uncompressed_len as usize;
                            let compressed_len = compressed_len as usize;
                            let mut uncompressed_data = vec![0_u8; uncompressed_len];

                            // read the data
                            let (data, length) = match compression {
                                Compression::Off => {
                                    if buf.len() < header_size + uncompressed_len {
                                        // Not enough data to read the message, need to wait for more
                                        return std::task::Poll::Pending;
                                    }

                                    (
                                        &buf[header_size..header_size + uncompressed_len],
                                        uncompressed_len,
                                    )
                                }

                                Compression::LZ4 => {
                                    if buf.len() < header_size + compressed_len {
                                        // Not enough data to read the message, need to wait for more
                                        return std::task::Poll::Pending;
                                    }

                                    let data = &buf[header_size..header_size + compressed_len];
                                    if let Err(err) = lz4_flex::block::decompress_into(
                                        data,
                                        &mut uncompressed_data,
                                    ) {
                                        return std::task::Poll::Ready(Some(Err(
                                            DecodeError::Lz4(err),
                                        )));
                                    }

                                    (&uncompressed_data[..], compressed_len)
                                }
                            };

                            // decode the message
                            let msg = rmp_serde::from_slice::<LogMsg>(data);

                            let read_bytes = header_size + length;
                            self.size_bytes += read_bytes as u64;
                            // As a message was fully read, consume the bytes we've processed
                            Pin::new(&mut self.reader).consume(read_bytes);

                            match msg {
                                Ok(msg) => Some(msg),
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
                        // Not enough data to read the header, need to wait for more
                        return std::task::Poll::Pending;
                    }

                    // decode the header
                    let data = &buf[..header_size];
                    let header = file::MessageHeader::decode_bytes(data)?;

                    if buf.len() < header_size + header.len as usize {
                        // Not enough data to read the message, need to wait for more
                        return std::task::Poll::Pending;
                    }

                    // decode the message
                    let data = &buf[header_size..header.len as usize + header_size];
                    let msg = file::decoder::bytes_to_message(header.kind, data)?;

                    let read_bytes = header_size as u64 + header.len;
                    self.size_bytes += read_bytes;

                    // As a message was fully read, consume the bytes we've processed
                    Pin::new(&mut self.reader).consume(read_bytes as usize);

                    msg
                }
            };

            let Some(mut msg) = msg else {
                // TODO(zehiko) support concatenated files
                if Self::peek_file_header() {
                    re_log::debug!(
                            "Reached end of stream, but it seems we have a concatenated file, continuing"
                        );
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

            return std::task::Poll::Ready(Some(Ok(msg)));
        }
    }
}

#[cfg(test)]
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
    async fn test_streaming_decoder() {
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
