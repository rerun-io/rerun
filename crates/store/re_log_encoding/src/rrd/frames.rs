// --- FileHeader ---
pub use re_build_info::CrateVersion; // convenience
pub use re_protos::common::v1alpha1::ext::Compression;

use crate::rrd::{Decodable, Encodable, OptionsError}; // convenience

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

/// The opening frame in an RRD stream.
///
/// During normal operations, there can only be a single [`StreamHeader`] per RRD stream.
/// It is possible to break that invariant by concatenating streams using external tools,
/// e.g. by doing something like `cat *.rrd > all_my_recordings.rrd`.
/// Passing that stream back through Rerun tools, e.g. `cat *.rrd | rerun rrd merge > all_my_recordings.rrd`,
/// would once again guarantee that only one stream header is present though.
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

/// The closing frame in an RRD stream. Keeps track of where the [`RrdFooter`]s can be found.
///
/// During normal operations, there can only be a single [`StreamFooter`] per RRD stream.
/// It is possible to break that invariant by concatenating streams using external tools,
/// e.g. by doing something like `cat *.rrd > all_my_recordings.rrd`.
/// Passing that stream back through Rerun tools, e.g. `cat *.rrd | rerun rrd merge > all_my_recordings.rrd`,
/// would once again guarantee that only one stream footer is present though.
/// I.e. that invariant holds as long as one stays within our ecosystem of tools.
///
/// [`RrdFooter`]: [crate::RrdFooter]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamFooter {
    /// Same as the one in the [`StreamHeader`], i.e. [`crate::rrd::RRD_FOURCC`].
    ///
    /// Used to go straight to the footer of an RRD stream ("backwards").
    pub fourcc: [u8; 4], // RRF2

    /// A unique identifier to disambiguate the footer from other frames.
    ///
    /// Always set to [`Self::RRD_IDENTIFIER`].
    pub identifier: [u8; 4], // FOOT

    /// One entry per RRD footer that is pointed at from this stream footer.
    ///
    /// The stream footer supports pointing to an arbitrary number of RRD footers.
    /// This variable length allows for incremental RRD manifests, where the manifest for a single
    /// recording gets built and logged in many chunks rather than all at once when the sink
    /// finally shuts down.
    ///
    /// We do not leverage that feature today, and so the number of entries is always 1 in practice.
    /// Having this already setup this way means that we will be able to support it in the future without
    /// having to break the framing protocol (i.e. going from `RRF2/FOOT` to `RRF3/FOOT`).
    pub entries: Vec<StreamFooterEntry>,
}

/// One specific entry in the [`StreamFooter`]. Each entry corresponds to an RRD footer.
///
/// As of today, there is always one and exactly one entry per [`StreamFooter`], see
/// [`StreamFooter::entries`] for more information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFooterEntry {
    /// The span in bytes where the serialized [`RrdFooter`] payload starts end ends, excluding
    /// the message header.
    ///
    /// I.e. a transport-level [`RrdFooter`] can be decoded from the bytes at (pseudo-code):
    /// ```text
    /// let start = rrd_footer_byte_span_from_start_excluding_header.start;
    /// let end = rrd_footer_byte_span_from_start_excluding_header.end();
    /// let bytes = &file[start..end];
    /// let rrd_footer = re_protos::RrdFooter::decode(bytes)?;
    /// let rrd_footer = rrd_footer.to_application()?;
    /// ```
    ///
    /// [`RrdFooter`]: [crate::RrdFooter]
    pub rrd_footer_byte_span_from_start_excluding_header: re_span::Span<u64>,

    /// Checksum for the [`RrdFooter`] payload.
    ///
    /// The footer is most often accessed by jumping straight to it, so this is a nice extra safety
    /// to make sure that we didn't just get "lucky" (or unlucky, rather) when jumping around and
    /// parsing random bytes.
    ///
    /// For now, the checksum algorithm is hardcoded to `xxh32`, the 32bit variant of the
    /// [`xxhash` family of hashing algorithms](https://xxhash.com/).
    /// This a is fast, HW-accelerated, non-cryptographic hash that is perfect for hashing
    /// RRD footers, which can potentially get very, very large.
    ///
    /// [`RrdFooter`]: [crate::RrdFooter]
    //
    // TODO(cmc): It shouldn't be the job of the StreamFooter to carry checksums for a specific
    // message's payload. All frames should have identifiers and CRCs for both themselves and their
    // payloads, in which case this CRC would belong in the MessageHeader.
    // TODO(cmc): In a potential future RRF3, we might make the choice of checksum algorithm
    // configurable via flag.
    pub crc_excluding_header: u32,
}

impl StreamFooter {
    /// The encoded size in bytes of a [`StreamFooter`].
    ///
    /// While [`StreamFooter`]s are technically of variable length, they always contain one and
    /// exactly one entry as of today. This value represents that.
    pub const ENCODED_SIZE_BYTES: usize =
        Self::ENCODED_SIZE_BYTES_IGNORING_ENTRIES + Self::ENCODED_SIZE_BYTES_SINGLE_ENTRY;

    const ENCODED_SIZE_BYTES_IGNORING_ENTRIES: usize = 12;
    const ENCODED_SIZE_BYTES_SINGLE_ENTRY: usize = 20;

    pub const CRC_SEED: u32 = 7850921; // "RERUN" in base 26 (A=0, Z=25)
    pub const RRD_IDENTIFIER: [u8; 4] = *b"FOOT";

    pub fn new(
        rrd_footer_byte_span_from_start_excluding_header: re_span::Span<u64>,
        crc_excluding_header: u32,
    ) -> Self {
        Self {
            fourcc: crate::RRD_FOURCC,
            identifier: Self::RRD_IDENTIFIER,
            entries: vec![StreamFooterEntry {
                rrd_footer_byte_span_from_start_excluding_header,
                crc_excluding_header,
            }],
        }
    }

    pub fn from_rrd_footer_bytes(
        rrd_footer_byte_offset_from_start_excluding_header: u64,
        rrd_footer_bytes: &[u8],
    ) -> Self {
        let crc_excluding_header = Self::compute_crc(rrd_footer_bytes);
        let rrd_footer_byte_span_from_start_excluding_header = re_span::Span {
            start: rrd_footer_byte_offset_from_start_excluding_header,
            len: rrd_footer_bytes.len() as u64,
        };
        Self {
            fourcc: crate::RRD_FOURCC,
            identifier: Self::RRD_IDENTIFIER,
            entries: vec![StreamFooterEntry {
                rrd_footer_byte_span_from_start_excluding_header,
                crc_excluding_header,
            }],
        }
    }

    pub fn compute_crc(rrd_footer_bytes_excluding_header: &[u8]) -> u32 {
        xxhash_rust::xxh32::xxh32(rrd_footer_bytes_excluding_header, Self::CRC_SEED)
    }
}

impl Encodable for StreamFooter {
    fn to_rrd_bytes(&self, out: &mut Vec<u8>) -> Result<u64, crate::rrd::CodecError> {
        let Self {
            fourcc,
            identifier,
            entries,
        } = self;

        let num_rrd_footers = entries.len() as u32;

        let before = out.len() as u64;

        // We start with the entries first, so that the static part of the stream footer can always
        // be accessed randomly using a fixed offset from the end of the file.
        for entry in entries {
            out.extend_from_slice(
                &entry
                    .rrd_footer_byte_span_from_start_excluding_header
                    .start
                    .to_le_bytes(),
            );
            out.extend_from_slice(
                &entry
                    .rrd_footer_byte_span_from_start_excluding_header
                    .len
                    .to_le_bytes(),
            );
            out.extend_from_slice(&entry.crc_excluding_header.to_le_bytes());
        }

        out.extend_from_slice(fourcc);
        out.extend_from_slice(identifier);
        out.extend_from_slice(&num_rrd_footers.to_le_bytes());

        let n = out.len() as u64 - before;
        assert_eq!(
            Self::ENCODED_SIZE_BYTES as u64,
            n,
            "Stream footers always point to a single RRD footer at the moment"
        );

        Ok(n)
    }
}

impl Decodable for StreamFooter {
    fn from_rrd_bytes(data: &[u8]) -> Result<Self, crate::rrd::CodecError> {
        if data.len() < Self::ENCODED_SIZE_BYTES {
            return Err(crate::rrd::CodecError::FrameDecoding(format!(
                "invalid StreamFooter length (expected {} but got {})",
                Self::ENCODED_SIZE_BYTES,
                data.len()
            )));
        }

        let fixed_data = &data[data.len() - Self::ENCODED_SIZE_BYTES_IGNORING_ENTRIES..];

        let to_array_4b = |slice: &[u8]| slice.try_into().expect("always returns an Ok() variant");

        let fourcc: [u8; 4] = to_array_4b(&fixed_data[0..4]);
        if fourcc != crate::RRD_FOURCC {
            return Err(crate::rrd::CodecError::FrameDecoding(format!(
                "invalid StreamFooter FourCC (expected {:?} but got {:?})",
                crate::RRD_FOURCC,
                fourcc,
            )));
        }

        let identifier: [u8; 4] = to_array_4b(&fixed_data[4..8]);
        if identifier != Self::RRD_IDENTIFIER {
            return Err(crate::rrd::CodecError::FrameDecoding(format!(
                "invalid StreamFooter identifier (expected {:?} but got {:?})",
                Self::RRD_IDENTIFIER,
                identifier,
            )));
        }

        let num_rrd_footers = u32::from_le_bytes(
            fixed_data[8..12]
                .try_into()
                .expect("cannot fail, checked above"),
        );

        let dynamic_data = &data[data.len() - Self::ENCODED_SIZE_BYTES..];

        let mut pos = 0;
        let entries = (0..num_rrd_footers)
            .map(|_| {
                let rrd_footer_byte_span_from_start_excluding_header = re_span::Span {
                    start: u64::from_le_bytes(
                        dynamic_data[pos..pos + 8]
                            .try_into()
                            .expect("cannot fail, checked above"),
                    ),
                    len: u64::from_le_bytes(
                        dynamic_data[pos + 8..pos + 16]
                            .try_into()
                            .expect("cannot fail, checked above"),
                    ),
                };

                let crc_excluding_header = u32::from_le_bytes(
                    dynamic_data[pos + 16..pos + 20]
                        .try_into()
                        .expect("cannot fail, checked above"),
                );

                pos += Self::ENCODED_SIZE_BYTES_SINGLE_ENTRY;

                StreamFooterEntry {
                    rrd_footer_byte_span_from_start_excluding_header,
                    crc_excluding_header,
                }
            })
            .collect();

        Ok(Self {
            fourcc,
            identifier,
            entries,
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
