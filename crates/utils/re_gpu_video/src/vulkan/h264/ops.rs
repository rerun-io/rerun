//! The `DecodeOp` IR crossing from the safe parser to the GPU backend.
//!
//! Everything here is plain data: no GPU handles, no unsafe code.
//! One access unit pushed into the [`super::Parser`] yields a sequence of ops
//! that the backend executes in order.

use std::ops::Range;

use h264_reader::nal::{pps::PicParameterSet, sps::SeqParameterSet};

/// One instruction for the GPU backend.
#[derive(Debug)]
pub enum DecodeOp {
    /// A new or changed SPS.
    ///
    /// The backend (re)creates its session parameters from it, and the session itself
    /// when the coded size, level, or DPB requirements changed.
    Sps(Box<SeqParameterSet>),

    /// A new or changed PPS.
    Pps(Box<PicParameterSet>),

    /// Decode one frame.
    DecodeFrame(DecodeInfo),

    /// The listed DPB slots are no longer used for reference.
    ///
    /// Emitted after the `DecodeFrame` whose reference marking freed them.
    /// The backend deactivates the slots, freed slot indices get reused by later frames.
    FreeSlots(Vec<u8>),
}

/// Everything the backend needs to decode one frame.
#[derive(Debug)]
pub struct DecodeInfo {
    pub sps_id: u8,
    pub pps_id: u8,

    /// Byte ranges of the slice NALs (without start codes) in the pushed access unit.
    ///
    /// Only valid for the data of the `push_access_unit` call that produced this op.
    pub slice_ranges: Vec<Range<usize>>,

    pub is_idr: bool,

    /// `idr_pic_id` of the slice headers, 0 for non-IDR frames.
    pub idr_pic_id: u16,

    /// All slices of the frame are intra coded (I or SI).
    pub is_intra: bool,

    /// The DPB slot this frame gets decoded into when it is a reference frame
    /// (`nal_ref_idc != 0`). `None` for non-reference frames, which only produce output.
    ///
    /// The slot is free at decode time. Slots freed by this frame's own reference
    /// marking follow in a separate [`DecodeOp::FreeSlots`].
    pub setup_slot: Option<u8>,

    /// The frame is marked as a long-term reference
    /// (IDR `long_term_reference_flag`, or memory management control operation 6).
    pub long_term_frame_idx: Option<u16>,

    pub frame_num: u16,

    /// `PicOrderCnt`, the presentation order key.
    /// Equal to the smaller of the two field order counts.
    pub poc: i32,

    /// Fed to `StdVideoDecodeH264PictureInfo::PicOrderCnt`.
    pub top_field_order_cnt: i32,
    pub bottom_field_order_cnt: i32,

    /// The reference frames the slices of this frame may use, one DPB slot each.
    /// The union of `ref_lists` entries over all slices.
    pub references: Vec<ReferenceInfo>,

    /// The reference picture lists of the first slice, mainly for tracing and debugging.
    /// The backend binds `references`, and the driver re-derives per-slice lists
    /// from the slice headers in the bitstream.
    pub ref_lists: RefLists,
}

/// One active reference frame in the DPB.
#[derive(Debug, Clone)]
pub struct ReferenceInfo {
    pub slot: u8,

    /// `FrameNum` for short-term references, `LongTermFrameIdx` for long-term ones,
    /// matching the `StdVideoDecodeH264ReferenceInfo::FrameNum` convention.
    pub frame_num: u16,

    pub top_field_order_cnt: i32,
    pub bottom_field_order_cnt: i32,

    pub is_long_term: bool,
}

/// Reference picture lists of one slice, as DPB slot indices in list order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RefLists {
    pub l0: Vec<u8>,
    pub l1: Vec<u8>,
}

impl std::fmt::Display for DecodeOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sps(sps) => {
                let (width, height) = sps.pixel_dimensions().unwrap_or((0, 0));
                write!(
                    f,
                    "Sps {{ id: {id}, {width}x{height}, poc_type: {poc_type}, max_num_ref_frames: {refs}, log2_max_frame_num: {frame_num_bits} }}",
                    id = sps.seq_parameter_set_id.id(),
                    poc_type = match sps.pic_order_cnt {
                        h264_reader::nal::sps::PicOrderCntType::TypeZero { .. } => 0,
                        h264_reader::nal::sps::PicOrderCntType::TypeOne { .. } => 1,
                        h264_reader::nal::sps::PicOrderCntType::TypeTwo => 2,
                    },
                    refs = sps.max_num_ref_frames,
                    frame_num_bits = sps.log2_max_frame_num(),
                )
            }

            Self::Pps(pps) => write!(
                f,
                "Pps {{ id: {id}, sps: {sps_id} }}",
                id = pps.pic_parameter_set_id.id(),
                sps_id = pps.seq_parameter_set_id.id(),
            ),

            Self::DecodeFrame(info) => info.fmt(f),

            Self::FreeSlots(slots) => write!(f, "FreeSlots {slots:?}"),
        }
    }
}

impl std::fmt::Display for DecodeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DecodeFrame {{ frame_num: {frame_num}, poc: {poc} (top {top}, bottom {bottom})",
            frame_num = self.frame_num,
            poc = self.poc,
            top = self.top_field_order_cnt,
            bottom = self.bottom_field_order_cnt,
        )?;
        if self.is_idr {
            write!(f, ", idr")?;
        }
        match (self.setup_slot, self.long_term_frame_idx) {
            (Some(slot), Some(long_term_idx)) => {
                write!(f, ", ref -> slot {slot} (long-term idx {long_term_idx})")?;
            }
            (Some(slot), None) => write!(f, ", ref -> slot {slot}")?,
            (None, _) => write!(f, ", non-ref")?,
        }
        if self.slice_ranges.len() != 1 {
            write!(f, ", slices: {}", self.slice_ranges.len())?;
        }
        if !self.references.is_empty() {
            write!(f, ", refs: [")?;
            for (i, reference) in self.references.iter().enumerate() {
                if i != 0 {
                    write!(f, ", ")?;
                }
                write!(
                    f,
                    "slot {slot} ({kind} {frame_num}, poc {poc})",
                    slot = reference.slot,
                    kind = if reference.is_long_term { "lt" } else { "fn" },
                    frame_num = reference.frame_num,
                    poc = reference
                        .top_field_order_cnt
                        .min(reference.bottom_field_order_cnt),
                )?;
            }
            write!(f, "]")?;
        }
        if !self.ref_lists.l0.is_empty() || !self.ref_lists.l1.is_empty() {
            write!(f, ", l0: {:?}", self.ref_lists.l0)?;
            if !self.ref_lists.l1.is_empty() {
                write!(f, ", l1: {:?}", self.ref_lists.l1)?;
            }
        }
        write!(f, " }}")
    }
}
