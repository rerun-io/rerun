//! Picture order count computation (H.264 spec section 8.2.1), for progressive frames.
//!
//! The POC orders decoded frames for presentation. Types 0, 1, and 2 are supported;
//! fields and interlacing are rejected before this module runs, so both field order
//! counts always belong to one frame.

use h264_reader::nal::{
    slice::PicOrderCountLsb,
    sps::{PicOrderCntType, SeqParameterSet},
};

use super::ParseError;

/// The order counts of one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Poc {
    pub top: i32,
    pub bottom: i32,
}

impl Poc {
    /// `PicOrderCnt` of the frame, the presentation order key.
    pub fn poc(&self) -> i32 {
        self.top.min(self.bottom)
    }
}

/// What [`PocState::compute`] needs to know about the current frame.
pub struct PocInput<'a> {
    pub is_idr: bool,
    pub nal_ref_idc: u8,
    pub frame_num: u16,

    /// `pic_order_cnt_lsb` syntax from the slice header, shape depends on the POC type.
    pub pic_order_cnt_lsb: &'a Option<PicOrderCountLsb>,

    /// The frame's reference marking contains memory management control operation 5,
    /// which rebases the POC of the current frame to zero and resets the prediction state.
    pub has_mmco5: bool,
}

/// POC prediction state across frames, in decoding order.
#[derive(Default)]
pub struct PocState {
    // Type 0: from the most recent reference frame.
    prev_poc_msb: i32,
    prev_poc_lsb: i32,

    // Types 1 and 2: from the previous frame, reference or not.
    prev_frame_num: u32,
    prev_frame_num_offset: u32,
}

impl PocState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Computes the order counts of the current frame and advances the prediction state.
    pub fn compute(
        &mut self,
        sps: &SeqParameterSet,
        input: &PocInput<'_>,
    ) -> Result<Poc, ParseError> {
        match &sps.pic_order_cnt {
            PicOrderCntType::TypeZero {
                log2_max_pic_order_cnt_lsb_minus4,
            } => self.compute_type0(u32::from(*log2_max_pic_order_cnt_lsb_minus4) + 4, input),

            PicOrderCntType::TypeOne {
                delta_pic_order_always_zero_flag: _,
                offset_for_non_ref_pic,
                offset_for_top_to_bottom_field,
                offsets_for_ref_frame,
            } => self.compute_type1(
                sps,
                *offset_for_non_ref_pic,
                *offset_for_top_to_bottom_field,
                offsets_for_ref_frame,
                input,
            ),

            PicOrderCntType::TypeTwo => Ok(self.compute_type2(sps, input)),
        }
    }

    /// Spec 8.2.1.1: POC from `pic_order_cnt_lsb` in the slice header,
    /// with the MSB tracked across wraparounds.
    fn compute_type0(
        &mut self,
        log2_max_lsb: u32,
        input: &PocInput<'_>,
    ) -> Result<Poc, ParseError> {
        let max_lsb = 1i32 << log2_max_lsb;

        let (lsb, delta_bottom) = match input.pic_order_cnt_lsb {
            Some(PicOrderCountLsb::Frame(lsb)) => (lsb.cast_signed(), 0),
            Some(PicOrderCountLsb::FieldsAbsolute {
                pic_order_cnt_lsb,
                delta_pic_order_cnt_bottom,
            }) => (pic_order_cnt_lsb.cast_signed(), *delta_pic_order_cnt_bottom),
            _ => return Err(ParseError::PocSyntaxMismatch),
        };

        let (prev_msb, prev_lsb) = if input.is_idr {
            (0, 0)
        } else {
            (self.prev_poc_msb, self.prev_poc_lsb)
        };

        let msb = if lsb < prev_lsb && prev_lsb - lsb >= max_lsb / 2 {
            prev_msb + max_lsb
        } else if lsb > prev_lsb && lsb - prev_lsb > max_lsb / 2 {
            prev_msb - max_lsb
        } else {
            prev_msb
        };

        let mut top = msb + lsb;
        let mut bottom = top + delta_bottom;
        if input.has_mmco5 {
            let poc = top.min(bottom);
            top -= poc;
            bottom -= poc;
        }

        // Type 0 predicts from the most recent reference frame only.
        if input.nal_ref_idc != 0 {
            if input.has_mmco5 {
                self.prev_poc_msb = 0;
                self.prev_poc_lsb = top;
            } else {
                self.prev_poc_msb = msb;
                self.prev_poc_lsb = lsb;
            }
        }

        Ok(Poc { top, bottom })
    }

    /// Spec 8.2.1.2: POC derived from `frame_num` through the expected-delta cycle in the SPS.
    fn compute_type1(
        &mut self,
        sps: &SeqParameterSet,
        offset_for_non_ref_pic: i32,
        offset_for_top_to_bottom_field: i32,
        offsets_for_ref_frame: &[i32],
        input: &PocInput<'_>,
    ) -> Result<Poc, ParseError> {
        let frame_num_offset = self.frame_num_offset(sps, input);
        let frame_num = u32::from(input.frame_num);

        let mut abs_frame_num = if offsets_for_ref_frame.is_empty() {
            0
        } else {
            frame_num_offset + frame_num
        };
        if input.nal_ref_idc == 0 && abs_frame_num > 0 {
            abs_frame_num -= 1;
        }

        let mut expected: i64 = if abs_frame_num > 0 {
            let num_in_cycle = offsets_for_ref_frame.len() as u32;
            let cycle_count = (abs_frame_num - 1) / num_in_cycle;
            let index_in_cycle = ((abs_frame_num - 1) % num_in_cycle) as usize;

            let delta_per_cycle: i64 = offsets_for_ref_frame
                .iter()
                .map(|&offset| i64::from(offset))
                .sum();
            let delta_in_cycle: i64 = offsets_for_ref_frame[..=index_in_cycle]
                .iter()
                .map(|&offset| i64::from(offset))
                .sum();
            i64::from(cycle_count) * delta_per_cycle + delta_in_cycle
        } else {
            0
        };
        if input.nal_ref_idc == 0 {
            expected += i64::from(offset_for_non_ref_pic);
        }

        let [delta0, delta1] = match input.pic_order_cnt_lsb {
            Some(PicOrderCountLsb::FieldsDelta(deltas)) => *deltas,
            _ => return Err(ParseError::PocSyntaxMismatch),
        };

        let mut top = (expected + i64::from(delta0)) as i32;
        let mut bottom = top + offset_for_top_to_bottom_field + delta1;
        if input.has_mmco5 {
            let poc = top.min(bottom);
            top -= poc;
            bottom -= poc;
        }

        self.advance_frame_num(frame_num, frame_num_offset, input.has_mmco5);
        Ok(Poc { top, bottom })
    }

    /// Spec 8.2.1.3: POC is display order == decoding order, derived from `frame_num` alone.
    fn compute_type2(&mut self, sps: &SeqParameterSet, input: &PocInput<'_>) -> Poc {
        let frame_num_offset = self.frame_num_offset(sps, input);
        let frame_num = u32::from(input.frame_num);

        let poc = if input.is_idr || input.has_mmco5 {
            // An MMCO 5 rebase results in 0 here: top == bottom, so the frame's own
            // order count gets subtracted from itself.
            0
        } else if input.nal_ref_idc == 0 {
            2 * (frame_num_offset + frame_num).cast_signed() - 1
        } else {
            2 * (frame_num_offset + frame_num).cast_signed()
        };

        self.advance_frame_num(frame_num, frame_num_offset, input.has_mmco5);
        Poc {
            top: poc,
            bottom: poc,
        }
    }

    /// `FrameNumOffset` accumulation shared by types 1 and 2:
    /// each `frame_num` wraparound adds `MaxFrameNum`.
    fn frame_num_offset(&self, sps: &SeqParameterSet, input: &PocInput<'_>) -> u32 {
        if input.is_idr {
            0
        } else if self.prev_frame_num > u32::from(input.frame_num) {
            self.prev_frame_num_offset + (1 << sps.log2_max_frame_num())
        } else {
            self.prev_frame_num_offset
        }
    }

    fn advance_frame_num(&mut self, frame_num: u32, frame_num_offset: u32, has_mmco5: bool) {
        if has_mmco5 {
            // The current frame is treated as if it had `frame_num` 0 from here on.
            self.prev_frame_num = 0;
            self.prev_frame_num_offset = 0;
        } else {
            self.prev_frame_num = frame_num;
            self.prev_frame_num_offset = frame_num_offset;
        }
    }
}
