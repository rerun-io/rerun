use std::pin::Pin;

use bytes::{Buf as _, BytesMut};
use re_build_info::CrateVersion;
use re_log::external::log::warn;
use re_log_types::LogMsg;
use tokio::io::{AsyncBufRead, AsyncReadExt as _};
use tokio_stream::Stream;

use crate::{
    codec::file::{self},
    EncodingOptions, VersionPolicy,
};

use super::{options_from_bytes, DecodeError, FileHeader};

/// A decoded [`LogMsg`] with extra contextual information.
///
/// Yielded by the [`StreamingDecoder`].
#[derive(Debug, Clone)]
pub struct StreamingLogMsg {
    /// The decoded [`LogMsg`].
    pub inner: LogMsg,

    /// How many bytes does one have to go through in the underlying storage resource in order to
    /// find the start of this message?
    ///
    /// Specifically, this points to the beginning of the message's **header**.
    pub byte_offset: u64,

    /// How many bytes does this message take in the underlying storage resource?
    ///
    /// This covers both the size of the message's header _and_ its body.
    pub byte_len: u64,
}

impl std::ops::Deref for StreamingLogMsg {
    type Target = LogMsg;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for StreamingLogMsg {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub struct StreamingDecoder<R: AsyncBufRead> {
    version: CrateVersion,
    options: EncodingOptions,
    reader: R,

    /// internal buffer for unprocessed bytes
    unprocessed_bytes: BytesMut,

    /// flag to indicate if we're expecting more data to be read.
    expect_more_data: bool,

    // total number of bytes read until now
    num_bytes_read: u64,
}

impl<R: AsyncBufRead + Unpin> StreamingDecoder<R> {
    pub async fn new(version_policy: VersionPolicy, mut reader: R) -> Result<Self, DecodeError> {
        let mut data = [0_u8; FileHeader::SIZE];

        reader
            .read_exact(&mut data)
            .await
            .map_err(DecodeError::Read)?;

        let (version, options) = options_from_bytes(version_policy, &data)?;

        Ok(Self {
            version,
            options,
            reader,
            unprocessed_bytes: BytesMut::new(),
            expect_more_data: false,
            num_bytes_read: FileHeader::SIZE as _,
        })
    }

    pub fn new_with_options(version: CrateVersion, options: EncodingOptions, reader: R) -> Self {
        Self {
            version,
            options,
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

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            let Self {
                options,
                reader,
                unprocessed_bytes,
                expect_more_data,
                ..
            } = &mut *self;

            let serializer = options.serializer;
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
                match options_from_bytes(VersionPolicy::Warn, data) {
                    Ok((version, options)) => {
                        self.version = CrateVersion::max(self.version, version);
                        self.options = options;

                        Pin::new(&mut self.reader).consume(buf_length);
                        self.unprocessed_bytes.advance(FileHeader::SIZE);
                        self.num_bytes_read += FileHeader::SIZE as u64;

                        continue;
                    }
                    Err(err) => return std::task::Poll::Ready(Some(Err(err))),
                }
            }

            let (msg, processed_length) = match serializer {
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
                    let msg = file::decoder::decode_bytes(header.kind, data)?;

                    (msg, header.len as usize + header_size)
                }
            };

            let Some(mut msg) = msg else {
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

            if let LogMsg::SetStoreInfo(msg) = &mut msg {
                // Propagate the protocol version from the header into the `StoreInfo` so that all
                // parts of the app can easily access it.
                msg.info.store_version = Some(self.version);
            }

            Pin::new(&mut self.reader).consume(buf_length);
            self.unprocessed_bytes.advance(processed_length);
            self.expect_more_data = false;

            let msg = StreamingLogMsg {
                inner: msg,
                byte_offset: self.num_bytes_read,
                byte_len: processed_length as _,
            };

            self.num_bytes_read += processed_length as u64;

            return std::task::Poll::Ready(Some(Ok(msg)));
        }
    }
}

#[cfg(all(test, feature = "decoder", feature = "encoder"))]
mod tests {
    use re_build_info::CrateVersion;
    use tokio_stream::StreamExt as _;

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
                decoder
                    .map(|res| res.map(|msg| msg.inner))
                    .collect::<Result<Vec<_>, _>>()
                    .await
                    .unwrap(),
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
                decoder
                    .map(|res| res.map(|msg| msg.inner))
                    .collect::<Result<Vec<_>, _>>()
                    .await
                    .unwrap(),
            );

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

            let decoder = StreamingDecoder::new(VersionPolicy::Error, buf_reader)
                .await
                .unwrap();

            let decoded_messages = decoder.collect::<Result<Vec<_>, _>>().await.unwrap();

            for msg_expected in &decoded_messages {
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
                    let msg = file::decoder::decode_bytes(header.kind, data)
                        .unwrap()
                        .unwrap();

                    similar_asserts::assert_eq!(msg_expected.inner, msg);
                }
            }

            let decoded_messages = strip_arrow_extensions_from_log_messages(
                decoded_messages
                    .clone()
                    .into_iter()
                    .map(|msg| msg.inner)
                    .collect::<Vec<_>>(),
            );

            similar_asserts::assert_eq!(decoded_messages, messages);
        }
    }
}
