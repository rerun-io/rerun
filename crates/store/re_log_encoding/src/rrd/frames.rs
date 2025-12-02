use crate::rrd::{Decodable, Encodable, OptionsError};

// --- FileHeader ---

pub use re_build_info::CrateVersion; // convenience
pub use re_protos::log_msg::v1alpha1::ext::Compression; // convenience

/// How we serialize the data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Serializer {
    Protobuf = 2,
}

// TODO(cmc): None of these options make sense to have at the global scope and/or in the StreamHeader.
// * Global scope: that makes the decoder stateful in a very bad way (think e.g. about loading
//   specific chunks straight from footer metadata).
// * StreamHeader: both of these are concerns that only apply to message payloads, and should therefore
//   be flags in the MessageHeader.
// In practice I believe both are effectively completely ignored everywhere it matters. They need
// to go away for real though.
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

/// The first frame in an RRD stream.
///
/// During normal operations, there can only be a single [`StreamHeader`] per RRD stream.
///
/// It is possible to break that invariant by concatenating streams using external tools,
/// e.g. by doing something like `cat *.rrd > all_my_recordings.rrd`.
/// Passing that stream back through Rerun tools, e.g. `cat *.rrd | rerun rrd merge > all_my_recordings.rrd`,
/// would once again guarantee that only one header is present though.
/// I.e. that invariant holds as long as one stays within our ecosystem of tools.
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
            // TODO(cmc): the extra heap-alloc and copy could be easily avoided with the
            // introduction of an InMemoryWriter trait or similar. In practice it makes no
            // difference and the cognitive overhead of this crate is already through the roof.
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
            return Err(crate::rrd::CodecError::FrameDecoding(format!(
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

/// The last frame in an RRD stream.
///
/// During normal operations, there can only be a single [`StreamFooter`] per RRD stream.
///
/// It is possible to break that invariant by concatenating streams using external tools,
/// e.g. by doing something like `cat *.rrd > all_my_recordings.rrd`.
/// Passing that stream back through Rerun tools, e.g. `cat *.rrd | rerun rrd merge > all_my_recordings.rrd`,
/// would once again guarantee that only one footer is present though.
/// I.e. that invariant holds as long as one stays within our ecosystem of tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFooter {
    /// Same as the one in the [`StreamHeader`], i.e. [`crate::rrd::RRD_FOURCC`].
    ///
    /// Used to go straight to the footer of an RRD stream ("backwards").
    pub fourcc: [u8; 4], // RRF2

    /// A unique identifier to disambiguate the footer from other frames.
    ///
    /// Always set to [`Self::RRD_IDENTIFIER`].
    pub identifier: [u8; 4], // FOOT

    /// The position in bytes where the serialized [`RrdFooter`] payload starts, excluding the
    /// message header.
    ///
    /// I.e. a transport-level [`RrdFooter`] can be decoded from the bytes at (pseudo-code):
    /// ```text
    /// let start = stream_footer.rrd_footer_byte_offset_from_start_excluding_header;
    /// let end = start + stream_footer.rrd_footer_byte_size_excluding_header;
    /// let bytes = &file[start..end];
    /// let rrd_footer = re_protos::RrdFooter::decode(bytes)?;
    /// let rrd_footer = rrd_footer.to_application()?;
    /// ```
    ///
    /// [`RrdFooter`]: [crate::RrdFooter]
    pub rrd_footer_byte_offset_from_start_excluding_header: u64,

    /// The size in bytes of the serialized [`RrdFooter`] payload, excluding the message header.
    ///
    /// This is guaranteed to be the same value as the `len` found in the associated message
    /// header, but duplicating it here makes it possible for decoders to get everything they
    /// need using a single IO.
    ///
    /// [`RrdFooter`]: [crate::RrdFooter]
    pub rrd_footer_byte_size_excluding_header: u64,

    /// Checksum for the [`RrdFooter`] payload.
    ///
    /// The footer is most often accessed by jumping straight to it, so this is a nice extra safety
    /// to make sure that we didn't just get "lucky" (or unlucky, rather) when jumping around and
    /// parsing random bytes.
    ///
    /// [`RrdFooter`]: [crate::RrdFooter]
    //
    // TODO(cmc): It shouldn't be the job of the StreamFooter to carry checksums for a specific
    // message's payload. All frames should have identifiers and CRCs for both themselves and their
    // payloads, in which case this CRC would belong in the MessageHeader.
    pub crc_excluding_header: u32,
}

impl StreamFooter {
    pub const ENCODED_SIZE_BYTES: usize = 28;
    pub const CRC_SEED: u32 = 7850921; // "RERUN" in base 26 (A=0, Z=25)
    pub const RRD_IDENTIFIER: [u8; 4] = *b"FOOT";

    pub fn new(
        rrd_footer_byte_offset_from_start_excluding_header: u64,
        rrd_footer_byte_size_excluding_header: u64,
        crc_excluding_header: u32,
    ) -> Self {
        Self {
            fourcc: crate::RRD_FOURCC,
            identifier: Self::RRD_IDENTIFIER,
            rrd_footer_byte_offset_from_start_excluding_header,
            rrd_footer_byte_size_excluding_header,
            crc_excluding_header,
        }
    }

    pub fn from_rrd_footer_bytes(
        rrd_footer_byte_offset_from_start_excluding_header: u64,
        rrd_footer_bytes: &[u8],
    ) -> Self {
        let crc_excluding_header = xxhash_rust::xxh32::xxh32(rrd_footer_bytes, Self::CRC_SEED);
        Self {
            fourcc: crate::RRD_FOURCC,
            identifier: Self::RRD_IDENTIFIER,
            rrd_footer_byte_offset_from_start_excluding_header,
            rrd_footer_byte_size_excluding_header: rrd_footer_bytes.len() as u64,
            crc_excluding_header,
        }
    }
}

impl Encodable for StreamFooter {
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, crate::rrd::CodecError> {
        let Self {
            fourcc,
            identifier,
            rrd_footer_byte_offset_from_start_excluding_header,
            rrd_footer_byte_size_excluding_header,
            crc_excluding_header: crc,
        } = self;

        let before = out.len() as u64;

        out.extend_from_slice(fourcc);
        out.extend_from_slice(identifier);
        out.extend_from_slice(&rrd_footer_byte_offset_from_start_excluding_header.to_le_bytes());
        out.extend_from_slice(&rrd_footer_byte_size_excluding_header.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());

        let n = out.len() as u64 - before;
        assert_eq!(Self::ENCODED_SIZE_BYTES as u64, n);

        Ok(n)
    }
}

impl Decodable for StreamFooter {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, crate::rrd::CodecError> {
        if data.len() != Self::ENCODED_SIZE_BYTES {
            return Err(crate::rrd::CodecError::FrameDecoding(format!(
                "invalid StreamFooter length (expected {} but got {})",
                Self::ENCODED_SIZE_BYTES,
                data.len()
            )));
        }

        let to_array_4b = |slice: &[u8]| slice.try_into().expect("always returns an Ok() variant");

        let fourcc: [u8; 4] = to_array_4b(&data[0..4]);
        if fourcc != crate::RRD_FOURCC {
            return Err(crate::rrd::CodecError::FrameDecoding(format!(
                "invalid StreamFooter FourCC (expected {:?} but got {:?})",
                crate::RRD_FOURCC,
                fourcc,
            )));
        }

        let identifier: [u8; 4] = to_array_4b(&data[4..8]);
        if identifier != Self::RRD_IDENTIFIER {
            return Err(crate::rrd::CodecError::FrameDecoding(format!(
                "invalid StreamFooter identifier (expected {:?} but got {:?})",
                Self::RRD_IDENTIFIER,
                identifier,
            )));
        }

        let rrd_footer_byte_offset_from_start_excluding_header =
            u64::from_le_bytes(data[8..16].try_into().expect("cannot fail, checked above"));
        let rrd_footer_byte_size_excluding_header =
            u64::from_le_bytes(data[16..24].try_into().expect("cannot fail, checked above"));
        let crc = u32::from_le_bytes(data[24..28].try_into().expect("cannot fail, checked above"));

        Ok(Self {
            fourcc,
            identifier,
            rrd_footer_byte_offset_from_start_excluding_header,
            rrd_footer_byte_size_excluding_header,
            crc_excluding_header: crc,
        })
    }
}

// --- MessageHeader ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum MessageKind {
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
            return Err(crate::rrd::CodecError::FrameDecoding(format!(
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
                return Err(crate::rrd::CodecError::FrameDecoding(format!(
                    "unknown MessageHeader kind: {kind:?}"
                )));
            }
        };

        let len = u64::from_le_bytes(data[8..16].try_into().expect("cannot fail, checked above"));

        Ok(Self { kind, len })
    }
}
