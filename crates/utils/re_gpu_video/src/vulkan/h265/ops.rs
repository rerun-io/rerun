//! The `DecodeOp` IR crossing from the safe parser to the GPU backend.
//!
//! Everything here is plain data: no GPU handles, no unsafe code, no `Rc` from
//! the `cros-codecs` parser. One access unit pushed into the [`super::Parser`]
//! yields a sequence of ops that the backend executes in order.

use std::ops::Range;

/// One instruction for the GPU backend.
#[derive(Debug)]
pub enum DecodeOp {
    /// A parameter set was added or changed.
    ///
    /// The backend recreates its session parameters from
    /// [`super::Parser::std_parameter_sets`], and the session itself when the
    /// coded size or DPB requirements changed.
    ParametersChanged,

    /// Decode one picture.
    DecodeFrame(DecodeInfo),
}

/// Everything the backend needs to decode one picture.
#[derive(Debug)]
pub struct DecodeInfo {
    pub vps_id: u8,
    pub sps_id: u8,
    pub pps_id: u8,

    /// Byte ranges of the slice segment NALs (without start codes) in the pushed
    /// access unit.
    ///
    /// Only valid for the data of the `push_access_unit` call that produced this op.
    pub slice_ranges: Vec<Range<usize>>,

    pub is_idr: bool,

    /// An intra random access point picture: IDR, BLA, or CRA. Decoding may start here.
    pub is_irap: bool,

    /// The DPB slot this picture decodes into.
    ///
    /// Unlike H.264 this is never absent: H.265 marks every decoded picture as
    /// used for short-term reference, and a later picture's reference set is what
    /// releases the slot again.
    pub setup_slot: u8,

    /// `PicOrderCntVal`, the presentation order key.
    pub poc: i32,

    /// The reference pictures this picture may use, one DPB slot each.
    /// The union of the three reference sets below, sorted by slot.
    pub references: Vec<ReferenceInfo>,

    /// `RefPicSetStCurrBefore`: DPB slots of the short-term references that
    /// precede this picture in presentation order, in list order.
    pub st_curr_before: Vec<u8>,

    /// `RefPicSetStCurrAfter`: the short-term references that follow it.
    pub st_curr_after: Vec<u8>,

    /// `RefPicSetLtCurr`: the long-term references.
    pub lt_curr: Vec<u8>,

    /// `short_term_ref_pic_set_sps_flag` of the slice headers: the short-term
    /// reference picture set came from the SPS instead of the slice header.
    pub short_term_ref_pic_set_sps_flag: bool,

    /// Bits the slice header spent on its own `st_ref_pic_set`,
    /// 0 when it came from the SPS.
    pub num_bits_for_st_ref_pic_set_in_slice: u16,

    /// `NumDeltaPocs` of the reference set this picture's set predicts from,
    /// 0 when it doesn't predict from another one.
    pub num_delta_pocs_of_ref_rps_idx: u8,
}

/// One active reference picture in the DPB.
#[derive(Debug, Clone)]
pub struct ReferenceInfo {
    pub slot: u8,

    /// `PicOrderCntVal` of the reference picture.
    pub poc: i32,

    pub is_long_term: bool,
}

impl std::fmt::Display for DecodeOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParametersChanged => write!(f, "ParametersChanged"),
            Self::DecodeFrame(info) => info.fmt(f),
        }
    }
}

impl std::fmt::Display for DecodeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DecodeFrame {{ poc: {poc}", poc = self.poc)?;
        if self.is_idr {
            write!(f, ", idr")?;
        } else if self.is_irap {
            write!(f, ", irap")?;
        }
        write!(f, ", slot {slot}", slot = self.setup_slot)?;
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
                    "slot {slot} ({kind} poc {poc})",
                    slot = reference.slot,
                    kind = if reference.is_long_term { "lt" } else { "st" },
                    poc = reference.poc,
                )?;
            }
            write!(f, "]")?;
        }
        if !self.st_curr_before.is_empty() {
            write!(f, ", before: {:?}", self.st_curr_before)?;
        }
        if !self.st_curr_after.is_empty() {
            write!(f, ", after: {:?}", self.st_curr_after)?;
        }
        if !self.lt_curr.is_empty() {
            write!(f, ", lt: {:?}", self.lt_curr)?;
        }
        write!(f, " }}")
    }
}
