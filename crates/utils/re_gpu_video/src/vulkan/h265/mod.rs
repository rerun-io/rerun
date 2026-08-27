//! Safe H.265 bitstream parser producing the plain-data [`DecodeOp`] IR for the
//! Vulkan backend.
//!
//! Vulkan Video does no bitstream parsing beyond slice data: the application
//! supplies picture order counts, reference sets, and DPB slot assignments. This
//! module computes those from the slice headers. It is 100% safe code with no GPU
//! dependencies, so its tests run everywhere (CI machines have no GPU with video
//! decode support).
//!
//! All "spec X.Y.Z" references are to Rec. ITU-T H.265.
//!
//! The syntax-level parsing (parameter sets, slice segment headers including the
//! reference picture set) comes from the `cros-codecs` crate, which `re_video`
//! already uses for stream inspection. What lives here is the semantic layer on
//! top: access unit handling, picture order counts ([`poc`]), reference sets and
//! DPB slots ([`rps`]), and the conversion of parameter sets into the Vulkan
//! `StdVideo` structs ([`std_params`], the only file here touching `ash`).
//!
//! Compared to the H.264 stack this is much smaller: no memory management control
//! operations, no sliding window, no field pictures, and one picture order count
//! variant instead of three.

mod ops;
mod parse;
mod poc;
mod rps;
mod std_params;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use ash::vk::native as std_video;
use cros_codecs::codec::h265::parser::{self as h265, Sps};

use crate::{ColorProperties, MatrixCoefficients, ParseError};

pub use ops::{DecodeInfo, DecodeOp};
pub use std_params::{PpsStdParams, SpsStdParams, VpsStdParams};

/// A slot index no picture occupies, what Vulkan expects in the unused tail of
/// the reference set arrays of a decode operation.
pub const UNUSED_SLOT: u8 = 0xff;

/// What the backend derives from the SPS active for a picture.
#[derive(Clone, Copy, Debug)]
pub struct SpsFacts {
    /// Coded width & height in luma samples.
    pub coded_extent: [u32; 2],

    /// Top-left corner of the display region within the coded picture.
    pub crop_offset: [i32; 2],

    /// Display width & height, the coded size minus the conformance window.
    pub display: [u32; 2],

    /// DPB slots the stream needs, its declared buffer fullness plus one spare.
    pub dpb_slots: u32,

    /// Upper bound on the reference pictures one decode operation binds,
    /// validated against the device limit.
    pub max_ref_frames: u32,

    pub std_profile_idc: std_video::StdVideoH265ProfileIdc,

    pub color: ColorProperties,
}

/// The `cros-codecs` parser together with everything derived from it.
///
/// `cros-codecs` shares parsed parameter sets through `Rc`, so this is not `Send`
/// on its own. Every one of those `Rc`s lives inside this struct, which travels
/// to the decoder thread as one value and is only ever used from there.
struct ParserState {
    parser: h265::Parser,

    /// The raw bytes of the parameter set NAL last seen per id, to skip the
    /// identical repeats encoders put in front of every random access point.
    /// Identical bytes mean an identical set.
    seen_vps: HashMap<u8, Vec<u8>>,
    seen_sps: HashMap<u8, Vec<u8>>,
    seen_pps: HashMap<u8, Vec<u8>>,

    vps_std: Vec<std_video::StdVideoH265VideoParameterSet>,
    sps_std: Vec<std_video::StdVideoH265SequenceParameterSet>,
    pps_std: Vec<std_video::StdVideoH265PictureParameterSet>,

    /// Keeps the allocations the std structs point into alive.
    owned_std: OwnedStdParams,

    facts: HashMap<u8, SpsFacts>,
}

/// The parameter-set conversions, kept alive for as long as the flat std arrays
/// that point into them.
#[derive(Default)]
struct OwnedStdParams {
    vps: Vec<VpsStdParams>,
    sps: Vec<SpsStdParams>,
    pps: Vec<PpsStdParams>,
}

// SAFETY: The `Rc`s of the `cros-codecs` parser never leave this struct, and the
// std parameter structs own the allocations their pointers refer into.
#[expect(unsafe_code)]
unsafe impl Send for ParserState {}

/// Turns annex-b access units into [`DecodeOp`]s, one call per access unit.
///
/// Decoding must start at an intra random access point carrying its parameter
/// sets, which is also what [`Self::reset`] returns to. Any error leaves the
/// parser waiting for the next one.
pub struct Parser {
    state: ParserState,

    poc: poc::PocState,
    dpb: rps::Dpb,
    pending: Option<PendingPicture>,

    /// Hardware DPB slot capacity.
    max_dpb_slots: u8,

    /// The slot count the DPB is currently configured for. Compared against what
    /// the active SPS asks for rather than tracking its id, since a changed SPS
    /// commonly keeps its id.
    configured_dpb_slots: Option<u32>,

    /// The next picture must be an intra random access point: start of stream,
    /// after [`Self::reset`], or after an error.
    awaiting_irap: bool,

    /// The pictures that precede the last random access point in presentation
    /// order can't be decoded: its references are gone. They are dropped rather
    /// than decoded from missing references.
    skipping_leading_pictures: bool,

    /// `sps_max_num_reorder_pics` of the SPS active for the most recent picture.
    reorder_delay: usize,
}

/// The slice segments of one picture, collected until the picture is complete.
struct PendingPicture {
    slices: Vec<parse::ParsedSlice>,
}

impl PendingPicture {
    fn first(&self) -> &parse::ParsedSlice {
        self.slices.first().expect("never empty")
    }
}

impl Parser {
    /// `max_dpb_slots` is the device's DPB slot capacity, an upper bound on what
    /// a stream may need.
    pub fn new(max_dpb_slots: u8) -> Self {
        Self {
            state: ParserState {
                parser: h265::Parser::default(),
                seen_vps: HashMap::new(),
                seen_sps: HashMap::new(),
                seen_pps: HashMap::new(),
                vps_std: Vec::new(),
                sps_std: Vec::new(),
                pps_std: Vec::new(),
                owned_std: OwnedStdParams::default(),
                facts: HashMap::new(),
            },
            poc: poc::PocState::default(),
            dpb: rps::Dpb::new(0),
            pending: None,
            max_dpb_slots,
            configured_dpb_slots: None,
            awaiting_irap: true,
            skipping_leading_pictures: false,
            reorder_delay: 0,
        }
    }

    /// Parses one annex-b access unit into the ops decoding it requires.
    ///
    /// The slice byte ranges in the returned ops point into `data`.
    /// On error the access unit produces nothing and the parser waits for the next
    /// random access point, any pictures it tracked before stay valid.
    pub fn push_access_unit(&mut self, data: &[u8]) -> Result<Vec<DecodeOp>, ParseError> {
        let result = self.push_inner(data);
        if result.is_err() {
            self.pending = None;
            self.awaiting_irap = true;
        }
        result
    }

    /// Drops all picture state for a seek. Parameter sets are kept: the next
    /// access unit must hold a random access point, which re-sends them anyway.
    pub fn reset(&mut self) {
        self.pending = None;
        self.poc.reset();
        self.dpb.clear();
        self.awaiting_irap = true;
        self.skipping_leading_pictures = false;
    }

    /// `sps_max_num_reorder_pics` of the active SPS: how many pictures may
    /// precede a picture in decoding order but follow it in presentation order.
    pub fn reorder_delay(&self) -> usize {
        self.reorder_delay
    }

    /// What the backend derives from the SPS active for a picture.
    pub fn sps_facts(&self, sps_id: u8) -> Result<SpsFacts, ParseError> {
        self.state
            .facts
            .get(&sps_id)
            .copied()
            .ok_or(ParseError::MissingReference { what: "SPS" })
    }

    /// The parameter sets a session parameters object is built from.
    ///
    /// The returned slices borrow the allocations the parser owns, which stay in
    /// place until the next parameter set change.
    pub fn std_parameter_sets(
        &self,
    ) -> (
        &[std_video::StdVideoH265VideoParameterSet],
        &[std_video::StdVideoH265SequenceParameterSet],
        &[std_video::StdVideoH265PictureParameterSet],
    ) {
        (
            &self.state.vps_std,
            &self.state.sps_std,
            &self.state.pps_std,
        )
    }

    fn push_inner(&mut self, data: &[u8]) -> Result<Vec<DecodeOp>, ParseError> {
        re_tracing::profile_function!();

        let mut ops = Vec::new();
        let mut parameters_changed = false;

        for range in super::annexb::nal_ranges(data)? {
            let nalu = parse::nalu(data, &range)?;
            let nal_type = nalu.header.type_;

            // The id of a parameter set only follows from parsing it, so the
            // repeat check compares the raw bytes against every set of its kind:
            // identical bytes mean an identical set.
            let bytes = &data[range.clone()];
            let is_repeat =
                |seen: &HashMap<u8, Vec<u8>>| seen.values().any(|seen| seen.as_slice() == bytes);

            match nal_type {
                h265::NaluType::VpsNut => {
                    if !is_repeat(&self.state.seen_vps) {
                        let id = parse::parse_vps(&mut self.state.parser, &nalu)?;
                        self.state.seen_vps.insert(id, bytes.to_vec());
                        parameters_changed = true;
                    }
                }

                h265::NaluType::SpsNut => {
                    if !is_repeat(&self.state.seen_sps) {
                        let sps = parse::parse_sps(&mut self.state.parser, &nalu)?;
                        let id = sps.seq_parameter_set_id;
                        let facts = sps_facts(sps)?;
                        self.state.facts.insert(id, facts);
                        self.state.seen_sps.insert(id, bytes.to_vec());
                        parameters_changed = true;
                    }
                }

                h265::NaluType::PpsNut => {
                    if !is_repeat(&self.state.seen_pps) {
                        let id = parse::parse_pps(&mut self.state.parser, &nalu)?;
                        self.state.seen_pps.insert(id, bytes.to_vec());
                        parameters_changed = true;
                    }
                }

                nal_type if is_slice_nal(nal_type) => {
                    let slice = parse::parse_slice(&mut self.state.parser, data, range)?;

                    if slice.header.first_slice_segment_in_pic_flag {
                        if let Some(finished) = self.pending.take() {
                            self.finalize_picture(&finished, &mut ops)?;
                        }
                        self.pending = Some(PendingPicture {
                            slices: vec![slice],
                        });
                    } else {
                        let Some(pending) = &mut self.pending else {
                            return Err(ParseError::IncompletePicture);
                        };
                        pending.slices.push(slice);
                    }
                }

                // Nothing decode-relevant in these.
                h265::NaluType::AudNut
                | h265::NaluType::EosNut
                | h265::NaluType::EobNut
                | h265::NaluType::FdNut
                | h265::NaluType::PrefixSeiNut
                | h265::NaluType::SuffixSeiNut => {}

                unsupported => {
                    return Err(ParseError::Unsupported(match unsupported {
                        h265::NaluType::RsvIrapVcl22 | h265::NaluType::RsvIrapVcl23 => {
                            "reserved intra random access point picture types"
                        }
                        _ => "reserved NAL unit type",
                    }));
                }
            }
        }

        // The push contract is one complete access unit: the picture is finished
        // now, no need to wait for the next picture's first slice to prove it.
        if let Some(finished) = self.pending.take() {
            self.finalize_picture(&finished, &mut ops)?;
        }

        if parameters_changed {
            self.rebuild_std_parameter_sets();
            // Ahead of the decode ops of this access unit: they need the new sets.
            ops.insert(0, DecodeOp::ParametersChanged);
        }

        Ok(ops)
    }

    /// Runs the per-picture decode processes on an assembled picture:
    /// picture order count (spec 8.3.1) and the reference sets (8.3.2).
    fn finalize_picture(
        &mut self,
        picture: &PendingPicture,
        ops: &mut Vec<DecodeOp>,
    ) -> Result<(), ParseError> {
        let first = picture.first();
        let header = &first.header;

        // A random access point that starts a new prediction sequence: every IDR
        // and BLA picture, plus a CRA picture opening the stream or ending a seek.
        let starts_sequence = first.is_irap && (first.is_idr || !self.dpb_has_pictures());

        if self.awaiting_irap {
            if !first.is_irap {
                return Err(ParseError::ExpectedRandomAccessPoint);
            }
            self.awaiting_irap = false;
            // The pictures that precede this one in presentation order reference
            // pictures that were never decoded.
            self.skipping_leading_pictures = true;
        } else if first.is_irap {
            // A random access point reached in decoding order: whether its
            // leading pictures are decodable depends on whether the stream
            // before it was decoded.
            self.skipping_leading_pictures = starts_sequence;
        }

        if first.is_rasl && self.skipping_leading_pictures {
            // Not decodable and not meant to be output, spec 8.1.3.
            return Ok(());
        }

        let (sps_id, pps_id, vps_id) = self.ids_of(header.pic_parameter_set_id)?;
        let facts = self.sps_facts(sps_id)?;
        let sps_ordering = self.sps_ordering(sps_id)?;

        // A new SPS resizes the DPB. It can only be activated at a random access
        // point, which empties the buffer first.
        if self.configured_dpb_slots != Some(facts.dpb_slots) {
            self.dpb.configure(facts.dpb_slots, self.max_dpb_slots)?;
            self.configured_dpb_slots = Some(facts.dpb_slots);
        }

        let poc = self.poc.compute(&poc::PocInput {
            pic_order_cnt_lsb: header.pic_order_cnt_lsb,
            log2_max_pic_order_cnt_lsb_minus4: sps_ordering.log2_max_pic_order_cnt_lsb_minus4,
            starts_sequence,
            temporal_id: first.temporal_id,
            is_rasl_radl_or_slnr: first.is_rasl_radl_or_slnr,
        })?;

        let short_term = self.short_term_set(sps_id, header)?;
        let long_term = long_term_entries(header);
        let max_poc_lsb = 1_i32 << (u32::from(sps_ordering.log2_max_pic_order_cnt_lsb_minus4) + 4);
        let (sets, _freed) = self.dpb.build_reference_sets(&rps::CurrentPicture {
            poc,
            starts_sequence,
            max_poc_lsb,
            short_term: rps::ShortTermSet {
                delta_poc_s0: &short_term.delta_poc_s0,
                used_by_curr_pic_s0: &short_term.used_by_curr_pic_s0,
                delta_poc_s1: &short_term.delta_poc_s1,
                used_by_curr_pic_s1: &short_term.used_by_curr_pic_s1,
            },
            long_term: &long_term,
        })?;

        ops.push(DecodeOp::DecodeFrame(DecodeInfo {
            vps_id,
            sps_id,
            pps_id,
            slice_ranges: picture
                .slices
                .iter()
                .map(|slice| slice.range.clone())
                .collect(),
            is_idr: first.is_idr,
            is_irap: first.is_irap,
            setup_slot: sets.setup_slot,
            poc,
            references: sets.references,
            st_curr_before: sets.st_curr_before,
            st_curr_after: sets.st_curr_after,
            lt_curr: sets.lt_curr,
            short_term_ref_pic_set_sps_flag: header.short_term_ref_pic_set_sps_flag,
            num_bits_for_st_ref_pic_set_in_slice: if header.short_term_ref_pic_set_sps_flag {
                0
            } else {
                header.st_rps_bits as u16
            },
            num_delta_pocs_of_ref_rps_idx: short_term.num_delta_pocs_of_ref_rps_idx,
        }));

        self.reorder_delay = usize::from(sps_ordering.max_num_reorder_pics);
        Ok(())
    }

    fn dpb_has_pictures(&self) -> bool {
        !self.dpb.is_empty()
    }

    /// The parameter set ids a slice's PPS id resolves to.
    fn ids_of(&self, pps_id: u8) -> Result<(u8, u8, u8), ParseError> {
        let pps = self
            .state
            .parser
            .get_pps(pps_id)
            .ok_or(ParseError::MissingReference { what: "PPS" })?;
        Ok((
            pps.seq_parameter_set_id,
            pps.pic_parameter_set_id,
            pps.sps.video_parameter_set_id,
        ))
    }

    /// The picture order count and reorder fields of an SPS.
    fn sps_ordering(&self, sps_id: u8) -> Result<SpsOrdering, ParseError> {
        let sps = self
            .state
            .parser
            .get_sps(sps_id)
            .ok_or(ParseError::MissingReference { what: "SPS" })?;
        Ok(SpsOrdering {
            log2_max_pic_order_cnt_lsb_minus4: sps.log2_max_pic_order_cnt_lsb_minus4,
            max_num_reorder_pics: sps.max_num_reorder_pics[usize::from(sps.max_sub_layers_minus1)],
        })
    }

    /// The current picture's short-term reference picture set, copied out of
    /// wherever the bitstream put it.
    fn short_term_set(
        &self,
        sps_id: u8,
        header: &h265::SliceHeader,
    ) -> Result<ShortTermSetData, ParseError> {
        let sps = self
            .state
            .parser
            .get_sps(sps_id)
            .ok_or(ParseError::MissingReference { what: "SPS" })?;

        let (set, curr_rps_idx) = if header.short_term_ref_pic_set_sps_flag {
            let index = usize::from(header.short_term_ref_pic_set_idx);
            let set = sps
                .short_term_ref_pic_set
                .get(index)
                .ok_or(ParseError::Invalid(
                    "slice references an unknown reference picture set",
                ))?;
            (set, index)
        } else {
            (
                &header.short_term_ref_pic_set,
                usize::from(sps.num_short_term_ref_pic_sets),
            )
        };

        // `NumDeltaPocs` of the set this one predicts from, which the driver needs
        // to parse a slice header's own set from the bitstream.
        let num_delta_pocs_of_ref_rps_idx = if set.inter_ref_pic_set_prediction_flag {
            let reference = curr_rps_idx
                .checked_sub(usize::from(set.delta_idx_minus1) + 1)
                .ok_or(ParseError::Invalid(
                    "reference picture set predicts from a set before the first",
                ))?;
            let reference =
                sps.short_term_ref_pic_set
                    .get(reference)
                    .ok_or(ParseError::Invalid(
                        "reference picture set predicts from an unknown set",
                    ))?;
            reference.num_delta_pocs as u8
        } else {
            0
        };

        let negative = usize::from(set.num_negative_pics);
        let positive = usize::from(set.num_positive_pics);
        Ok(ShortTermSetData {
            delta_poc_s0: set.delta_poc_s0[..negative].to_vec(),
            used_by_curr_pic_s0: set.used_by_curr_pic_s0[..negative].to_vec(),
            delta_poc_s1: set.delta_poc_s1[..positive].to_vec(),
            used_by_curr_pic_s1: set.used_by_curr_pic_s1[..positive].to_vec(),
            num_delta_pocs_of_ref_rps_idx,
        })
    }

    /// Rebuilds every std parameter set from the parser's current sets.
    ///
    /// A changed SPS also changes what its picture parameter sets refer to, so
    /// all three kinds get rebuilt together rather than tracked individually.
    fn rebuild_std_parameter_sets(&mut self) {
        re_tracing::profile_function!();

        let state = &mut self.state;
        state.owned_std = OwnedStdParams::default();

        // The id ranges the syntax allows, all cheap to scan.
        for id in 0..=15 {
            if let Some(vps) = state.parser.get_vps(id) {
                state.owned_std.vps.push(VpsStdParams::build(vps));
            }
        }
        for id in 0..=15 {
            if let Some(sps) = state.parser.get_sps(id) {
                state.owned_std.sps.push(SpsStdParams::build(sps));
            }
        }
        for id in 0..=63 {
            if let Some(pps) = state.parser.get_pps(id) {
                state.owned_std.pps.push(PpsStdParams::build(pps));
            }
        }

        state.vps_std = state
            .owned_std
            .vps
            .iter()
            .map(|params| *params.std())
            .collect();
        state.sps_std = state
            .owned_std
            .sps
            .iter()
            .map(|params| *params.std())
            .collect();
        state.pps_std = state
            .owned_std
            .pps
            .iter()
            .map(|params| *params.std())
            .collect();
    }
}

/// The picture order count and reorder fields of an SPS.
struct SpsOrdering {
    log2_max_pic_order_cnt_lsb_minus4: u8,
    max_num_reorder_pics: u8,
}

/// One picture's short-term reference picture set, detached from the parser.
struct ShortTermSetData {
    delta_poc_s0: Vec<i32>,
    used_by_curr_pic_s0: Vec<bool>,
    delta_poc_s1: Vec<i32>,
    used_by_curr_pic_s1: Vec<bool>,
    num_delta_pocs_of_ref_rps_idx: u8,
}

/// The long-term reference entries of a slice header.
fn long_term_entries(header: &h265::SliceHeader) -> Vec<rps::LongTermEntry> {
    let count = usize::from(header.num_long_term_sps) + usize::from(header.num_long_term_pics);
    (0..count.min(16))
        .map(|index| rps::LongTermEntry {
            poc_lsb_lt: header.poc_lsb_lt[index],
            delta_poc_msb_cycle_lt: header.delta_poc_msb_cycle_lt[index],
            msb_present: header.delta_poc_msb_present_flag[index],
            used_by_curr_pic: header.used_by_curr_pic_lt[index],
        })
        .collect()
}

fn is_slice_nal(nal_type: h265::NaluType) -> bool {
    matches!(
        nal_type,
        h265::NaluType::TrailN
            | h265::NaluType::TrailR
            | h265::NaluType::TsaN
            | h265::NaluType::TsaR
            | h265::NaluType::StsaN
            | h265::NaluType::StsaR
            | h265::NaluType::RadlN
            | h265::NaluType::RadlR
            | h265::NaluType::RaslN
            | h265::NaluType::RaslR
            | h265::NaluType::BlaWLp
            | h265::NaluType::BlaWRadl
            | h265::NaluType::BlaNLp
            | h265::NaluType::IdrWRadl
            | h265::NaluType::IdrNLp
            | h265::NaluType::CraNut
    )
}

/// What the backend needs from an SPS, extracted before its borrow ends.
fn sps_facts(sps: &Sps) -> Result<SpsFacts, ParseError> {
    let coded_width = u32::from(sps.pic_width_in_luma_samples);
    let coded_height = u32::from(sps.pic_height_in_luma_samples);

    // The conformance window offsets are in chroma units, two luma samples each
    // for the 4:2:0 streams that make it here.
    let (crop_left, crop_right, crop_top, crop_bottom) = if sps.conformance_window_flag {
        (
            sps.conf_win_left_offset * 2,
            sps.conf_win_right_offset * 2,
            sps.conf_win_top_offset * 2,
            sps.conf_win_bottom_offset * 2,
        )
    } else {
        (0, 0, 0, 0)
    };
    let display_width = coded_width
        .checked_sub(crop_left + crop_right)
        .filter(|&width| width > 0)
        .ok_or(ParseError::Invalid(
            "the conformance window exceeds the coded width",
        ))?;
    let display_height = coded_height
        .checked_sub(crop_top + crop_bottom)
        .filter(|&height| height > 0)
        .ok_or(ParseError::Invalid(
            "the conformance window exceeds the coded height",
        ))?;

    // The declared buffer fullness bounds the reference pictures the stream keeps.
    // One spare slot on top absorbs the current picture.
    let declared_fullness =
        u32::from(sps.max_dec_pic_buffering_minus1[usize::from(sps.max_sub_layers_minus1)]) + 1;
    let dpb_slots = declared_fullness + 1;

    Ok(SpsFacts {
        coded_extent: [coded_width, coded_height],
        crop_offset: [crop_left.cast_signed(), crop_top.cast_signed()],
        display: [display_width, display_height],
        dpb_slots,
        max_ref_frames: declared_fullness,
        std_profile_idc: std_params::std_profile_idc_of(sps),
        color: color_properties(sps),
    })
}

/// Color properties from the SPS VUI, absent fields left at their defaults.
fn color_properties(sps: &Sps) -> ColorProperties {
    let vui = &sps.vui_parameters;
    let present = sps.vui_parameters_present_flag;
    ColorProperties {
        full_range: present && vui.video_full_range_flag,
        matrix_coefficients: if present && vui.colour_description_present_flag {
            match vui.matrix_coeffs {
                1 => MatrixCoefficients::Bt709,
                5 | 6 => MatrixCoefficients::Bt601,
                _ => MatrixCoefficients::Unspecified,
            }
        } else {
            MatrixCoefficients::Unspecified
        },
    }
}
