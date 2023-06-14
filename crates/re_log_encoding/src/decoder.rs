//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

pub mod stream;

use std::io::Read;

use re_log_types::LogMsg;

use crate::{Compression, EncodingOptions, Serializer};

// ----------------------------------------------------------------------------

fn warn_on_version_mismatch(encoded_version: [u8; 4]) {
    use re_build_info::CrateVersion;

    // We used 0000 for all .rrd files up until 2023-02-27, post 0.2.0 release:
    let encoded_version = if encoded_version == [0, 0, 0, 0] {
        CrateVersion::new(0, 2, 0)
    } else {
        CrateVersion::from_bytes(encoded_version)
    };

    let local_version = CrateVersion::parse(env!("CARGO_PKG_VERSION"));

    if !encoded_version.is_compatible_with(local_version) {
        re_log::warn!("Found log stream with Rerun version {encoded_version}, which is incompatible with the local Rerun version {local_version}. Loading will try to continue, but might fail in subtle ways.");
    }
}

// ----------------------------------------------------------------------------

/// On failure to encode or serialize a [`LogMsg`].
#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    #[error("Not an .rrd file")]
    NotAnRrd,

    #[error("Found an .rrd file from a Rerun version from 0.5.1 or earlier")]
    OldRrdVersion,

    #[error("Failed to decode the options: {0}")]
    Options(#[from] crate::OptionsError),

    #[error("Failed to read: {0}")]
    Read(std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4(std::io::Error),

    #[error("MsgPack error: {0}")]
    MsgPack(#[from] rmp_serde::decode::Error),
}

// ----------------------------------------------------------------------------

pub fn decode_bytes(bytes: &[u8]) -> Result<Vec<LogMsg>, DecodeError> {
    let decoder = Decoder::new(std::io::Cursor::new(bytes))?;
    let mut msgs = vec![];
    for msg in decoder {
        msgs.push(msg?);
    }
    Ok(msgs)
}

// ----------------------------------------------------------------------------

enum Decompressor<R: std::io::Read> {
    Uncompressed(R),
    Lz4(lz4_flex::frame::FrameDecoder<R>),
}

impl<R: std::io::Read> Decompressor<R> {
    fn new(compression: Compression, read: R) -> Self {
        match compression {
            Compression::Off => Self::Uncompressed(read),
            Compression::LZ4 => Self::Lz4(lz4_flex::frame::FrameDecoder::new(read)),
        }
    }

    /// Gets a mutable reference to the underlying reader in this decompressor
    fn get_mut(&mut self) -> &mut R {
        match self {
            Decompressor::Uncompressed(read) => read,
            Decompressor::Lz4(lz4) => lz4.get_mut(),
        }
    }

    /* pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), DecodeError> {
        use std::io::Read as _;

        match self {
            Decompressor::Uncompressed(read) => read.read_exact(buf).map_err(DecodeError::Read),
            Decompressor::Lz4(lz4) => lz4.read_exact(buf).map_err(DecodeError::Lz4),
        }
    } */
}

impl<R: std::io::Read> Read for Decompressor<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Decompressor::Uncompressed(read) => read.read(buf),
            Decompressor::Lz4(lz4) => lz4.read(buf),
        }
    }
}

// ----------------------------------------------------------------------------

pub struct Decoder<R: std::io::Read> {
    decompressor: Decompressor<R>,
    buffer: Vec<u8>,
}

const STREAM_HEADER_SIZE: usize = 12;
const MESSAGE_HEADER_SIZE: usize = 8;

pub fn read_options(bytes: &[u8; STREAM_HEADER_SIZE]) -> Result<EncodingOptions, DecodeError> {
    let mut read = std::io::Cursor::new(bytes);

    {
        let mut magic = [0_u8; 4];
        read.read_exact(&mut magic).map_err(DecodeError::Read)?;
        if &magic == b"RRF0" {
            return Err(DecodeError::OldRrdVersion);
        } else if &magic != crate::RRD_HEADER {
            return Err(DecodeError::NotAnRrd);
        }
    }

    {
        let mut version_bytes = [0_u8; 4];
        read.read_exact(&mut version_bytes)
            .map_err(DecodeError::Read)?;
        warn_on_version_mismatch(version_bytes);
    }

    let options = {
        let mut options_bytes = [0_u8; 4];
        read.read_exact(&mut options_bytes)
            .map_err(DecodeError::Read)?;
        EncodingOptions::from_bytes(options_bytes)?
    };

    match options.serializer {
        Serializer::MsgPack => {}
    }

    Ok(options)
}

impl<R: std::io::Read> Decoder<R> {
    pub fn new(mut read: R) -> Result<Self, DecodeError> {
        re_tracing::profile_function!();

        let mut data = [0_u8; STREAM_HEADER_SIZE];
        read.read_exact(&mut data).map_err(DecodeError::Read)?;
        let options = read_options(&data)?;

        Ok(Self {
            decompressor: Decompressor::new(options.compression, read),
            buffer: vec![],
        })
    }
}

impl<R: std::io::Read> Iterator for Decoder<R> {
    type Item = Result<LogMsg, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        re_tracing::profile_function!();

        let mut len = [0_u8; MESSAGE_HEADER_SIZE];
        self.decompressor.get_mut().read_exact(&mut len).ok()?;
        let len = u64::from_le_bytes(len) as usize;

        self.buffer.resize(len, 0);

        {
            re_tracing::profile_scope!("lz4");
            if let Err(err) = self.decompressor.read_exact(&mut self.buffer) {
                return Some(Err(DecodeError::Read(err)));
            }
        }

        re_tracing::profile_scope!("MsgPack deser");
        match rmp_serde::from_read(&mut self.buffer.as_slice()) {
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
        row_id: RowId::random(),
        info: StoreInfo {
            application_id: ApplicationId("test".to_owned()),
            store_id: StoreId::random(StoreKind::Recording),
            is_official_example: true,
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

        let decoded_messages = Decoder::new(&mut file.as_slice())
            .unwrap()
            .collect::<Result<Vec<LogMsg>, DecodeError>>()
            .unwrap();

        assert_eq!(messages, decoded_messages);
    }
}
