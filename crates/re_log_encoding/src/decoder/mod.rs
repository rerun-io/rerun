//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

pub mod stream;

use re_build_info::CrateVersion;
use re_log_types::LogMsg;

use crate::FileHeader;
use crate::MessageHeader;
use crate::OLD_RRD_HEADERS;
use crate::{Compression, EncodingOptions, Serializer};

// ----------------------------------------------------------------------------

/// How to handle version mismatches during decoding.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VersionPolicy {
    /// Warn if the versions don't match, but continue loading.
    ///
    /// We usually use this for loading `.rrd` recordings.
    Warn,

    /// Return [`DecodeError::IncompatibleRerunVersion`] if the versions aren't compatible.
    ///
    /// We usually use this for tests, and for loading `.blueprint` files.
    Error,
}

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

    const LOCAL_VERSION: CrateVersion = CrateVersion::parse(env!("CARGO_PKG_VERSION"));

    if encoded_version.is_compatible_with(LOCAL_VERSION) {
        Ok(())
    } else {
        match version_policy {
            VersionPolicy::Warn => {
                re_log::warn_once!(
                    "Found log stream with Rerun version {encoded_version}, \
                     which is incompatible with the local Rerun version {LOCAL_VERSION}. \
                     Loading will try to continue, but might fail in subtle ways."
                );
                Ok(())
            }
            VersionPolicy::Error => Err(DecodeError::IncompatibleRerunVersion {
                file: encoded_version,
                local: LOCAL_VERSION,
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
    Read(std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4(lz4_flex::block::DecompressError),

    #[error("MsgPack error: {0}")]
    MsgPack(#[from] rmp_serde::decode::Error),
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

pub fn read_options(
    version_policy: VersionPolicy,
    bytes: &[u8],
) -> Result<EncodingOptions, DecodeError> {
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
        Serializer::MsgPack => {}
    }

    Ok(options)
}

pub struct Decoder<R: std::io::Read> {
    compression: Compression,
    read: R,
    uncompressed: Vec<u8>, // scratch space
    compressed: Vec<u8>,   // scratch space
}

impl<R: std::io::Read> Decoder<R> {
    pub fn new(version_policy: VersionPolicy, mut read: R) -> Result<Self, DecodeError> {
        re_tracing::profile_function!();

        let mut data = [0_u8; FileHeader::SIZE];
        read.read_exact(&mut data).map_err(DecodeError::Read)?;
        let compression = read_options(version_policy, &data)?.compression;

        Ok(Self {
            compression,
            read,
            uncompressed: vec![],
            compressed: vec![],
        })
    }
}

impl<R: std::io::Read> Iterator for Decoder<R> {
    type Item = Result<LogMsg, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        re_tracing::profile_function!();

        let header = match MessageHeader::decode(&mut self.read) {
            Ok(header) => header,
            Err(err) => match err {
                DecodeError::Read(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return None
                }
                other => return Some(Err(other)),
            },
        };

        let uncompressed_len = header.uncompressed_len as usize;
        self.uncompressed
            .resize(self.uncompressed.len().max(uncompressed_len), 0);

        match self.compression {
            Compression::Off => {
                re_tracing::profile_scope!("read uncompressed");
                if let Err(err) = self
                    .read
                    .read_exact(&mut self.uncompressed[..uncompressed_len])
                {
                    return Some(Err(DecodeError::Read(err)));
                }
            }
            Compression::LZ4 => {
                let compressed_len = header.compressed_len as usize;
                self.compressed
                    .resize(self.compressed.len().max(compressed_len), 0);

                {
                    re_tracing::profile_scope!("read compressed");
                    if let Err(err) = self.read.read_exact(&mut self.compressed[..compressed_len]) {
                        return Some(Err(DecodeError::Read(err)));
                    }
                }

                re_tracing::profile_scope!("lz4");
                if let Err(err) = lz4_flex::block::decompress_into(
                    &self.compressed[..compressed_len],
                    &mut self.uncompressed[..uncompressed_len],
                ) {
                    return Some(Err(DecodeError::Lz4(err)));
                }
            }
        }

        re_tracing::profile_scope!("MsgPack deser");
        match rmp_serde::from_slice(&self.uncompressed[..uncompressed_len]) {
            Ok(msg) => Some(Ok(msg)),
            Err(err) => Some(Err(err.into())),
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(all(feature = "decoder", feature = "encoder"))]
#[test]
fn test_encode_decode() {
    use re_log_types::{
        ApplicationId, LogMsg, RowId, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
        Time,
    };

    let messages = vec![LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: RowId::new(),
        info: StoreInfo {
            application_id: ApplicationId("test".to_owned()),
            store_id: StoreId::random(StoreKind::Recording),
            started: Time::now(),
            store_source: StoreSource::RustSdk {
                rustc_version: String::new(),
                llvm_version: String::new(),
            },
            store_kind: re_log_types::StoreKind::Recording,
        },
    })];

    let options = [
        EncodingOptions {
            compression: Compression::Off,
            serializer: Serializer::MsgPack,
        },
        EncodingOptions {
            compression: Compression::LZ4,
            serializer: Serializer::MsgPack,
        },
    ];

    for options in options {
        let mut file = vec![];
        crate::encoder::encode(options, messages.iter(), &mut file).unwrap();

        let decoded_messages = Decoder::new(VersionPolicy::Error, &mut file.as_slice())
            .unwrap()
            .collect::<Result<Vec<LogMsg>, DecodeError>>()
            .unwrap();

        assert_eq!(messages, decoded_messages);
    }
}
