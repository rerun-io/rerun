//! Picture order count derivation, spec 8.3.1.
//!
//! Much simpler than H.264's: one variant, no field pictures, no MMCO. The whole
//! process is a sliding most-significant-bits counter over `slice_pic_order_cnt_lsb`,
//! anchored on the previous picture that may serve as a reference for it.

use crate::ParseError;

/// What [`PocState::compute`] needs about the current picture.
pub struct PocInput {
    /// `slice_pic_order_cnt_lsb`, 0 for IDR pictures where it is absent.
    pub pic_order_cnt_lsb: u16,

    /// `log2_max_pic_order_cnt_lsb_minus4` of the active SPS.
    pub log2_max_pic_order_cnt_lsb_minus4: u8,

    /// The picture is an intra random access point that starts a new prediction
    /// sequence (`NoRaslOutputFlag` equal to 1): every IDR and BLA picture, and a
    /// CRA picture that opens the stream or follows a seek.
    ///
    /// Resets the most-significant bits to zero.
    pub starts_sequence: bool,

    pub temporal_id: u8,

    /// The picture is a RASL, RADL, or sub-layer non-reference picture, and so
    /// can never become the anchor for the following pictures' counts.
    pub is_rasl_radl_or_slnr: bool,
}

/// The rolling most-significant bits of the picture order count.
#[derive(Default)]
pub struct PocState {
    /// `PicOrderCntMsb` of `prevTid0Pic`.
    prev_msb: i32,

    /// `slice_pic_order_cnt_lsb` of `prevTid0Pic`.
    prev_lsb: i32,

    /// A `prevTid0Pic` exists: some picture was decoded since the last reset.
    has_prev: bool,
}

impl PocState {
    /// Drops the anchor picture: the next picture must start a new sequence.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Computes `PicOrderCntVal` of the current picture and updates the anchor.
    pub fn compute(&mut self, input: &PocInput) -> Result<i32, ParseError> {
        let log2_max_lsb = u32::from(input.log2_max_pic_order_cnt_lsb_minus4) + 4;
        if log2_max_lsb > 16 {
            return Err(ParseError::Invalid(
                "log2_max_pic_order_cnt_lsb out of range",
            ));
        }
        let max_lsb = 1_i32 << log2_max_lsb;
        let lsb = i32::from(input.pic_order_cnt_lsb);
        if lsb >= max_lsb {
            return Err(ParseError::Invalid(
                "slice_pic_order_cnt_lsb exceeds its declared width",
            ));
        }

        let msb = if input.starts_sequence || !self.has_prev {
            0
        } else if lsb < self.prev_lsb && (self.prev_lsb - lsb) >= max_lsb / 2 {
            self.prev_msb + max_lsb
        } else if lsb > self.prev_lsb && (lsb - self.prev_lsb) > max_lsb / 2 {
            self.prev_msb - max_lsb
        } else {
            self.prev_msb
        };

        // `prevTid0Pic`: the previous picture in decoding order with a temporal id
        // of 0 that is none of RASL, RADL, or sub-layer non-reference.
        if input.temporal_id == 0 && !input.is_rasl_radl_or_slnr {
            self.prev_msb = msb;
            self.prev_lsb = lsb;
            self.has_prev = true;
        }

        Ok(msb + lsb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(lsb: u16, starts_sequence: bool) -> PocInput {
        PocInput {
            pic_order_cnt_lsb: lsb,
            // A 16-entry count, wrapping at 16.
            log2_max_pic_order_cnt_lsb_minus4: 0,
            starts_sequence,
            temporal_id: 0,
            is_rasl_radl_or_slnr: false,
        }
    }

    /// An IDR anchors the count at zero and the following pictures count up from it.
    #[test]
    fn counts_up_from_the_random_access_point() {
        let mut poc = PocState::default();
        assert_eq!(poc.compute(&input(0, true)).unwrap(), 0);
        assert_eq!(poc.compute(&input(4, false)).unwrap(), 4);
        assert_eq!(poc.compute(&input(2, false)).unwrap(), 2);
        assert_eq!(poc.compute(&input(6, false)).unwrap(), 6);
    }

    /// Once the least-significant bits wrap around, the most-significant bits
    /// carry, so the counts keep growing.
    #[test]
    fn wraps_the_least_significant_bits() {
        let mut poc = PocState::default();
        assert_eq!(poc.compute(&input(0, true)).unwrap(), 0);
        for lsb in 1..16 {
            assert_eq!(poc.compute(&input(lsb, false)).unwrap(), i32::from(lsb));
        }
        assert_eq!(poc.compute(&input(0, false)).unwrap(), 16);
        assert_eq!(poc.compute(&input(1, false)).unwrap(), 17);
    }

    /// A picture that arrives after the wrap but is presented before it keeps the
    /// lower count, the anchor does not drag it into the new cycle.
    #[test]
    fn a_late_picture_before_the_wrap_keeps_its_count() {
        let mut poc = PocState::default();
        assert_eq!(poc.compute(&input(0, true)).unwrap(), 0);
        for lsb in 1..15 {
            assert_eq!(poc.compute(&input(lsb, false)).unwrap(), i32::from(lsb));
        }
        // The wrap: the anchor moves into the second cycle.
        assert_eq!(poc.compute(&input(0, false)).unwrap(), 16);
        // A picture from the first cycle, arriving out of order.
        assert_eq!(poc.compute(&input(15, false)).unwrap(), 15);
    }

    /// Pictures that may not anchor the count leave the previous anchor in place.
    #[test]
    fn non_anchor_pictures_do_not_move_the_anchor() {
        let mut poc = PocState::default();
        assert_eq!(poc.compute(&input(0, true)).unwrap(), 0);
        assert_eq!(poc.compute(&input(8, false)).unwrap(), 8);

        let mut skipped = input(14, false);
        skipped.is_rasl_radl_or_slnr = true;
        assert_eq!(poc.compute(&skipped).unwrap(), 14);

        let mut higher_layer = input(15, false);
        higher_layer.temporal_id = 1;
        assert_eq!(poc.compute(&higher_layer).unwrap(), 15);

        // The anchor is still the picture with count 8: from there 1 is 7 steps
        // down, below the half-cycle threshold, so no carry happens.
        assert_eq!(poc.compute(&input(1, false)).unwrap(), 1);
    }

    /// A random access point in the middle of a stream restarts the
    /// most-significant bits, keeping only what its own bits say.
    #[test]
    fn a_mid_stream_random_access_point_resets_the_counter() {
        let mut poc = PocState::default();
        assert_eq!(poc.compute(&input(0, true)).unwrap(), 0);
        for lsb in 1..16 {
            assert_eq!(poc.compute(&input(lsb, false)).unwrap(), i32::from(lsb));
        }
        // Well into the second cycle.
        assert_eq!(poc.compute(&input(0, false)).unwrap(), 16);
        assert_eq!(poc.compute(&input(1, false)).unwrap(), 17);
        // A random access point drops the accumulated cycles.
        assert_eq!(poc.compute(&input(6, true)).unwrap(), 6);
        assert_eq!(poc.compute(&input(7, false)).unwrap(), 7);
    }

    /// Counts wider than the declared bit width are rejected.
    #[test]
    fn out_of_range_counts_are_rejected() {
        let mut poc = PocState::default();
        assert!(poc.compute(&input(16, true)).is_err());
    }
}
