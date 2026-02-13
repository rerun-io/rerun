use crate::RawRrdManifest;
use crate::rrd::decoder::state_machine::DecoderState;
use crate::rrd::{DecodeError, Decoder, DecoderEntrypoint};

// ---

impl<T: DecoderEntrypoint> Decoder<T> {
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

// ---

/// Iteratively decodes the contents of an arbitrary buffered reader.
pub struct DecoderIterator<T, R: std::io::BufRead> {
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

impl<T, R: std::io::BufRead> DecoderIterator<T, R> {
    pub fn num_bytes_processed(&self) -> u64 {
        self.decoder.byte_chunks.num_read() as _
    }
}

impl<T: DecoderEntrypoint, R: std::io::BufRead> DecoderIterator<T, R> {
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

impl<T: DecoderEntrypoint, R: std::io::BufRead> std::iter::Iterator for DecoderIterator<T, R> {
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
                        Ok(None) if self.decoder.state == DecoderState::WaitingForStreamHeader => {
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

#[cfg(all(test, feature = "encoder"))]
mod tests {
    #![expect(unsafe_code, clippy::unwrap_used, clippy::undocumented_unsafe_blocks)] // tests

    use re_build_info::CrateVersion;
    use re_chunk::RowId;
    use re_log_types::{LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};
    use re_protos::log_msg::v1alpha1 as proto;
    use re_protos::log_msg::v1alpha1::log_msg::Msg as LogMsgProto;

    use crate::rrd::{Compression, DecoderApp, EncodingOptions, Serializer};
    use crate::{Encoder, ToTransport as _};

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

    /// Convert the test log message to their proto version and tweak them so that:
    /// - `StoreId` do not have an `ApplicationId`
    /// - `StoreInfo` does have an `ApplicationId`
    #[expect(deprecated)]
    fn legacy_fake_log_messages() -> Vec<LogMsgProto> {
        fake_log_messages()
            .into_iter()
            .map(|msg| msg.to_transport(Compression::Off).unwrap())
            .map(|mut log_msg| {
                match &mut log_msg {
                    proto::log_msg::Msg::SetStoreInfo(set_store_info) => {
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

                    proto::log_msg::Msg::ArrowMsg(proto::ArrowMsg { store_id, .. })
                    | proto::log_msg::Msg::BlueprintActivationCommand(
                        proto::BlueprintActivationCommand {
                            blueprint_id: store_id,
                            ..
                        },
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
            let mut encoder = Encoder::new_eager(version, options, write)?;
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

        let mut file = vec![];

        let options = EncodingOptions::PROTOBUF_UNCOMPRESSED;
        let mut encoder = Encoder::new_eager(rrd_version, options, &mut file).unwrap();
        for message in messages.clone() {
            unsafe {
                encoder
                    .append_transport(&message)
                    .expect("encoding should succeed");
            }
        }
        drop(encoder);

        let decoded_messages: Vec<_> = DecoderApp::decode_lazy(file.as_slice())
            .map(Result::unwrap)
            .collect();
        assert_eq!(decoded_messages.len(), messages.len());
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
        assert!(matches!(messages[0], proto::log_msg::Msg::SetStoreInfo(..)));
        assert!(matches!(messages[1], proto::log_msg::Msg::ArrowMsg(..)));
        assert!(matches!(
            messages[2],
            proto::log_msg::Msg::BlueprintActivationCommand(..)
        ));

        // make out-of-order messages
        let mut out_of_order_messages = vec![messages[1].clone(), messages[2].clone()];
        out_of_order_messages.extend(messages);

        let mut file = vec![];

        let options = EncodingOptions::PROTOBUF_UNCOMPRESSED;
        let mut encoder = Encoder::new_eager(rrd_version, options, &mut file).unwrap();
        for message in out_of_order_messages.clone() {
            unsafe {
                encoder
                    .append_transport(&message)
                    .expect("encoding should succeed");
            }
        }
        drop(encoder);

        let decoded_messages: Vec<_> = DecoderApp::decode_lazy(file.as_slice())
            .map(Result::unwrap)
            .collect();
        assert_eq!(decoded_messages.len(), orig_message_count);
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
                    crate::Encoder::new_eager(CrateVersion::LOCAL, options, writer).unwrap();
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
                    crate::Encoder::new_eager(CrateVersion::LOCAL, options, writer).unwrap();
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
                    crate::Encoder::new_eager(CrateVersion::LOCAL, options, writer).unwrap();
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
                    crate::Encoder::new_eager(CrateVersion::LOCAL, options, writer).unwrap();
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
