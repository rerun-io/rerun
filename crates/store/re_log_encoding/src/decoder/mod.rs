//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

pub mod stream;

#[cfg(feature = "decoder")]
pub mod streaming;

use std::io::BufRead as _;
use std::io::Read as _;

use re_build_info::CrateVersion;
use re_log_types::LogMsg;

use crate::FileHeader;
use crate::OLD_RRD_HEADERS;
use crate::app_id_cache::ApplicationIdCache;
use crate::codec;
use crate::codec::file::decoder;
use crate::{EncodingOptions, Serializer};
// ----------------------------------------------------------------------------

fn warn_on_version_mismatch(encoded_version: [u8; 4]) -> Result<(), DecodeError> {
    // We used 0000 for all .rrd files up until 2023-02-27, post 0.2.0 release:
    let encoded_version = if encoded_version == [0, 0, 0, 0] {
        CrateVersion::new(0, 2, 0)
    } else {
        CrateVersion::from_bytes(encoded_version)
    };

    if encoded_version.major == 0 && encoded_version.minor < 23 {
        // We broke compatibility for 0.23 for (hopefully) the last time.
        Err(DecodeError::IncompatibleRerunVersion {
            file: encoded_version,
            local: CrateVersion::LOCAL,
        })
    } else if encoded_version <= CrateVersion::LOCAL {
        // Loading old files should be fine, and if it is not, the chunk migration in re_sorbet should already log a warning.
        Ok(())
    } else {
        re_log::warn_once!(
            "Found data stream with Rerun version {encoded_version} which is newer than the local Rerun version ({}). This file may contain data that is not compatible with this version of Rerun. Consider updating Rerun.",
            CrateVersion::LOCAL
        );
        Ok(())
    }
}

// ----------------------------------------------------------------------------

/// On failure to encode or serialize a [`LogMsg`].
#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    #[error("Not an .rrd file")]
    NotAnRrd,

    #[error("Data was from an old, incompatible Rerun version")]
    OldRrdVersion,

    #[error(
        "Data from Rerun version {file}, which is incompatible with the local Rerun version {local}"
    )]
    IncompatibleRerunVersion {
        file: CrateVersion,
        local: CrateVersion,
    },

    /// This is returned when `ArrowMsg` or `BlueprintActivationCommand` are received with a legacy
    /// store id (missing the application id) before the corresponding `SetStoreInfo` message. In
    /// that case, the best effort is to recover by dropping such message with a warning.
    #[error("Message with an unknown application id was received.")]
    StoreIdMissingApplicationId,

    #[error("Failed to decode the options: {0}")]
    Options(#[from] crate::OptionsError),

    #[error("Failed to read: {0}")]
    Read(#[from] std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4(#[from] lz4_flex::block::DecompressError),

    #[error("Protobuf error: {0}")]
    Protobuf(#[from] re_protos::external::prost::DecodeError),

    #[error("Could not convert type from protobuf: {0}")]
    TypeConversion(#[from] re_protos::TypeConversionError),

    #[error("Sorbet error: {0}")]
    SorbetError(#[from] re_sorbet::SorbetError),

    #[error("Failed to read chunk: {0}")]
    Chunk(#[from] re_chunk::ChunkError),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Codec error: {0}")]
    Codec(#[from] codec::CodecError),
}

// ----------------------------------------------------------------------------

pub fn decode_bytes(bytes: &[u8]) -> Result<Vec<LogMsg>, DecodeError> {
    re_tracing::profile_function!();
    let decoder = Decoder::new(std::io::Cursor::new(bytes))?;
    let mut msgs = vec![];
    for msg in decoder {
        msgs.push(msg?);
    }
    Ok(msgs)
}

// ----------------------------------------------------------------------------

/// Read encoding options from the beginning of the stream.
pub fn read_options(
    reader: &mut impl std::io::Read,
) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut data = [0_u8; FileHeader::SIZE];
    reader.read_exact(&mut data).map_err(DecodeError::Read)?;

    options_from_bytes(&data)
}

/// Read encoding options from the beginning of the stream asynchronously.
pub async fn read_options_async(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut data = [0_u8; FileHeader::SIZE];

    use tokio::io::AsyncReadExt as _;
    reader
        .read_exact(&mut data)
        .await
        .map_err(DecodeError::Read)?;

    options_from_bytes(&data)
}

pub fn options_from_bytes(bytes: &[u8]) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut read = std::io::Cursor::new(bytes);

    let FileHeader {
        magic,
        version,
        options,
    } = FileHeader::decode(&mut read)?;

    if OLD_RRD_HEADERS.contains(&magic) {
        return Err(DecodeError::OldRrdVersion);
    } else if &magic != crate::RRD_HEADER {
        return Err(DecodeError::NotAnRrd);
    }

    warn_on_version_mismatch(version)?;

    match options.serializer {
        Serializer::Protobuf => {}
    }

    Ok((CrateVersion::from_bytes(version), options))
}

enum Reader<R: std::io::Read> {
    Raw(R),
    Buffered(std::io::BufReader<R>),
}

impl<R: std::io::Read> std::io::Read for Reader<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Raw(read) => read.read(buf),
            Self::Buffered(read) => read.read(buf),
        }
    }
}

pub struct Decoder<R: std::io::Read> {
    version: CrateVersion,
    options: EncodingOptions,
    read: Reader<R>,

    /// The size in bytes of the data that has been decoded up to now.
    size_bytes: u64,

    /// The application id cache used for migrating old data.
    app_id_cache: ApplicationIdCache,
}

impl<R: std::io::Read> Decoder<R> {
    /// Instantiates a new decoder.
    ///
    /// This does not support concatenated streams (i.e. streams of bytes where multiple RRD files
    /// -- not recordings, RRD files! -- follow each other).
    ///
    /// If you're not familiar with concatenated RRD streams, then this is probably the function
    /// that you want to be using.
    ///
    /// See also:
    /// * [`Decoder::new_concatenated`]
    pub fn new(mut read: R) -> Result<Self, DecodeError> {
        re_tracing::profile_function!();

        let mut data = [0_u8; FileHeader::SIZE];
        read.read_exact(&mut data).map_err(DecodeError::Read)?;

        let (version, options) = options_from_bytes(&data)?;

        Ok(Self {
            version,
            options,
            read: Reader::Raw(read),
            size_bytes: FileHeader::SIZE as _,
            app_id_cache: ApplicationIdCache::default(),
        })
    }

    pub fn new_with_options(options: EncodingOptions, version: CrateVersion, read: R) -> Self {
        Self {
            version,
            options,
            read: Reader::Raw(read),
            size_bytes: FileHeader::SIZE as _,
            app_id_cache: ApplicationIdCache::default(),
        }
    }

    /// Instantiates a new concatenated decoder.
    ///
    /// This will gracefully handle concatenated RRD streams (i.e. streams of bytes where multiple
    /// RRD files -- not recordings, RRD files! -- follow each other), at the cost of extra
    /// performance overhead, by looking ahead for potential `FileHeader`s in the stream.
    ///
    /// The [`CrateVersion`] of the final, deconcatenated stream will correspond to the most recent
    /// version among all the versions found in the stream.
    ///
    /// This is particularly useful when working with stdio streams.
    ///
    /// If you're not familiar with concatenated RRD streams, then you probably want to use
    /// [`Decoder::new`] instead.
    ///
    /// See also:
    /// * [`Decoder::new`]
    pub fn new_concatenated(mut read: std::io::BufReader<R>) -> Result<Self, DecodeError> {
        re_tracing::profile_function!();

        let mut data = [0_u8; FileHeader::SIZE];
        read.read_exact(&mut data).map_err(DecodeError::Read)?;

        let (version, options) = options_from_bytes(&data)?;

        Ok(Self {
            version,
            options,
            read: Reader::Buffered(read),
            size_bytes: FileHeader::SIZE as _,
            app_id_cache: ApplicationIdCache::default(),
        })
    }

    /// Returns the Rerun version that was used to encode the data in the first place.
    #[inline]
    pub fn version(&self) -> CrateVersion {
        self.version
    }

    // TODO(jan): stop returning number of read bytes, use cursors wrapping readers instead.
    /// Returns the size in bytes of the data that has been decoded up to now.
    #[inline]
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// Returns the next message in the stream, dropping messages missing application id that cannot
    /// be migrated (because they arrived before `SetStoreInfo`).
    fn next<F, T>(&mut self, mut decoder: F) -> Option<Result<T, DecodeError>>
    where
        F: FnMut(&mut ApplicationIdCache, &mut Reader<R>) -> Result<(u64, Option<T>), DecodeError>,
    {
        //TODO(#10730): remove this if/when we remove the legacy `StoreId` migration.
        loop {
            let result = self.next_impl(&mut decoder);
            if matches!(result, Some(Err(DecodeError::StoreIdMissingApplicationId))) {
                re_log::warn_once!(
                    "Dropping message without application id which arrived before `SetStoreInfo`."
                );
            } else {
                return result;
            }
        }
    }

    /// Returns the next message in the stream.
    fn next_impl<F, T>(&mut self, decoder: &mut F) -> Option<Result<T, DecodeError>>
    where
        F: FnMut(&mut ApplicationIdCache, &mut Reader<R>) -> Result<(u64, Option<T>), DecodeError>,
    {
        re_tracing::profile_function!();

        if self.peek_file_header() {
            // We've found another file header in the middle of the stream, it's time to switch
            // gears and start over on this new file.

            let mut data = [0_u8; FileHeader::SIZE];
            if let Err(err) = self.read.read_exact(&mut data).map_err(DecodeError::Read) {
                return Some(Err(err));
            }

            let (version, options) = match options_from_bytes(&data) {
                Ok(opts) => opts,
                Err(err) => return Some(Err(err)),
            };

            self.version = CrateVersion::max(self.version, version);
            self.options = options;
            self.size_bytes += FileHeader::SIZE as u64;
        }

        let msg = match self.options.serializer {
            Serializer::Protobuf => match decoder(&mut self.app_id_cache, &mut self.read) {
                Ok((read_bytes, msg)) => {
                    self.size_bytes += read_bytes;
                    msg
                }

                Err(err) => match err {
                    DecodeError::Read(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                        return None;
                    }
                    _ => return Some(Err(err)),
                },
            },
        };

        let Some(msg) = msg else {
            // we might have a concatenated stream, so we peek beyond end of file marker to see
            if self.peek_file_header() {
                re_log::debug!(
                    "Reached end of stream, but it seems we have a concatenated file, continuing"
                );
                return self.next_impl(decoder);
            }

            re_log::trace!("Reached end of stream, iterator complete");
            return None;
        };

        Some(Ok(msg))
    }

    /// Peeks ahead in search of additional `FileHeader`s in the stream.
    ///
    /// Returns true if a valid header was found.
    ///
    /// No-op if the decoder wasn't initialized with [`Decoder::new_concatenated`].
    fn peek_file_header(&mut self) -> bool {
        match &mut self.read {
            Reader::Raw(_) => false,
            Reader::Buffered(read) => {
                if read.fill_buf().map_err(DecodeError::Read).is_err() {
                    return false;
                }

                let mut read = std::io::Cursor::new(read.buffer());
                if FileHeader::decode(&mut read).is_err() {
                    return false;
                }

                true
            }
        }
    }

    /// Returns a [`RawIterator`] over the transport-level data (Protobuf).
    pub fn into_raw_iter(self) -> RawIterator<R> {
        RawIterator { decoder: self }
    }
}

/// Iterator over the transport-level data (Protobuf).
///
/// Application-level data (Arrow) is not decoded.
pub struct RawIterator<R: std::io::Read> {
    decoder: Decoder<R>,
}

impl<R: std::io::Read> RawIterator<R> {
    /// Returns the size in bytes of the data that has been decoded up to now.
    //
    // TODO(jan): stop returning number of read bytes, use cursors wrapping readers instead.
    #[inline]
    pub fn size_bytes(&self) -> u64 {
        self.decoder.size_bytes
    }
}

impl<R: std::io::Read> Iterator for Decoder<R> {
    type Item = Result<LogMsg, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next(decoder::decode_to_app)
    }
}

impl<R: std::io::Read> Iterator for RawIterator<R> {
    type Item = Result<re_protos::log_msg::v1alpha1::log_msg::Msg, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.decoder
            .next(|_app_id_cache, reader| decoder::decode_to_transport(reader))
    }
}

// ----------------------------------------------------------------------------

#[cfg(all(test, feature = "decoder", feature = "encoder"))]
mod tests {
    #![allow(clippy::unwrap_used)] // acceptable for tests

    use re_build_info::CrateVersion;
    use re_chunk::RowId;
    use re_log_types::{SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};
    use re_protos::log_msg::v1alpha1 as proto;
    use re_protos::log_msg::v1alpha1::LogMsg as LogMsgProto;

    use super::*;
    use crate::Compression;
    use crate::codec::arrow::encode_arrow;
    use crate::encoder::DroppableEncoder;

    pub fn fake_log_messages() -> Vec<LogMsg> {
        let store_id = StoreId::random(StoreKind::Blueprint);

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
                info: StoreInfo {
                    store_id: store_id.clone(),
                    cloned_from: None,
                    store_source: StoreSource::RustSdk {
                        rustc_version: String::new(),
                        llvm_version: String::new(),
                    },
                    store_version: Some(CrateVersion::LOCAL),
                },
            }),
            LogMsg::ArrowMsg(store_id.clone(), arrow_msg),
            LogMsg::BlueprintActivationCommand(re_log_types::BlueprintActivationCommand {
                blueprint_id: store_id,
                make_active: true,
                make_default: true,
            }),
        ]
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
            LogMsg::ArrowMsg(store_id, arrow_msg) => {
                let payload = encode_arrow(&arrow_msg.batch, Compression::Off)
                    .expect("compression should succeed");
                let arrow_msg = ArrowMsg {
                    store_id: Some(store_id.clone().into()),
                    chunk_id: Some(arrow_msg.chunk_id.into()),
                    compression: proto::Compression::None as i32,
                    uncompressed_size: payload.uncompressed_size as i32,
                    encoding: Encoding::ArrowIpc as i32,
                    payload: payload.data.into(),
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
                };

                log_msg
            })
            .collect()
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
            crate::encoder::encode_ref(rrd_version, options, messages.iter().map(Ok), &mut file)
                .unwrap();

            let decoded_messages = Decoder::new(&mut file.as_slice())
                .unwrap()
                .collect::<Result<Vec<LogMsg>, DecodeError>>()
                .unwrap();

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

            let mut encoder = DroppableEncoder::new(rrd_version, options, &mut file).unwrap();
            for message in messages.clone() {
                encoder
                    .append_proto(message)
                    .expect("encoding should succeed");
            }
            drop(encoder);

            let decoded_messages = Decoder::new(&mut file.as_slice())
                .unwrap()
                .collect::<Result<Vec<LogMsg>, DecodeError>>()
                .unwrap();

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

            let mut encoder = DroppableEncoder::new(rrd_version, options, &mut file).unwrap();
            for message in out_of_order_messages.clone() {
                encoder
                    .append_proto(message)
                    .expect("encoding should succeed");
            }
            drop(encoder);

            let decoded_messages = Decoder::new(&mut file.as_slice())
                .unwrap()
                .collect::<Result<Vec<LogMsg>, DecodeError>>()
                .unwrap();

            assert_eq!(decoded_messages.len(), orig_message_count);
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
            let writer = std::io::Cursor::new(&mut data);
            let mut encoder1 =
                crate::encoder::Encoder::new(CrateVersion::LOCAL, options, writer).unwrap();
            for message in &messages {
                encoder1.append(message).unwrap();
            }
            encoder1.finish().unwrap();

            let written = data.len() as u64;
            let mut writer = std::io::Cursor::new(&mut data);
            writer.set_position(written);
            let mut encoder2 =
                crate::encoder::Encoder::new(CrateVersion::LOCAL, options, writer).unwrap();
            for message in &messages {
                encoder2.append(message).unwrap();
            }
            encoder2.finish().unwrap();

            let decoder =
                Decoder::new_concatenated(std::io::BufReader::new(data.as_slice())).unwrap();

            let decoded_messages = decoder.into_iter().collect::<Result<Vec<_>, _>>().unwrap();

            similar_asserts::assert_eq!(decoded_messages, [messages.clone(), messages].concat());
        }
    }
}
