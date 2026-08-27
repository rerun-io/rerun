//! Grouping slice NALs into pictures.
//!
//! One pushed access unit normally holds exactly one frame, but a frame can consist of
//! several slices, and defensively one push may hold several frames. A slice with
//! `first_mb_in_slice == 0` starts a new picture (spec 7.4.1.2.4 lists more conditions,
//! but they only matter for arbitrary slice order, which [`super::Parser`] rejects by
//! requiring slices in raster order).

use super::ParseError;
use super::parse::ParsedSlice;

/// The slices of the picture being assembled.
pub struct PendingPicture {
    pub slices: Vec<ParsedSlice>,
}

impl PendingPicture {
    pub fn new(first_slice: ParsedSlice) -> Result<Self, ParseError> {
        if first_slice.header.first_mb_in_slice != 0 {
            // A picture must start at the top: getting its tail first means the pushed
            // access unit was cut mid-frame.
            return Err(ParseError::IncompletePicture);
        }
        Ok(Self {
            slices: vec![first_slice],
        })
    }

    pub fn first(&self) -> &ParsedSlice {
        &self.slices[0]
    }

    /// Whether the slice starts the next picture instead of continuing the pending one.
    pub fn is_picture_boundary(slice: &ParsedSlice) -> bool {
        slice.header.first_mb_in_slice == 0
    }

    /// Adds a continuation slice, validating it belongs to the same picture
    /// and keeps raster order.
    pub fn push(&mut self, slice: ParsedSlice) -> Result<(), ParseError> {
        let first = self.first();
        let last = self.slices.last().expect("at least the first slice");

        let consistent = slice.pps_id == first.pps_id
            && slice.is_idr == first.is_idr
            && slice.nal_ref_idc == first.nal_ref_idc
            && slice.header.frame_num == first.header.frame_num
            && slice.header.idr_pic_id == first.header.idr_pic_id;
        if !consistent {
            return Err(ParseError::InconsistentSlices);
        }
        if slice.header.first_mb_in_slice <= last.header.first_mb_in_slice {
            // Arbitrary slice order is a baseline-profile feature that neither the
            // picture boundary detection nor hardware decoders support.
            return Err(ParseError::Unsupported("slices out of raster order"));
        }

        self.slices.push(slice);
        Ok(())
    }
}
