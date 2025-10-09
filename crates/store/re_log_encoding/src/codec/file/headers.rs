use crate::codec::{Compression, Serializer};

// --- FileHeader ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodingOptions {
    pub compression: Compression,
    pub serializer: Serializer,
}

impl EncodingOptions {
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
                    1 => return Err(OptionsError::RemovedMsgPackSerializer),
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
pub struct FileHeader {
    #[allow(dead_code)] // only used with the "encoder" feature
    pub fourcc: [u8; 4],
    pub version: [u8; 4],
    pub options: EncodingOptions,
}

#[cfg(any(feature = "encoder", feature = "decoder"))]
impl FileHeader {
    #[cfg(feature = "decoder")]
    pub const SIZE: usize = 12;

    #[cfg(feature = "encoder")]
    pub fn encode(&self, write: &mut impl std::io::Write) -> Result<(), crate::EncodeError> {
        write
            .write_all(&self.fourcc)
            .map_err(crate::EncodeError::Write)?;
        write
            .write_all(&self.version)
            .map_err(crate::EncodeError::Write)?;
        write
            .write_all(&self.options.to_bytes())
            .map_err(crate::EncodeError::Write)?;
        Ok(())
    }

    #[cfg(feature = "decoder")]
    pub fn decode(read: &mut impl std::io::Read) -> Result<Self, crate::DecodeError> {
        let to_array_4b = |slice: &[u8]| slice.try_into().expect("always returns an Ok() variant");

        let mut buffer = [0_u8; Self::SIZE];
        read.read_exact(&mut buffer)
            .map_err(crate::DecodeError::Read)?;
        let fourcc = to_array_4b(&buffer[0..4]);

        // Check magic bytes FIRST
        if crate::OLD_RRD_FOURCC.contains(&fourcc) {
            return Err(crate::DecodeError::OldRrdVersion);
        } else if fourcc != crate::RRD_FOURCC {
            return Err(crate::DecodeError::NotAnRrd(crate::NotAnRrdError {
                expected_fourcc: crate::RRD_FOURCC,
                actual_fourcc: fourcc,
            }));
        }

        let version = to_array_4b(&buffer[4..8]);
        let options = EncodingOptions::from_bytes(to_array_4b(&buffer[8..]))?;
        Ok(Self {
            fourcc,
            version,
            options,
        })
    }
}

// --- MessageHeader ---

#[allow(dead_code)] // used behind feature flag
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum MessageKind {
    #[default]
    End = Self::END,
    SetStoreInfo = Self::SET_STORE_INFO,
    ArrowMsg = Self::ARROW_MSG,
    BlueprintActivationCommand = Self::BLUEPRINT_ACTIVATION_COMMAND,
}

#[allow(dead_code)] // used behind feature flag
impl MessageKind {
    const END: u64 = 0;
    const SET_STORE_INFO: u64 = 1;
    const ARROW_MSG: u64 = 2;
    const BLUEPRINT_ACTIVATION_COMMAND: u64 = 3;
}

#[allow(dead_code)] // used behind feature flag
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    pub kind: MessageKind,
    pub len: u64,
}

impl MessageHeader {
    #[allow(dead_code)] // used behind feature flag
    /// Size of an encoded message header, in bytes.
    pub const SIZE_BYTES: usize = 16;

    // NOTE: We use little-endian encoding, because we live in the 21st century.
    #[cfg(feature = "encoder")]
    #[allow(dead_code)] // TODO(cmc): codec revamp
    pub fn encode(&self, buf: &mut impl std::io::Write) -> Result<(), crate::EncodeError> {
        let Self { kind, len } = *self;
        buf.write_all(&(kind as u64).to_le_bytes())?;
        buf.write_all(&len.to_le_bytes())?;
        Ok(())
    }

    #[cfg(feature = "decoder")]
    #[allow(dead_code)] // TODO(cmc): codec revamp
    pub fn decode(data: &mut impl std::io::Read) -> Result<Self, crate::DecodeError> {
        let mut buf = [0; Self::SIZE_BYTES];
        data.read_exact(&mut buf)?;

        Self::from_bytes(&buf)
    }

    /// Decode a message header from a byte buffer. Input buffer must be exactly 16 bytes long.
    /// TODO(zehiko) this should be public, we need to shuffle things around to ensure that #8726
    #[cfg(feature = "decoder")]
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::DecodeError> {
        if buf.len() != Self::SIZE_BYTES {
            return Err(crate::DecodeError::Codec(
                crate::codec::CodecError::HeaderDecoding(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid header length",
                )),
            ));
        }
        #[allow(clippy::unwrap_used)] // cannot fail
        let kind = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let kind = match kind {
            MessageKind::END => MessageKind::End,
            MessageKind::SET_STORE_INFO => MessageKind::SetStoreInfo,
            MessageKind::ARROW_MSG => MessageKind::ArrowMsg,
            MessageKind::BLUEPRINT_ACTIVATION_COMMAND => MessageKind::BlueprintActivationCommand,
            _ => {
                return Err(crate::DecodeError::Codec(
                    crate::codec::CodecError::UnknownMessageHeader,
                ));
            }
        };

        #[allow(clippy::unwrap_used)] // cannot fail
        let len = u64::from_le_bytes(buf[8..16].try_into().unwrap());

        Ok(Self { kind, len })
    }
}
