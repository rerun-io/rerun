//! Glue around the `h264-reader` crate: NAL splitting and syntax-level parsing.

use std::ops::Range;

use h264_reader::Context;
use h264_reader::nal::{
    Nal as _, NalHeader, RefNal, UnitType,
    pps::PicParameterSet,
    slice::{FieldPic, SliceHeader},
    sps::SeqParameterSet,
};
use re_video_parsing::SpsInfo;

use super::ParseError;

/// A slice NAL with its parsed header, detached from the `h264-reader` context borrows.
pub struct ParsedSlice {
    /// The slice NAL bytes (without start code) in the pushed access unit.
    pub range: Range<usize>,

    pub nal_ref_idc: u8,
    pub is_idr: bool,
    pub sps_id: u8,
    pub pps_id: u8,
    pub header: SliceHeader,
}

/// Parses and validates a slice header against the parameter sets seen so far.
pub fn parse_slice(
    ctx: &Context,
    nal_header: NalHeader,
    nal: &RefNal<'_>,
    range: Range<usize>,
) -> Result<ParsedSlice, ParseError> {
    let (header, sps, pps) = SliceHeader::from_bits(ctx, &mut nal.rbsp_bits(), nal_header)
        .map_err(|err| ParseError::nal("slice header", err))?;

    if header.field_pic != FieldPic::Frame {
        return Err(ParseError::Unsupported("interlaced video (field pictures)"));
    }
    if header.redundant_pic_cnt.is_some_and(|count| count > 0) {
        return Err(ParseError::Unsupported("redundant coded pictures"));
    }

    Ok(ParsedSlice {
        range,
        nal_ref_idc: nal_header.nal_ref_idc(),
        is_idr: nal_header.nal_unit_type() == UnitType::SliceLayerWithoutPartitioningIdr,
        sps_id: sps.seq_parameter_set_id.id(),
        pps_id: pps.pic_parameter_set_id.id(),
        header,
    })
}

/// Parses an SPS and rejects streams the Vulkan H.264 decode profile can't handle.
pub fn parse_sps(nal: &RefNal<'_>) -> Result<SeqParameterSet, ParseError> {
    let sps =
        SeqParameterSet::from_bits(nal.rbsp_bits()).map_err(|err| ParseError::nal("SPS", err))?;

    let info = SpsInfo::new(&sps).map_err(|err| ParseError::nal("SPS", err))?;
    if let Some(unsupported) = crate::h264_unsupported_bitstream(&info) {
        return Err(ParseError::UnsupportedStream(unsupported));
    }

    Ok(sps)
}

/// Parses a PPS and rejects streams the Vulkan H.264 decode profile can't handle.
pub fn parse_pps(ctx: &Context, nal: &RefNal<'_>) -> Result<PicParameterSet, ParseError> {
    let pps = PicParameterSet::from_bits(ctx, nal.rbsp_bits())
        .map_err(|err| ParseError::nal("PPS", err))?;

    if pps.slice_groups.is_some() {
        return Err(ParseError::Unsupported("slice groups (FMO)"));
    }

    Ok(pps)
}
