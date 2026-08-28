//! Turning annex-b access units into what `VideoToolbox` wants: the parameter sets a
//! `CMVideoFormatDescription` is built from, and the picture NALs length-prefixed.
//!
//! Pure CPU code with no `VideoToolbox` types in it, so it is unit-tested on all platforms.

use crate::{Codec, ParseError};

/// The parameter sets of one stream, in the order the format description takes them.
#[derive(Default, PartialEq, Eq)]
pub struct ParameterSets {
    /// H.265 only.
    pub vps: Option<Vec<u8>>,

    pub sps: Option<Vec<u8>>,
    pub pps: Option<Vec<u8>>,
}

impl ParameterSets {
    /// The sets in the order `CMVideoFormatDescriptionCreateFrom*ParameterSets` expects,
    /// `None` while any required one is still missing.
    pub fn in_order(&self) -> Option<Vec<&[u8]>> {
        let mut sets = Vec::with_capacity(3);
        if let Some(vps) = &self.vps {
            sets.push(vps.as_slice());
        }
        sets.push(self.sps.as_deref()?);
        sets.push(self.pps.as_deref()?);
        Some(sets)
    }
}

/// What one access unit contributes: updated parameter sets, and its picture data.
pub struct AccessUnit {
    /// The picture NALs, each prefixed with its 4-byte big-endian length.
    pub sample_data: Vec<u8>,

    /// The access unit starts a new group of pictures and can be decoded on its own.
    pub is_random_access: bool,

    /// A parameter set in this access unit differs from the one before it,
    /// so the format description and the session have to be rebuilt.
    pub parameters_changed: bool,
}

/// Splits access units of one codec, tracking the parameter sets seen so far.
pub struct AccessUnitSplitter {
    codec: Codec,
    parameters: ParameterSets,

    /// `cros-codecs` keeps the parsed parameter sets, so the H.265 parser is kept
    /// across access units. Unused for H.264, whose sequence parameter sets parse
    /// on their own.
    h265_parser: cros_codecs::codec::h265::parser::Parser,

    reorder_depth: usize,
}

impl AccessUnitSplitter {
    pub fn new(codec: Codec) -> Self {
        Self {
            codec,
            parameters: ParameterSets::default(),
            h265_parser: cros_codecs::codec::h265::parser::Parser::default(),
            reorder_depth: 0,
        }
    }

    pub fn parameters(&self) -> &ParameterSets {
        &self.parameters
    }

    /// `max_num_reorder_frames` of the sequence parameter set seen last.
    pub fn reorder_depth(&self) -> usize {
        self.reorder_depth
    }

    /// The number of length prefix bytes the sample data uses.
    pub const NAL_LENGTH_SIZE: i32 = 4;

    pub fn split(&mut self, data: &[u8]) -> Result<AccessUnit, ParseError> {
        let mut unit = AccessUnit {
            sample_data: Vec::new(),
            is_random_access: false,
            parameters_changed: false,
        };

        for range in crate::annexb::nal_ranges(data)? {
            let nal = &data[range];
            match classify(self.codec, nal)? {
                NalKind::ParameterSet(kind) => {
                    let slot = match kind {
                        ParameterSetKind::Vps => &mut self.parameters.vps,
                        ParameterSetKind::Sps => &mut self.parameters.sps,
                        ParameterSetKind::Pps => &mut self.parameters.pps,
                    };
                    if slot.as_deref() != Some(nal) {
                        *slot = Some(nal.to_vec());
                        unit.parameters_changed = true;
                        if matches!(kind, ParameterSetKind::Sps) {
                            self.reorder_depth = self.parse_reorder_depth(nal)?;
                        }
                    }
                }

                NalKind::Picture { is_random_access } => {
                    unit.is_random_access |= is_random_access;
                    let length = u32::try_from(nal.len())
                        .map_err(|_err| ParseError::Invalid("NAL unit larger than 4 GiB"))?;
                    unit.sample_data.extend_from_slice(&length.to_be_bytes());
                    unit.sample_data.extend_from_slice(nal);
                }

                // SEI, access unit delimiters, filler and the like: the format
                // description carries everything the decoder needs.
                NalKind::Other => {}
            }
        }

        Ok(unit)
    }

    /// Forgets the parameter sets, so the next ones are reported as a change.
    pub fn reset(&mut self) {
        self.parameters = ParameterSets::default();
    }

    /// The reorder depth a sequence parameter set declares.
    ///
    /// `VideoToolbox` emits frames in decoding order, so this is what the reorder
    /// buffer puts them back into presentation order with.
    fn parse_reorder_depth(&mut self, nal: &[u8]) -> Result<usize, ParseError> {
        match self.codec {
            Codec::H264 => {
                let nal = h264_reader::nal::RefNal::new(nal, &[], true);
                let sps = h264_reader::nal::sps::SeqParameterSet::from_bits(
                    h264_reader::nal::Nal::rbsp_bits(&nal),
                )
                .map_err(|err| ParseError::nal("SPS", err))?;
                Ok(crate::reorder_depth::h264(&sps))
            }

            Codec::H265 => {
                let nalu = crate::annexb::h265_nalu(nal, &(0..nal.len()))?;
                let sps = self
                    .h265_parser
                    .parse_sps(&nalu)
                    .map_err(|err| ParseError::nal("SPS", err))?;
                Ok(crate::reorder_depth::h265(sps))
            }

            Codec::AV1 => Err(ParseError::Unsupported(
                "AV1 is not decodable through VideoToolbox",
            )),
        }
    }
}

// SAFETY: The `Rc`s of the `cros-codecs` parser never leave this struct.
#[expect(unsafe_code)]
unsafe impl Send for AccessUnitSplitter {}

enum ParameterSetKind {
    Vps,
    Sps,
    Pps,
}

enum NalKind {
    ParameterSet(ParameterSetKind),

    /// A coded slice, the data the decoder is fed.
    Picture {
        is_random_access: bool,
    },

    Other,
}

fn classify(codec: Codec, nal: &[u8]) -> Result<NalKind, ParseError> {
    match codec {
        Codec::H264 => {
            let header = *nal
                .first()
                .ok_or(ParseError::Invalid("Empty H.264 NAL unit"))?;
            if header & 0x80 != 0 {
                return Err(ParseError::Invalid(
                    "H.264 NAL unit with forbidden_zero_bit",
                ));
            }
            Ok(match header & 0x1f {
                7 => NalKind::ParameterSet(ParameterSetKind::Sps),
                8 => NalKind::ParameterSet(ParameterSetKind::Pps),
                // IDR slices, the only random access points H.264 has.
                5 => NalKind::Picture {
                    is_random_access: true,
                },
                1..=4 => NalKind::Picture {
                    is_random_access: false,
                },
                _ => NalKind::Other,
            })
        }

        Codec::H265 => {
            if nal.len() < 2 {
                return Err(ParseError::Invalid("Truncated H.265 NAL unit header"));
            }
            if nal[0] & 0x80 != 0 {
                return Err(ParseError::Invalid(
                    "H.265 NAL unit with forbidden_zero_bit",
                ));
            }
            Ok(match (nal[0] >> 1) & 0x3f {
                32 => NalKind::ParameterSet(ParameterSetKind::Vps),
                33 => NalKind::ParameterSet(ParameterSetKind::Sps),
                34 => NalKind::ParameterSet(ParameterSetKind::Pps),
                // BLA, IDR and CRA pictures: the intra random access points.
                16..=23 => NalKind::Picture {
                    is_random_access: true,
                },
                0..=31 => NalKind::Picture {
                    is_random_access: false,
                },
                _ => NalKind::Other,
            })
        }

        Codec::AV1 => Err(ParseError::Unsupported(
            "AV1 is not decodable through VideoToolbox",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::AccessUnitSplitter;
    use crate::Codec;

    /// Real parameter sets, taken from the `ippp` fixtures: the splitter parses the
    /// sequence parameter set for its reorder depth, so made up bytes won't do.
    const H264_SPS: &[u8] = &[
        0x67, 0x64, 0x00, 0x0a, 0xac, 0xb2, 0x08, 0x4d, 0x80, 0x88, 0x00, 0x00, 0x03, 0x00, 0x08,
        0x00, 0x00, 0x03, 0x01, 0xe0, 0x78, 0x91, 0x32, 0x40,
    ];
    const H264_PPS: &[u8] = &[0x68, 0xeb, 0xc0, 0x94, 0xb2, 0x2c];
    /// The same, from the wider half of the `sps_change` fixture.
    const H264_SPS_WIDER: &[u8] = &[
        0x67, 0x64, 0x00, 0x0a, 0xac, 0xb2, 0x0c, 0x4d, 0x80, 0x88, 0x00, 0x00, 0x03, 0x00, 0x08,
        0x00, 0x00, 0x03, 0x01, 0xe0, 0x78, 0x91, 0x32, 0x40,
    ];
    const H265_VPS: &[u8] = &[
        0x40, 0x01, 0x0c, 0x01, 0xff, 0xff, 0x01, 0x60, 0x00, 0x00, 0x03, 0x00, 0x90, 0x00, 0x00,
        0x03, 0x00, 0x00, 0x03, 0x00, 0x1e, 0x92, 0x80, 0x90,
    ];
    const H265_SPS: &[u8] = &[
        0x42, 0x01, 0x01, 0x01, 0x60, 0x00, 0x00, 0x03, 0x00, 0x90, 0x00, 0x00, 0x03, 0x00, 0x00,
        0x03, 0x00, 0x1e, 0xa0, 0x20, 0x81, 0x05, 0x96, 0x4a, 0x92, 0x4c, 0xaf, 0x01, 0x68, 0x08,
        0x00, 0x00, 0x03, 0x00, 0x08, 0x00, 0x00, 0x03, 0x00, 0xf0, 0x40,
    ];
    const H265_PPS: &[u8] = &[0x44, 0x01, 0xc1, 0x71, 0xa1, 0x12];

    fn annexb(nals: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        for nal in nals {
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(nal);
        }
        out
    }

    /// Parameter sets go into the format description, slices into the sample data,
    /// and repeats of the same parameter sets don't ask for a new session.
    #[test]
    fn h264_separates_parameter_sets_from_slices() {
        let mut splitter = AccessUnitSplitter::new(Codec::H264);

        let idr: &[u8] = &[0x65, 0xaa, 0xbb];
        let unit = splitter.split(&annexb(&[H264_SPS, H264_PPS, idr])).unwrap();
        assert!(unit.is_random_access);
        assert!(unit.parameters_changed);
        assert_eq!(unit.sample_data, [0, 0, 0, 3, 0x65, 0xaa, 0xbb]);
        assert_eq!(
            splitter.parameters().in_order().unwrap(),
            vec![H264_SPS, H264_PPS]
        );

        let slice: &[u8] = &[0x41, 0xcc];
        let unit = splitter
            .split(&annexb(&[H264_SPS, H264_PPS, slice]))
            .unwrap();
        assert!(!unit.is_random_access);
        assert!(!unit.parameters_changed);
        assert_eq!(unit.sample_data, [0, 0, 0, 2, 0x41, 0xcc]);
    }

    /// A new sequence parameter set mid-stream asks for a new session.
    #[test]
    fn h264_reports_changed_parameter_sets() {
        let mut splitter = AccessUnitSplitter::new(Codec::H264);
        splitter
            .split(&annexb(&[H264_SPS, H264_PPS, &[0x65, 0xaa]]))
            .unwrap();

        let unit = splitter
            .split(&annexb(&[H264_SPS_WIDER, H264_PPS, &[0x65, 0xaa]]))
            .unwrap();
        assert!(unit.parameters_changed);
    }

    /// H.265 has a video parameter set on top, and its random access points are
    /// a whole range of NAL unit types.
    #[test]
    fn h265_collects_three_parameter_sets() {
        let mut splitter = AccessUnitSplitter::new(Codec::H265);

        // NAL unit type 19: IDR_W_RADL.
        let idr: &[u8] = &[0x26, 0x01, 0xaf];
        let unit = splitter
            .split(&annexb(&[H265_VPS, H265_SPS, H265_PPS, idr]))
            .unwrap();
        assert!(unit.is_random_access);
        assert_eq!(
            splitter.parameters().in_order().unwrap(),
            vec![H265_VPS, H265_SPS, H265_PPS]
        );
        assert_eq!(unit.sample_data, [0, 0, 0, 3, 0x26, 0x01, 0xaf]);

        // NAL unit type 1: TRAIL_R.
        let trail: &[u8] = &[0x02, 0x01, 0xd0];
        let unit = splitter.split(&annexb(&[trail])).unwrap();
        assert!(!unit.is_random_access);
        assert_eq!(unit.sample_data, [0, 0, 0, 3, 0x02, 0x01, 0xd0]);
    }

    /// The reorder depth comes from the sequence parameter set, and the fixtures
    /// are encoded without reordering.
    #[test]
    fn reads_the_reorder_depth_from_the_sequence_parameter_set() {
        let mut splitter = AccessUnitSplitter::new(Codec::H264);
        splitter
            .split(&annexb(&[H264_SPS, H264_PPS, &[0x65, 0xaa]]))
            .unwrap();
        assert_eq!(splitter.reorder_depth(), 0);

        let mut splitter = AccessUnitSplitter::new(Codec::H265);
        splitter
            .split(&annexb(&[
                H265_VPS,
                H265_SPS,
                H265_PPS,
                &[0x26, 0x01, 0xaf],
            ]))
            .unwrap();
        assert_eq!(splitter.reorder_depth(), 0);
    }
}
