use std::pin::Pin;

use re_build_info::CrateVersion;
use re_log_types::LogMsg;
use tokio::io::{AsyncBufRead, AsyncReadExt};
use tokio_stream::Stream;

use crate::{
    codec::file::{self, MessageHeader},
    EncodingOptions, VersionPolicy,
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
            match Pin::new(&mut self.reader).poll_fill_buf(cx) {
                std::task::Poll::Ready(Ok(buf)) => {
                    if buf.is_empty() {
                        return std::task::Poll::Ready(None);
                    } else {
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

                        let header_size = std::mem::size_of::<MessageHeader>();
                        if buf.len() < header_size {
                            // Not enough data to read the header, need to wait for more
                            return std::task::Poll::Pending;
                        }

                        // decode the header
                        let data = &buf[..header_size];
                        let header = file::MessageHeader::bytes_to_header(data)?;

                        if buf.len() < header.len as usize {
                            // Not enough data to read the message, need to wait for more
                            return std::task::Poll::Pending;
                        }

                        // decode the message
                        let data = &buf[..header_size];
                        let msg = file::decoder::bytes_to_message(header.kind, data)?;

                        let read_bytes = header_size as u64 + header.len;
                        self.size_bytes += read_bytes;

                        // As a message was fully read, consume the bytes we've processed
                        Pin::new(&mut self.reader).consume(read_bytes as usize);

                        if let Some(mut msg) = msg {
                            if let LogMsg::SetStoreInfo(msg) = &mut msg {
                                // Propagate the protocol version from the header into the `StoreInfo` so that all
                                // parts of the app can easily access it.
                                msg.info.store_version = Some(self.version);
                            }

                            return std::task::Poll::Ready(Some(Ok(msg)));
                        } else {
                            // TODO(zehiko) support concatanated files
                            return std::task::Poll::Ready(None);
                        }
                    }
                }
                std::task::Poll::Ready(Err(err)) => {
                    return std::task::Poll::Ready(Some(Err(DecodeError::Read(err))));
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}
