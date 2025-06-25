#[cfg(feature = "decoder")]
pub mod decoder;
#[cfg(feature = "encoder")]
pub mod encoder;

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
#[derive(Debug, Clone, Copy)]
pub(crate) struct MessageHeader {
    pub(crate) kind: MessageKind,
    pub(crate) len: u64,
}

impl MessageHeader {
    #[allow(dead_code)] // used behind feature flag
    /// Size of an encoded message header, in bytes.
    pub const SIZE_BYTES: usize = 16;

    // NOTE: We use little-endian encoding, because we live in
    //       the 21st century.
    #[cfg(feature = "encoder")]
    pub(crate) fn encode(
        &self,
        buf: &mut impl std::io::Write,
    ) -> Result<(), crate::encoder::EncodeError> {
        let Self { kind, len } = *self;
        buf.write_all(&(kind as u64).to_le_bytes())?;
        buf.write_all(&len.to_le_bytes())?;
        Ok(())
    }

    #[cfg(feature = "decoder")]
    pub(crate) fn decode(
        data: &mut impl std::io::Read,
    ) -> Result<Self, crate::decoder::DecodeError> {
        let mut buf = [0; Self::SIZE_BYTES];
        data.read_exact(&mut buf)?;

        Self::from_bytes(&buf)
    }

    /// Decode a message header from a byte buffer. Input buffer must be exactly 16 bytes long.
    /// TODO(zehiko) this should be public, we need to shuffle things around to ensure that #8726
    #[cfg(feature = "decoder")]
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::decoder::DecodeError> {
        if buf.len() != Self::SIZE_BYTES {
            return Err(crate::decoder::DecodeError::Codec(
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
                return Err(crate::decoder::DecodeError::Codec(
                    crate::codec::CodecError::UnknownMessageHeader,
                ));
            }
        };

        #[allow(clippy::unwrap_used)] // cannot fail
        let len = u64::from_le_bytes(buf[8..16].try_into().unwrap());

        Ok(Self { kind, len })
    }
}
