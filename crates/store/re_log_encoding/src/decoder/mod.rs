//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

pub mod stream;

#[cfg(feature = "decoder")]
pub mod streaming;

use std::io::BufRead as _;
use std::io::Read as _;

use re_build_info::CrateVersion;
use re_log_types::LogMsg;

use crate::codec;
use crate::codec::file::decoder;
use crate::FileHeader;
use crate::VersionPolicy;
use crate::OLD_RRD_HEADERS;
use crate::{EncodingOptions, Serializer};

// ----------------------------------------------------------------------------

fn warn_on_version_mismatch(
    version_policy: VersionPolicy,
    encoded_version: [u8; 4],
) -> Result<(), DecodeError> {
    // We used 0000 for all .rrd files up until 2023-02-27, post 0.2.0 release:
    let encoded_version = if encoded_version == [0, 0, 0, 0] {
        CrateVersion::new(0, 2, 0)
    } else {
        CrateVersion::from_bytes(encoded_version)
    };

    if encoded_version.is_compatible_with(CrateVersion::LOCAL) {
        Ok(())
    } else {
        match version_policy {
            VersionPolicy::Warn => {
                re_log::warn_once!(
                    "Found log stream with Rerun version {encoded_version}, \
                     which is incompatible with the local Rerun version {}. \
                     Loading will try to continue, but might fail in subtle ways.",
                    CrateVersion::LOCAL,
                );
                Ok(())
            }
            VersionPolicy::Error => Err(DecodeError::IncompatibleRerunVersion {
                file: encoded_version,
                local: CrateVersion::LOCAL,
            }),
        }
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

    #[error("Data from Rerun version {file}, which is incompatible with the local Rerun version {local}")]
    IncompatibleRerunVersion {
        file: CrateVersion,
        local: CrateVersion,
    },

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

    #[error("Failed to read chunk: {0}")]
    Chunk(#[from] re_chunk::ChunkError),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Codec error: {0}")]
    Codec(#[from] codec::CodecError),
}

// ----------------------------------------------------------------------------

pub fn decode_bytes(
    version_policy: VersionPolicy,
    bytes: &[u8],
) -> Result<Vec<LogMsg>, DecodeError> {
    re_tracing::profile_function!();
    let decoder = Decoder::new(version_policy, std::io::Cursor::new(bytes))?;
    let mut msgs = vec![];
    for msg in decoder {
        msgs.push(msg?);
    }
    Ok(msgs)
}

// ----------------------------------------------------------------------------

/// Read encoding options from the beginning of the stream.
pub fn read_options(
    version_policy: VersionPolicy,
    reader: &mut impl std::io::Read,
) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut data = [0_u8; FileHeader::SIZE];
    reader.read_exact(&mut data).map_err(DecodeError::Read)?;

    options_from_bytes(version_policy, &data)
}

/// Read encoding options from the beginning of the stream asynchronously.
pub async fn read_options_async(
    version_policy: VersionPolicy,
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut data = [0_u8; FileHeader::SIZE];

    use tokio::io::AsyncReadExt as _;
    reader
        .read_exact(&mut data)
        .await
        .map_err(DecodeError::Read)?;

    options_from_bytes(version_policy, &data)
}

pub fn options_from_bytes(
    version_policy: VersionPolicy,
    bytes: &[u8],
) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
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

    warn_on_version_mismatch(version_policy, version)?;

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
    pub fn new(version_policy: VersionPolicy, mut read: R) -> Result<Self, DecodeError> {
        re_tracing::profile_function!();

        let mut data = [0_u8; FileHeader::SIZE];
        read.read_exact(&mut data).map_err(DecodeError::Read)?;

        let (version, options) = options_from_bytes(version_policy, &data)?;

        Ok(Self {
            version,
            options,
            read: Reader::Raw(read),
            size_bytes: FileHeader::SIZE as _,
        })
    }

    pub fn new_with_options(options: EncodingOptions, version: CrateVersion, read: R) -> Self {
        Self {
            version,
            options,
            read: Reader::Raw(read),
            size_bytes: FileHeader::SIZE as _,
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
    pub fn new_concatenated(
        version_policy: VersionPolicy,
        mut read: std::io::BufReader<R>,
    ) -> Result<Self, DecodeError> {
        re_tracing::profile_function!();

        let mut data = [0_u8; FileHeader::SIZE];
        read.read_exact(&mut data).map_err(DecodeError::Read)?;

        let (version, options) = options_from_bytes(version_policy, &data)?;

        Ok(Self {
            version,
            options,
            read: Reader::Buffered(read),
            size_bytes: FileHeader::SIZE as _,
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
}

impl<R: std::io::Read> Iterator for Decoder<R> {
    type Item = Result<LogMsg, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        re_tracing::profile_function!();

        if self.peek_file_header() {
            // We've found another file header in the middle of the stream, it's time to switch
            // gears and start over on this new file.

            let mut data = [0_u8; FileHeader::SIZE];
            if let Err(err) = self.read.read_exact(&mut data).map_err(DecodeError::Read) {
                return Some(Err(err));
            }

            let (version, options) = match options_from_bytes(VersionPolicy::Warn, &data) {
                Ok(opts) => opts,
                Err(err) => return Some(Err(err)),
            };

            self.version = CrateVersion::max(self.version, version);
            self.options = options;
            self.size_bytes += FileHeader::SIZE as u64;
        }

        let msg = match self.options.serializer {
            Serializer::Protobuf => match decoder::decode(&mut self.read) {
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

        let Some(mut msg) = msg else {
            // we might have a concatenated stream, so we peek beyond end of file marker to see
            if self.peek_file_header() {
                re_log::debug!(
                    "Reached end of stream, but it seems we have a concatenated file, continuing"
                );
                return self.next();
            }

            re_log::trace!("Reached end of stream, iterator complete");
            return None;
        };

        if let LogMsg::SetStoreInfo(msg) = &mut msg {
            // Propagate the protocol version from the header into the `StoreInfo` so that all
            // parts of the app can easily access it.
            msg.info.store_version = Some(self.version());
        }

        Some(Ok(msg))
    }
}

// ----------------------------------------------------------------------------

#[cfg(all(test, feature = "decoder", feature = "encoder"))]
mod tests {
    #![allow(clippy::unwrap_used)] // acceptable for tests

    use crate::Compression;

    use super::*;
    use re_build_info::CrateVersion;
    use re_chunk::RowId;
    use re_log_types::{ApplicationId, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};

    // TODO(#3741): remove this once we are all in on arrow-rs
    pub fn strip_arrow_extensions_from_log_messages(log_msg: Vec<LogMsg>) -> Vec<LogMsg> {
        log_msg
            .into_iter()
            .map(LogMsg::strip_arrow_extension_types)
            .collect()
    }

    pub fn fake_log_messages() -> Vec<LogMsg> {
        let store_id = StoreId::random(StoreKind::Blueprint);

        let arrow_msg = re_chunk::Chunk::builder("test_entity".into())
            .with_archetype(
                re_chunk::RowId::new(),
                re_log_types::TimePoint::default().with(
                    re_log_types::Timeline::new_sequence("blueprint"),
                    re_log_types::TimeInt::from_milliseconds(re_log_types::NonMinI64::MIN),
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

        strip_arrow_extensions_from_log_messages(vec![
            LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::new(),
                info: StoreInfo {
                    application_id: ApplicationId("test".to_owned()),
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
        ])
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

            let decoded_messages = strip_arrow_extensions_from_log_messages(
                Decoder::new(VersionPolicy::Error, &mut file.as_slice())
                    .unwrap()
                    .collect::<Result<Vec<LogMsg>, DecodeError>>()
                    .unwrap(),
            );

            similar_asserts::assert_eq!(decoded_messages, messages);
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

            let decoder = Decoder::new_concatenated(
                VersionPolicy::Error,
                std::io::BufReader::new(data.as_slice()),
            )
            .unwrap();

            let decoded_messages = strip_arrow_extensions_from_log_messages(
                decoder.into_iter().collect::<Result<Vec<_>, _>>().unwrap(),
            );

            similar_asserts::assert_eq!(decoded_messages, [messages.clone(), messages].concat());
        }
    }
}
