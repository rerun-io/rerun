//! Crate that handles encoding of rerun log types.

#[cfg(feature = "decoder")]
pub mod decoder;

#[cfg(feature = "encoder")]
pub mod encoder;

pub mod codec;

pub mod protobuf_conversions;

#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
mod file_sink;

#[cfg(feature = "stream_from_http")]
pub mod stream_rrd_from_http;

// ---------------------------------------------------------------------

#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
pub use file_sink::{FileSink, FileSinkError};

// ----------------------------------------------------------------------------

#[cfg(any(feature = "encoder", feature = "decoder"))]
const RRD_HEADER: &[u8; 4] = b"RRF2";

#[cfg(feature = "decoder")]
const OLD_RRD_HEADERS: &[[u8; 4]] = &[*b"RRF0", *b"RRF1"];

// ----------------------------------------------------------------------------

/// Compression format used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Compression {
    Off = 0,

    /// Very fast compression and decompression, but not very good compression ratio.
    LZ4 = 1,
}

/// How we serialize the data
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Serializer {
    MsgPack = 1,
    Protobuf = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodingOptions {
    pub compression: Compression,
    pub serializer: Serializer,
}

impl EncodingOptions {
    pub const MSGPACK_UNCOMPRESSED: Self = Self {
        compression: Compression::Off,
        serializer: Serializer::MsgPack,
    };
    pub const MSGPACK_COMPRESSED: Self = Self {
        compression: Compression::LZ4,
        serializer: Serializer::MsgPack,
    };
    pub const PROTOBUF_COMPRESSED: Self = Self {
        compression: Compression::LZ4,
        serializer: Serializer::Protobuf,
    };
    pub const PROTOBUF_UNCOMPRESSED: Self = Self {
        compression: Compression::Off,
        serializer: Serializer::Protobuf,
    };

    pub fn from_bytes(bytes: [u8; 4]) -> Result<Self, OptionsError> {
        match bytes {
            [compression, serializer, 0, 0] => {
                let compression = match compression {
                    0 => Compression::Off,
                    1 => Compression::LZ4,
                    _ => return Err(OptionsError::UnknownCompression(compression)),
                };
                let serializer = match serializer {
                    1 => Serializer::MsgPack,
                    2 => Serializer::Protobuf,
                    _ => return Err(OptionsError::UnknownSerializer(serializer)),
                };
                Ok(Self {
                    compression,
                    serializer,
                })
            }
            _ => Err(OptionsError::UnknownReservedBytes),
        }
    }

    pub fn to_bytes(self) -> [u8; 4] {
        [
            self.compression as u8,
            self.serializer as u8,
            0, // reserved
            0, // reserved
        ]
    }
}

/// On failure to decode [`EncodingOptions`]
#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum OptionsError {
    #[error("Reserved bytes not zero")]
    UnknownReservedBytes,

    #[error("Unknown compression: {0}")]
    UnknownCompression(u8),

    // TODO(jan): Remove this at some point, realistically 1-2 releases after 0.23
    #[error(
        "You are trying to load an old .rrd file that's not supported by this version of Rerun."
    )]
    RemovedMsgPackSerializer,

    #[error("Unknown serializer: {0}")]
    UnknownSerializer(u8),
}

#[cfg(any(feature = "encoder", feature = "decoder"))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct FileHeader {
    pub magic: [u8; 4],
    pub version: [u8; 4],
    pub options: EncodingOptions,
}

#[cfg(any(feature = "encoder", feature = "decoder"))]
impl FileHeader {
    #[cfg(feature = "decoder")]
    pub const SIZE: usize = 12;

    #[cfg(feature = "encoder")]
    pub fn encode(&self, write: &mut impl std::io::Write) -> Result<(), encoder::EncodeError> {
        write
            .write_all(&self.magic)
            .map_err(encoder::EncodeError::Write)?;
        write
            .write_all(&self.version)
            .map_err(encoder::EncodeError::Write)?;
        write
            .write_all(&self.options.to_bytes())
            .map_err(encoder::EncodeError::Write)?;
        Ok(())
    }

    #[cfg(feature = "decoder")]
    pub fn decode(read: &mut impl std::io::Read) -> Result<Self, decoder::DecodeError> {
        let to_array_4b = |slice: &[u8]| slice.try_into().expect("always returns an Ok() variant");

        let mut buffer = [0_u8; Self::SIZE];
        read.read_exact(&mut buffer)
            .map_err(decoder::DecodeError::Read)?;
        let magic = to_array_4b(&buffer[0..4]);
        let version = to_array_4b(&buffer[4..8]);
        let options = EncodingOptions::from_bytes(to_array_4b(&buffer[8..]))?;
        Ok(Self {
            magic,
            version,
            options,
        })
    }
}

#[cfg(any(feature = "encoder", feature = "decoder"))]
#[derive(Clone, Copy)]
pub(crate) enum MessageHeader {
    Data {
        /// `compressed_len` is equal to `uncompressed_len` for uncompressed streams
        compressed_len: u32,
        uncompressed_len: u32,
    },
    EndOfStream,
}

#[cfg(any(feature = "encoder", feature = "decoder"))]
impl MessageHeader {
    #[cfg(feature = "decoder")]
    pub const SIZE: usize = 8;

    #[cfg(feature = "encoder")]
    pub fn encode(&self, write: &mut impl std::io::Write) -> Result<(), encoder::EncodeError> {
        match self {
            Self::Data {
                compressed_len,
                uncompressed_len,
            } => {
                write
                    .write_all(&compressed_len.to_le_bytes())
                    .map_err(encoder::EncodeError::Write)?;
                write
                    .write_all(&uncompressed_len.to_le_bytes())
                    .map_err(encoder::EncodeError::Write)?;
            }
            Self::EndOfStream => {
                write
                    .write_all(&0_u64.to_le_bytes())
                    .map_err(encoder::EncodeError::Write)?;
            }
        }
        Ok(())
    }

    #[cfg(feature = "decoder")]
    pub fn decode(read: &mut impl std::io::Read) -> Result<Self, decoder::DecodeError> {
        let mut buffer = [0_u8; Self::SIZE];
        read.read_exact(&mut buffer)
            .map_err(decoder::DecodeError::Read)?;

        Self::from_bytes(&buffer)
    }

    /// Decode a message header from a byte buffer. Input buffer must be exactly 8 bytes long.
    #[cfg(feature = "decoder")]
    pub fn from_bytes(data: &[u8]) -> Result<Self, decoder::DecodeError> {
        if data.len() != 8 {
            return Err(decoder::DecodeError::Codec(
                codec::CodecError::HeaderDecoding(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid header length",
                )),
            ));
        }

        fn u32_from_le_slice(bytes: &[u8]) -> u32 {
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        }

        if u32_from_le_slice(&data[0..4]) == 0 && u32_from_le_slice(&data[4..]) == 0 {
            Ok(Self::EndOfStream)
        } else {
            let compressed = u32_from_le_slice(&data[0..4]);
            let uncompressed = u32_from_le_slice(&data[4..]);
            Ok(Self::Data {
                compressed_len: compressed,
                uncompressed_len: uncompressed,
            })
        }
    }
}
