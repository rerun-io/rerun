//! Safe H.264 bitstream parser producing the plain-data [`DecodeOp`] IR for the Vulkan backend.
//!
//! Vulkan Video does no bitstream parsing beyond slice data: the application supplies
//! picture order counts, reference lists, and DPB slot assignments. This module computes
//! those from the slice headers. It is 100% safe code with no GPU dependencies, so its
//! tests run everywhere (CI machines have no GPU with video decode support).
//!
//! All "spec X.Y.Z" references are to Rec. ITU-T H.264 (V16, 06/2026):
//! <https://www.itu.int/rec/T-REC-H.264-202606-I>.
//!
//! The syntax-level parsing (NAL framing, SPS/PPS, slice headers) comes from the
//! `h264-reader` crate, what lives here is the semantic layer on top:
//! access unit handling ([`au`]), picture order counts ([`poc`]), reference tracking and
//! DPB slots ([`refs`]), and the conversion of parameter sets into the Vulkan `StdVideo`
//! structs ([`std_params`], the only file here touching `ash` types).

mod au;
mod ops;
mod parse;
mod poc;
mod refs;
mod std_params;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use h264_reader::nal::{Nal as _, RefNal, UnitType, sps::SeqParameterSet};

pub use ops::{DecodeInfo, DecodeOp};
pub use std_params::{PpsStdParams, SpsStdParams};

pub(crate) use crate::ParseError;

/// Turns annex-b access units into [`DecodeOp`]s, one call per access unit.
///
/// Decoding must start at an IDR frame carrying its SPS/PPS, which is also what
/// [`Self::reset`] returns to. Any error leaves the parser waiting for the next IDR.
pub struct Parser {
    ctx: h264_reader::Context,

    /// Last emitted parameter sets by id, to skip re-emitting the identical repeats
    /// encoders put in front of every IDR frame. The PPS is fingerprinted through its
    /// `Debug` output since `h264-reader` gives it no `PartialEq`.
    emitted_sps: HashMap<u8, SeqParameterSet>,
    emitted_pps: HashMap<u8, String>,

    poc: poc::PocState,
    dpb: refs::Dpb,
    pending: Option<au::PendingPicture>,

    /// Hardware DPB slot capacity.
    max_dpb_slots: u8,

    /// The next frame must be an IDR: start of stream, after [`Self::reset`], or after an error.
    awaiting_idr: bool,

    /// `max_num_reorder_frames` of the SPS active for the most recent frame.
    reorder_delay: usize,
}

impl Parser {
    /// `max_dpb_slots` is the device's DPB slot capacity, an upper bound on the
    /// `max_num_ref_frames + 1` slots a stream may need.
    pub fn new(max_dpb_slots: u8) -> Self {
        Self {
            ctx: h264_reader::Context::new(),
            emitted_sps: HashMap::new(),
            emitted_pps: HashMap::new(),
            poc: poc::PocState::default(),
            dpb: refs::Dpb::default(),
            pending: None,
            max_dpb_slots,
            awaiting_idr: true,
            reorder_delay: 0,
        }
    }

    /// Parses one annex-b access unit into the ops decoding it requires.
    ///
    /// The slice byte ranges in the returned ops point into `data`.
    /// On error the access unit produces nothing and the parser waits for the next
    /// IDR frame, any frames it tracked before stay valid.
    pub fn push_access_unit(&mut self, data: &[u8]) -> Result<Vec<DecodeOp>, ParseError> {
        let result = self.push_inner(data);
        if result.is_err() {
            self.pending = None;
            self.awaiting_idr = true;
        }
        result
    }

    /// Drops all frame state for a seek. Parameter sets are kept:
    /// the next access unit must hold an IDR frame, which re-sends them anyway.
    pub fn reset(&mut self) {
        self.pending = None;
        self.poc.reset();
        // The backend drops its DPB slot state along with the parser, no `FreeSlots` needed.
        let _ = self.dpb.clear();
        self.awaiting_idr = true;
    }

    /// `max_num_reorder_frames` of the active SPS: how many frames may precede a frame
    /// in decoding order but follow it in presentation order.
    pub fn reorder_delay(&self) -> usize {
        self.reorder_delay
    }

    fn push_inner(&mut self, data: &[u8]) -> Result<Vec<DecodeOp>, ParseError> {
        re_tracing::profile_function!();

        let mut ops = Vec::new();

        for range in parse::nal_ranges(data)? {
            let nal = RefNal::new(&data[range.clone()], &[], true);
            let nal_header = nal
                .header()
                .map_err(|err| ParseError::nal("NAL header", err))?;

            match nal_header.nal_unit_type() {
                UnitType::SeqParameterSet => {
                    let sps = parse::parse_sps(&nal)?;
                    let id = sps.seq_parameter_set_id.id();
                    if self.emitted_sps.get(&id) != Some(&sps) {
                        self.emitted_sps.insert(id, sps.clone());
                        self.ctx.put_seq_param_set(sps.clone());
                        ops.push(DecodeOp::Sps(Box::new(sps)));
                    }
                }

                UnitType::PicParameterSet => {
                    let pps = parse::parse_pps(&self.ctx, &nal)?;
                    let id = pps.pic_parameter_set_id.id();
                    let fingerprint = format!("{pps:?}");
                    if self.emitted_pps.get(&id) != Some(&fingerprint) {
                        self.emitted_pps.insert(id, fingerprint);
                        self.ctx.put_pic_param_set(pps.clone());
                        ops.push(DecodeOp::Pps(Box::new(pps)));
                    }
                }

                UnitType::SliceLayerWithoutPartitioningIdr
                | UnitType::SliceLayerWithoutPartitioningNonIdr => {
                    if self.awaiting_idr
                        && self.pending.is_none()
                        && nal_header.nal_unit_type()
                            == UnitType::SliceLayerWithoutPartitioningNonIdr
                    {
                        return Err(ParseError::ExpectedRandomAccessPoint);
                    }

                    let slice = parse::parse_slice(&self.ctx, nal_header, &nal, range)?;
                    match &mut self.pending {
                        Some(pending) if !au::PendingPicture::is_picture_boundary(&slice) => {
                            pending.push(slice)?;
                        }
                        _ => {
                            if let Some(finished) = self.pending.take() {
                                self.finalize_picture(&finished, &mut ops)?;
                            }
                            self.pending = Some(au::PendingPicture::new(slice)?);
                        }
                    }
                }

                // Nothing decode-relevant in these.
                UnitType::SEI
                | UnitType::AccessUnitDelimiter
                | UnitType::FillerData
                | UnitType::EndOfSeq
                | UnitType::EndOfStream
                | UnitType::Unspecified(_) => {}

                unsupported => {
                    return Err(ParseError::Unsupported(match unsupported {
                        UnitType::SliceDataPartitionALayer
                        | UnitType::SliceDataPartitionBLayer
                        | UnitType::SliceDataPartitionCLayer => "slice data partitioning",
                        UnitType::SliceExtension
                        | UnitType::SliceExtensionViewComponent
                        | UnitType::PrefixNALUnit
                        | UnitType::SubsetSeqParameterSet => "SVC/MVC extensions",
                        UnitType::SliceLayerWithoutPartitioningAux => "auxiliary coded pictures",
                        _ => "reserved NAL unit type",
                    }));
                }
            }
        }

        // The push contract is one complete access unit: the frame is finished now,
        // no need to wait for the next frame's first slice to prove it.
        if let Some(finished) = self.pending.take() {
            self.finalize_picture(&finished, &mut ops)?;
        }

        Ok(ops)
    }

    /// Runs the per-frame decode processes on an assembled picture:
    /// POC (spec 8.2.1), reference lists (8.2.4), marking & slot assignment (8.2.5).
    fn finalize_picture(
        &mut self,
        picture: &au::PendingPicture,
        ops: &mut Vec<DecodeOp>,
    ) -> Result<(), ParseError> {
        let first = picture.first();
        let is_idr = first.is_idr;

        if self.awaiting_idr && !is_idr {
            return Err(ParseError::ExpectedRandomAccessPoint);
        }
        if is_idr && first.nal_ref_idc == 0 {
            return Err(ParseError::Invalid("IDR frame with nal_ref_idc 0"));
        }

        // Clone the active parameter sets out of the context, ending its borrow.
        // They exist, otherwise the slice headers wouldn't have parsed.
        let sps_id = h264_reader::nal::sps::SeqParamSetId::from_u32(u32::from(first.sps_id))
            .map_err(|err| ParseError::nal("SPS id", err))?;
        let pps_id = h264_reader::nal::pps::PicParamSetId::from_u32(u32::from(first.pps_id))
            .map_err(|err| ParseError::nal("PPS id", err))?;
        let sps = self
            .ctx
            .sps_by_id(sps_id)
            .ok_or(ParseError::Invalid("slice references an unknown SPS"))?
            .clone();
        let pps = self
            .ctx
            .pps_by_id(pps_id)
            .ok_or(ParseError::Invalid("slice references an unknown PPS"))?
            .clone();

        // An IDR frame empties the DPB before it is decoded, making room before new SPS
        // requirements apply. This `FreeSlots` precedes the `DecodeFrame`: the IDR
        // references nothing and may reuse a slot index freed here.
        if is_idr {
            let freed = self.dpb.clear();
            if !freed.is_empty() {
                ops.push(DecodeOp::FreeSlots(freed));
            }
        }
        self.dpb.configure(&sps, self.max_dpb_slots)?;

        if !is_idr {
            self.dpb.check_frame_num(&sps, first.header.frame_num)?;
        }

        let marking = first.header.dec_ref_pic_marking.as_ref();
        let poc = self.poc.compute(
            &sps,
            &poc::PocInput {
                is_idr,
                nal_ref_idc: first.nal_ref_idc,
                frame_num: first.header.frame_num,
                pic_order_cnt_lsb: &first.header.pic_order_cnt_lsb,
                has_mmco5: refs::has_mmco5(marking),
            },
        )?;

        let current = refs::CurrentFrame {
            frame_num: first.header.frame_num,
            poc,
            max_frame_num: 1 << sps.log2_max_frame_num(),
        };

        // Reference lists are per slice. The decode op binds their union and reports
        // the first slice's lists for tracing.
        let mut references = Vec::new();
        let mut ref_lists = ops::RefLists::default();
        for (index, slice) in picture.slices.iter().enumerate() {
            let lists = self
                .dpb
                .ref_lists(&sps, &pps, &slice.header, &current, &mut references)?;
            if index == 0 {
                ref_lists = lists;
            }
        }
        references.sort_by_key(|reference| reference.slot);

        let outcome = self.dpb.mark(&current, marking)?;

        ops.push(DecodeOp::DecodeFrame(DecodeInfo {
            sps_id: first.sps_id,
            pps_id: first.pps_id,
            slice_ranges: picture
                .slices
                .iter()
                .map(|slice| slice.range.clone())
                .collect(),
            is_idr,
            idr_pic_id: first.header.idr_pic_id.unwrap_or(0) as u16,
            is_intra: picture.slices.iter().all(|slice| {
                matches!(
                    slice.header.slice_type.family,
                    h264_reader::nal::slice::SliceFamily::I
                        | h264_reader::nal::slice::SliceFamily::SI
                )
            }),
            setup_slot: outcome.setup_slot,
            scratch_slot: outcome.scratch_slot,
            long_term_frame_idx: outcome.long_term_frame_idx,
            frame_num: first.header.frame_num,
            poc: poc.poc(),
            top_field_order_cnt: poc.top,
            bottom_field_order_cnt: poc.bottom,
            references,
            ref_lists,
        }));
        if !outcome.freed.is_empty() {
            ops.push(DecodeOp::FreeSlots(outcome.freed));
        }

        self.reorder_delay = max_num_reorder_frames(&sps) as usize;
        self.awaiting_idr = false;
        Ok(())
    }
}

/// `max_num_reorder_frames` of an SPS: from the VUI when present,
/// otherwise the level-based `MaxDpbFrames` default (spec E.2.1).
fn max_num_reorder_frames(sps: &SeqParameterSet) -> u32 {
    if let Some(restrictions) = sps
        .vui_parameters
        .as_ref()
        .and_then(|vui| vui.bitstream_restrictions.as_ref())
    {
        return restrictions.max_num_reorder_frames;
    }

    let profile_idc = u8::from(sps.profile_idc);
    if matches!(profile_idc, 44 | 86 | 100 | 110 | 122 | 244) && sps.constraint_flags.flag3() {
        return 0;
    }

    let frame_size_in_mbs =
        (sps.pic_width_in_mbs_minus1 + 1) * (sps.pic_height_in_map_units_minus1 + 1);
    (max_dpb_mbs(sps) / frame_size_in_mbs.max(1)).min(16)
}

/// `MaxDpbMbs` for the SPS's level (spec Table A-1).
fn max_dpb_mbs(sps: &SeqParameterSet) -> u32 {
    // Level 1b is signaled as level 1.1 plus constraint_set3_flag
    // (or as level_idc 9 where profiles allow).
    if sps.level_idc == 11 && sps.constraint_flags.flag3() {
        return 396;
    }
    match sps.level_idc {
        0..=10 => 396,
        11 => 900,
        12..=20 => 2376,
        21 => 4752,
        22..=30 => 8100,
        31 => 18000,
        32 => 20480,
        33..=41 => 32768,
        42 => 34816,
        43..=50 => 110400,
        51..=52 => 184320,
        // Levels 6 to 6.2, and a permissive default for whatever comes after.
        _ => 696320,
    }
}
