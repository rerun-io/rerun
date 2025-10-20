use crate::rrd::{Decodable, Encodable, OptionsError};

// TODO: the fact that the RRD format isn't self-synchronising is kind of a PITA, but then again we
// carry arbitrary payloads so that seems par for the course.

// --- FileHeader ---

pub use re_build_info::CrateVersion; // convenience
pub use re_protos::log_msg::v1alpha1::ext::Compression; // convenience

/// How we serialize the data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Serializer {
    Protobuf = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodingOptions {
    pub compression: Compression,
    pub serializer: Serializer,
}

impl EncodingOptions {
    pub const ENCODED_SIZE_BYTES: usize = 4;
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
}

impl Encodable for EncodingOptions {
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, crate::rrd::CodecError> {
        let Self {
            compression,
            serializer,
        } = *self;

        let before = out.len() as u64;

        out.extend_from_slice(&[
            compression as u8,
            serializer as u8,
            0, // reserved
            0, // reserved
        ]);

        let n = out.len() as u64 - before;
        assert_eq!(Self::ENCODED_SIZE_BYTES as u64, n);

        Ok(n)
    }
}

impl Decodable for EncodingOptions {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, crate::rrd::CodecError> {
        match data {
            &[compression, serializer, 0, 0] => {
                let compression = match compression {
                    0 => Compression::Off,
                    1 => Compression::LZ4,
                    _ => return Err(OptionsError::UnknownCompression(compression).into()),
                };
                let serializer = match serializer {
                    1 => return Err(OptionsError::RemovedMsgPackSerializer.into()),
                    2 => Serializer::Protobuf,
                    _ => return Err(OptionsError::UnknownSerializer(serializer).into()),
                };
                Ok(Self {
                    compression,
                    serializer,
                })
            }

            _ => Err(OptionsError::UnknownReservedBytes.into()),
        }
    }
}

// ---

#[derive(Debug, Clone, Copy)]
pub struct StreamHeader {
    pub fourcc: [u8; 4],
    pub version: [u8; 4],
    pub options: EncodingOptions,
}

impl StreamHeader {
    pub const ENCODED_SIZE_BYTES: usize = 12;
}

impl StreamHeader {
    pub fn to_version_and_options(
        self,
    ) -> Result<(CrateVersion, EncodingOptions), crate::rrd::CodecError> {
        {
            // We used 0000 for all .rrd files up until 2023-02-27, post 0.2.0 release:
            let encoded_version = if self.version == [0, 0, 0, 0] {
                CrateVersion::new(0, 2, 0)
            } else {
                CrateVersion::from_bytes(self.version)
            };

            if encoded_version.major == 0 && encoded_version.minor < 23 {
                // We broke compatibility for 0.23 for (hopefully) the last time.
                return Err(crate::rrd::CodecError::IncompatibleRerunVersion {
                    file: Box::new(encoded_version),
                    local: Box::new(CrateVersion::LOCAL),
                });
            } else if encoded_version <= CrateVersion::LOCAL {
                // Loading old files should be fine, and if it is not, the chunk migration in re_sorbet should already log a warning.
            } else {
                re_log::warn_once!(
                    "Found data stream with Rerun version {encoded_version} which is newer than the local Rerun version ({}). This file may contain data that is not compatible with this version of Rerun. Consider updating Rerun.",
                    CrateVersion::LOCAL
                );
            }
        }

        Ok((CrateVersion::from_bytes(self.version), self.options))
    }
}

impl Encodable for StreamHeader {
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, crate::rrd::CodecError> {
        let Self {
            fourcc,
            version,
            options,
        } = self;

        let before = out.len() as u64;

        out.extend_from_slice(fourcc);
        out.extend_from_slice(version);
        {
            // TODO(cmc): the extra heap-alloc and copy could be easily avoided.
            let mut options_out = Vec::new();
            options.to_rrd_bytes(&mut options_out)?;
            out.extend_from_slice(&options_out);
        }

        let n = out.len() as u64 - before;
        assert_eq!(Self::ENCODED_SIZE_BYTES as u64, n);

        Ok(n)
    }
}

impl Decodable for StreamHeader {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, crate::rrd::CodecError> {
        if data.len() != Self::ENCODED_SIZE_BYTES {
            return Err(crate::rrd::CodecError::HeaderDecoding(format!(
                "invalid StreamHeader length (expected {} but got {})",
                Self::ENCODED_SIZE_BYTES,
                data.len()
            )));
        }

        let to_array_4b = |slice: &[u8]| slice.try_into().expect("always returns an Ok() variant");

        let fourcc = to_array_4b(&data[0..4]);

        // Check magic bytes FIRST
        if crate::rrd::OLD_RRD_FOURCC.contains(&fourcc) {
            return Err(crate::rrd::CodecError::OldRrdVersion);
        } else if fourcc != crate::rrd::RRD_FOURCC {
            return Err(crate::rrd::CodecError::NotAnRrd(
                crate::rrd::NotAnRrdError {
                    expected_fourcc: crate::rrd::RRD_FOURCC,
                    actual_fourcc: fourcc,
                },
            ));
        }

        let version = to_array_4b(&data[4..8]);
        let options = EncodingOptions::from_rrd_bytes(&data[8..])?;
        Ok(Self {
            fourcc,
            version,
            options,
        })
    }
}

// ---

// TODO: let's try to put a footer, any footer, and see what happens, i guess

// TODO: im starting to think that we need both a FooterStart and FooterEnd kinda thing, actually:
// * one when moving forward through a stream, so the decoder can have a clue wtf is going on
// * one when going straight for the footer

// TODO: let's not fuck this up and have a magic value from the start in there (RRZ2?)
//
// The best thing possible would be to follow the classic `[MAGIC][LENGTH][PAYLOAD...][CRC]`
// structure everywhere, but not sure if we can do so in a bw way everywhere (RRF3?).

// TODO: the fact that we do not track the format version in redap shouldnt even be a problem since
// we just assume the chunk keys to point to ArrowMsgs directly anyway.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFooter {
    pub len: u64,
    // TODO: crc in the header seems a bit unorthodox, but do we care?
    pub crc: u32,
}

impl StreamFooter {
    pub const ENCODED_SIZE_BYTES: usize = 16;
    pub const FOURCC: [u8; 4] = *b"RRSF";
}

impl Encodable for StreamFooter {
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, crate::rrd::CodecError> {
        let Self { len, crc } = self;

        let before = out.len() as u64;

        out.extend_from_slice(&Self::FOURCC);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());

        let n = out.len() as u64 - before;
        assert_eq!(Self::ENCODED_SIZE_BYTES as u64, n);

        Ok(n)
    }
}

impl Decodable for StreamFooter {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, crate::rrd::CodecError> {
        if data.len() != Self::ENCODED_SIZE_BYTES {
            return Err(crate::rrd::CodecError::FooterDecoding(format!(
                "invalid StreamFooter length (expected {} but got {})",
                Self::ENCODED_SIZE_BYTES,
                data.len()
            )));
        }

        let to_array_4b = |slice: &[u8]| slice.try_into().expect("always returns an Ok() variant");

        let fourcc: [u8; 4] = to_array_4b(&data[0..4]);
        if fourcc != Self::FOURCC {
            return Err(crate::rrd::CodecError::FooterDecoding(format!(
                "invalid StreamFooter FourCC (expected {:?} but got {:?})",
                Self::FOURCC,
                fourcc,
            )));
        }

        let len = u64::from_le_bytes(data[4..12].try_into().expect("cannot fail, checked above"));
        let crc = u32::from_le_bytes(data[12..16].try_into().expect("cannot fail, checked above"));

        Ok(Self { len, crc })
    }
}

// --- MessageHeader ---

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum MessageKind {
    #[default]
    End = Self::END,
    SetStoreInfo = Self::SET_STORE_INFO,
    ArrowMsg = Self::ARROW_MSG,
    BlueprintActivationCommand = Self::BLUEPRINT_ACTIVATION_COMMAND,
}

impl MessageKind {
    const END: u64 = 0;
    const SET_STORE_INFO: u64 = 1;
    const ARROW_MSG: u64 = 2;
    const BLUEPRINT_ACTIVATION_COMMAND: u64 = 3;
}

// TODO: if we did have magic bytes for some various headers, that would also avoid some hacks in
// the decoder itself (maybe remove the need for peeking altogether?).
// And it would certainly avoid introducing even more hacks to differentiate between a stream
// header and a stream footer when a messageheader fails to decode.

// TODO: we definitely do not need 64bits of `kind` here, so we could retrofit a simple magic value
// in there. it's not foolproof in and of itself, but combined with everything else it almost is,
// and would make the RRD format self-synchronising. That would ofc be a breaking change but it's
// certainly possible to make it bw compatible somehow.
// I should really open an issue for this.
//
// This is especially useful because the Compression field in the header is useless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    pub kind: MessageKind,
    pub len: u64,
}

impl MessageHeader {
    pub const ENCODED_SIZE_BYTES: usize = 16;
}

impl Encodable for MessageHeader {
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, crate::rrd::CodecError> {
        let Self { kind, len } = *self;

        let before = out.len() as u64;

        // TODO: nooooooooooooo, i forgot this is all little endian 😭 that means we cannot
        // possibly make this bw compatible, we're fucked.
        // we can duplicate the state machine for RRF3, none of the decoders on top change, it's
        // fine. And if we go there then... well we can do whatever the fuck we want really.
        //
        // Begs the question: what would we want to do if there was no baggage?
        out.extend_from_slice(&(kind as u64).to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());

        let n = out.len() as u64 - before;
        assert_eq!(Self::ENCODED_SIZE_BYTES as u64, n);

        Ok(n)
    }
}

impl Decodable for MessageHeader {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, crate::rrd::CodecError> {
        if data.len() != Self::ENCODED_SIZE_BYTES {
            return Err(crate::rrd::CodecError::HeaderDecoding(format!(
                "invalid MessageHeader length (expected {} but got {})",
                Self::ENCODED_SIZE_BYTES,
                data.len()
            )));
        }

        let kind = u64::from_le_bytes(data[0..8].try_into().expect("cannot fail, checked above"));
        let kind = match kind {
            MessageKind::END => MessageKind::End,
            MessageKind::SET_STORE_INFO => MessageKind::SetStoreInfo,
            MessageKind::ARROW_MSG => MessageKind::ArrowMsg,
            MessageKind::BLUEPRINT_ACTIVATION_COMMAND => MessageKind::BlueprintActivationCommand,
            _ => {
                return Err(crate::rrd::CodecError::HeaderDecoding(format!(
                    "unknown MessageHeader kind: {kind:?}"
                )));
            }
        };

        let len = u64::from_le_bytes(data[8..16].try_into().expect("cannot fail, checked above"));

        Ok(Self { kind, len })
    }
}
