//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

pub mod stream;

use std::io::BufRead as _;
use std::io::Read;

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
    /// We usually use this for tests, and for loading `.rbl` blueprint files.
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
        Serializer::MsgPack => {}
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
    compression: Compression,
    read: Reader<R>,
    uncompressed: Vec<u8>, // scratch space
    compressed: Vec<u8>,   // scratch space
}

impl<R: std::io::Read> Decoder<R> {
    /// Instantiates a new decoder.
    ///
    /// This does not support multiplexed streams.
    ///
    /// If you're not familiar with multiplexed RRD streams, then this is probably the function
    /// that you want to be using.
    ///
    /// See also:
    /// * [`Decoder::new_multiplexed`]
    pub fn new(version_policy: VersionPolicy, mut read: R) -> Result<Self, DecodeError> {
        re_tracing::profile_function!();

        let mut data = [0_u8; FileHeader::SIZE];
        read.read_exact(&mut data).map_err(DecodeError::Read)?;

        let (version, options) = read_options(version_policy, &data)?;
        let compression = options.compression;

        Ok(Self {
            version,
            compression,
            read: Reader::Raw(read),
            uncompressed: vec![],
            compressed: vec![],
        })
    }

    /// Instantiates a new multiplexed decoder.
    ///
    /// This will gracefully handle multiplexed RRD streams, at the cost of extra performance
    /// overhead, by looking ahead for potential `FileHeader`s in the stream.
    ///
    /// The [`CrateVersion`] of the final, demultiplexed stream will correspond to the most recent
    /// version among all the versions found in the stream.
    ///
    /// This is particularly useful when working with stdio streams.
    ///
    /// If you're not familiar with multiplexed RRD streams, then you probably want to use
    /// [`Decoder::new`] instead.
    ///
    /// See also:
    /// * [`Decoder::new`]
    pub fn new_multiplexed(
        version_policy: VersionPolicy,
        mut read: std::io::BufReader<R>,
    ) -> Result<Self, DecodeError> {
        re_tracing::profile_function!();

        let mut data = [0_u8; FileHeader::SIZE];
        read.read_exact(&mut data).map_err(DecodeError::Read)?;

        let (version, options) = read_options(version_policy, &data)?;
        let compression = options.compression;

        Ok(Self {
            version,
            compression,
            read: Reader::Buffered(read),
            uncompressed: vec![],
            compressed: vec![],
        })
    }

    /// Returns the Rerun version that was used to encode the data in the first place.
    #[inline]
    pub fn version(&self) -> CrateVersion {
        self.version
    }

    /// Peeks ahead in search of additional `FileHeader`s in the stream.
    ///
    /// Returns true if a valid header was found.
    ///
    /// No-op if the decoder wasn't initialized with [`Decoder::new_multiplexed`].
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

            let (version, options) = match read_options(VersionPolicy::Warn, &data) {
                Ok(opts) => opts,
                Err(err) => return Some(Err(err)),
            };
            let compression = options.compression;

            self.version = CrateVersion::max(self.version, version);
            self.compression = compression;
        }

        let header = match MessageHeader::decode(&mut self.read) {
            Ok(header) => header,
            Err(err) => match err {
                DecodeError::Read(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return None;
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
            Ok(re_log_types::LogMsg::SetStoreInfo(mut msg)) => {
                // Propagate the protocol version from the header into the `StoreInfo` so that all
                // parts of the app can easily access it.
                msg.info.store_version = Some(self.version());
                Some(Ok(re_log_types::LogMsg::SetStoreInfo(msg)))
            }
            Ok(msg) => Some(Ok(msg)),
            Err(err) => Some(Err(err.into())),
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(all(feature = "decoder", feature = "encoder"))]
#[test]
fn test_encode_decode() {
    use re_chunk::RowId;
    use re_log_types::{
        ApplicationId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource, Time,
    };

    let rrd_version = CrateVersion::LOCAL;

    let messages = vec![LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: *RowId::new(),
        info: StoreInfo {
            application_id: ApplicationId("test".to_owned()),
            store_id: StoreId::random(StoreKind::Recording),
            cloned_from: None,
            is_official_example: true,
            started: Time::now(),
            store_source: StoreSource::RustSdk {
                rustc_version: String::new(),
                llvm_version: String::new(),
            },
            store_version: Some(rrd_version),
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
        crate::encoder::encode(rrd_version, options, messages.iter(), &mut file).unwrap();

        let decoded_messages = Decoder::new(VersionPolicy::Error, &mut file.as_slice())
            .unwrap()
            .collect::<Result<Vec<LogMsg>, DecodeError>>()
            .unwrap();

        assert_eq!(messages, decoded_messages);
    }
}
