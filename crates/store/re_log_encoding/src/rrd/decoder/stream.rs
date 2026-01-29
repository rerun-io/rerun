use std::pin::Pin;

use tokio::io::AsyncBufRead;
use tokio_stream::{Stream, StreamExt as _};

use crate::RawRrdManifest;
use crate::rrd::decoder::state_machine::DecoderState;
use crate::rrd::{DecodeError, Decoder, DecoderEntrypoint};

// ---

impl<T: DecoderEntrypoint + Unpin> Decoder<T> {
    /// Instantiates a new lazy decoding stream on top of the given buffered reader.
    ///
    /// This does not perform any IO until the returned stream is polled. I.e. this will not
    /// fail if the reader doesn't even contain valid RRD data.
    ///
    /// This takes a `BufRead` instead of a `Read` because:
    /// * This guarantees this will never run on non-buffered input.
    /// * This lets the end-user in control of the buffering, which prevents unfortunately stacked
    ///   buffers (and thus exploding memory usage and copies).
    ///
    /// See also [`Self::decode_lazy_async_with_opts`].
    pub fn decode_lazy_async<R: tokio::io::AsyncBufRead>(reader: R) -> DecoderStream<T, R> {
        let wait_for_eos = false;
        Self::decode_lazy_async_with_opts(reader, wait_for_eos)
    }

    /// Same as [`Self::decode_lazy_async`], with extra options.
    ///
    /// * `wait_for_eos`: if true, the decoder will always wait for an end-of-stream marker before
    ///   calling it a day, even if the underlying reader has already reached its EOF state (…for now).
    ///   This only really makes sense when running in tail mode (see `RetryableFileReader`), otherwise
    ///   we'd rather terminate early when a potentially short-circuited (and therefore lacking a proper
    ///   end-of-stream marker) RRD stream indicates EOF.
    pub fn decode_lazy_async_with_opts<R: tokio::io::AsyncBufRead>(
        reader: R,
        wait_for_eos: bool,
    ) -> DecoderStream<T, R> {
        let decoder = Self::new();
        DecoderStream {
            decoder,
            reader,
            wait_for_eos,
            first_msg: None,
        }
    }

    /// Instantiates a new eager decoding stream on top of the given buffered reader.
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
    /// See also [`Self::decode_eager_async_with_opts`].
    pub async fn decode_eager_async<R: tokio::io::AsyncBufRead + Unpin>(
        reader: R,
    ) -> Result<DecoderStream<T, R>, DecodeError> {
        let wait_for_eos = false;
        Self::decode_eager_async_with_opts(reader, wait_for_eos).await
    }

    /// Same as [`Self::decode_eager_async`], with extra options.
    ///
    /// * `wait_for_eos`: if true, the decoder will always wait for an end-of-stream marker before
    ///   calling it a day, even if the underlying reader has already reached its EOF state (…for now).
    ///   This only really makes sense when running in tail mode (see `RetryableFileReader`), otherwise
    ///   we'd rather terminate early when a potentially short-circuited (and therefore lacking a proper
    ///   end-of-stream marker) RRD stream indicates EOF.
    pub async fn decode_eager_async_with_opts<R: tokio::io::AsyncBufRead + Unpin>(
        reader: R,
        wait_for_eos: bool,
    ) -> Result<DecoderStream<T, R>, DecodeError> {
        let decoder = Self::new();
        let mut it = DecoderStream {
            decoder,
            reader,
            wait_for_eos,
            first_msg: None,
        };

        it.first_msg = it.next().await.transpose()?;

        Ok(it)
    }
}

// ---

/// Iteratively decodes the contents of an arbitrary buffered reader.
pub struct DecoderStream<T, R: AsyncBufRead> {
    pub decoder: Decoder<T>,
    pub reader: R,

    /// If true, the decoder will always wait for an end-of-stream marker before calling it a day,
    /// even if the underlying reader has already reached its EOF state (…for now).
    ///
    /// This only really makes sense when running in tail mode (see `RetryableFileReader`),
    /// otherwise we'd rather terminate early when a potentially short-circuited (and therefore
    /// lacking a proper end-of-stream marker) RRD stream indicates EOF.
    pub wait_for_eos: bool,

    /// See [`Decoder::decode_eager`] for more information.
    pub first_msg: Option<T>,
}

impl<T: DecoderEntrypoint, R: AsyncBufRead> DecoderStream<T, R> {
    /// Returns all the RRD manifests accumulated _so far_.
    ///
    /// RRD manifests are parsed from footers, of which there might be more than one e.g. in the
    /// case of concatenated streams.
    ///
    /// This is not cheap: it automatically performs the transport to app level conversion.
    pub fn rrd_manifests(&self) -> Result<Vec<RawRrdManifest>, DecodeError> {
        self.decoder.rrd_manifests()
    }
}

// NOTE: This is the exact same implementation as `impl Iterator for DecoderIterator`, just asyncified.
impl<T: DecoderEntrypoint + Unpin, R: AsyncBufRead + Unpin> Stream for DecoderStream<T, R> {
    type Item = Result<T, DecodeError>;

    #[tracing::instrument(name = "streaming_decoder", level = "trace", skip_all)]
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(first_msg) = self.first_msg.take() {
            // The stream was eagerly initialized so make sure to return the first message if there's any.
            return std::task::Poll::Ready(Some(Ok(first_msg)));
        }

        loop {
            let Self {
                decoder,
                reader,
                wait_for_eos,
                first_msg: _,
            } = &mut *self;

            let mut reader = Pin::new(reader);

            match decoder.try_read() {
                Ok(Some(msg)) => return std::task::Poll::Ready(Some(Ok(msg))),
                Ok(None) => {}
                Err(err) => return std::task::Poll::Ready(Some(Err(err))),
            }

            match reader.as_mut().poll_fill_buf(cx) {
                // EOF
                std::task::Poll::Ready(Ok([])) => {
                    // There's nothing more to read…
                    match decoder.try_read() {
                        // …but we still have enough buffered that we can still manage to decode
                        // more messages, so go on for now.
                        Ok(Some(msg)) => return std::task::Poll::Ready(Some(Ok(msg))),

                        // …and we don't want to explicitly wait around for more to come, so just leave.
                        Ok(None) if !*wait_for_eos => return std::task::Poll::Ready(None),

                        // …and the underlying decoder already considers that it's done (i.e. it's
                        // waiting for a whole new stream to begin): time to stop.
                        Ok(None) if decoder.state == DecoderState::WaitingForStreamHeader => {
                            return std::task::Poll::Ready(None);
                        }

                        // …but the underlying decoder doesn't believe it's done yet (i.e. it's still
                        // waiting for an EOS marker to show up): we continue.
                        Ok(None) => {}

                        Err(err) => return std::task::Poll::Ready(Some(Err(err))),
                    }
                }

                std::task::Poll::Ready(Ok(buf)) => {
                    decoder.push_byte_chunk(buf.to_vec());
                    let len = buf.len(); // borrowck limitation
                    reader.consume(len);
                }

                std::task::Poll::Ready(Err(err))
                    if err.kind() == std::io::ErrorKind::Interrupted => {}

                std::task::Poll::Ready(Err(err)) => {
                    return std::task::Poll::Ready(Some(Err(err.into())));
                }

                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

#[cfg(all(test, feature = "encoder"))]
mod tests {
    use re_build_info::CrateVersion;
    use re_chunk::RowId;
    use re_log_types::{LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};
    use tokio_stream::StreamExt as _;

    use crate::DecoderApp;
    use crate::rrd::{Compression, EncodingOptions, Serializer};

    #[expect(clippy::unwrap_used)] // acceptable for tests
    fn fake_log_messages() -> Vec<LogMsg> {
        let store_id = StoreId::random(StoreKind::Blueprint, "test_app");

        let arrow_msg = re_chunk::Chunk::builder("test_entity")
            .with_archetype(
                re_chunk::RowId::new(),
                re_log_types::TimePoint::default().with(
                    re_log_types::Timeline::new_sequence("blueprint"),
                    re_log_types::TimeInt::from_millis(re_log_types::NonMinI64::MIN),
                ),
                &re_sdk_types::blueprint::archetypes::Background::new(
                    re_sdk_types::blueprint::components::BackgroundKind::SolidColor,
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

            let decoder = DecoderApp::decode_eager_async(buf_reader).await.unwrap();
            let decoded_messages = decoder.map(Result::unwrap).collect::<Vec<_>>().await;

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

            let decoder = DecoderApp::decode_eager_async(buf_reader).await.unwrap();
            let decoded_messages = decoder.map(Result::unwrap).collect::<Vec<_>>().await;

            similar_asserts::assert_eq!(decoded_messages, messages);
        }
    }
}
