#[cfg(feature = "decoder")]
pub(crate) mod decoder;
#[cfg(feature = "encoder")]
pub(crate) mod encoder;

#[allow(dead_code)] // used in encoder/decoder behind feature flag
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub(crate) enum MessageKind {
    SetStoreInfo = 1,
    ArrowMsg = 2,
    BlueprintActivationCommand = 3,
    End = 255,
}

#[allow(dead_code)] // used in encoder/decoder behind feature flag
#[derive(Debug, Clone, Copy)]
pub(crate) struct MessageHeader {
    pub(crate) kind: MessageKind,
    pub(crate) len: u32,
}

impl MessageKind {
    #[cfg(feature = "encoder")]
    pub(crate) fn encode(
        &self,
        buf: &mut impl std::io::Write,
    ) -> Result<(), crate::encoder::EncodeError> {
        let kind: u32 = *self as u32;
        buf.write_all(&kind.to_le_bytes())?;
        Ok(())
    }

    #[cfg(feature = "decoder")]
    pub(crate) fn decode(
        data: &mut impl std::io::Read,
    ) -> Result<Self, crate::decoder::DecodeError> {
        let mut buf = [0; 4];
        data.read_exact(&mut buf)?;

        match u32::from_le_bytes(buf) {
            1 => Ok(Self::SetStoreInfo),
            2 => Ok(Self::ArrowMsg),
            3 => Ok(Self::BlueprintActivationCommand),
            255 => Ok(Self::End),
            _ => Err(crate::decoder::DecodeError::Codec(
                crate::codec::CodecError::UnknownMessageHeader,
            )),
        }
    }
}

impl MessageHeader {
    #[cfg(feature = "encoder")]
    pub(crate) fn encode(
        &self,
        buf: &mut impl std::io::Write,
    ) -> Result<(), crate::encoder::EncodeError> {
        self.kind.encode(buf)?;
        buf.write_all(&self.len.to_le_bytes())?;
        Ok(())
    }

    #[cfg(feature = "decoder")]
    pub(crate) fn decode(
        data: &mut impl std::io::Read,
    ) -> Result<Self, crate::decoder::DecodeError> {
        let kind = MessageKind::decode(data)?;
        let mut buf = [0; 4];
        data.read_exact(&mut buf)?;
        let len = u32::from_le_bytes(buf);

        Ok(Self { kind, len })
    }
}
