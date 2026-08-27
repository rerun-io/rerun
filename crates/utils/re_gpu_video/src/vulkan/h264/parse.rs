//! Glue around the `h264-reader` crate: NAL splitting and syntax-level parsing.

use std::ops::Range;

use h264_reader::Context;
use h264_reader::nal::{
    Nal as _, NalHeader, RefNal, UnitType,
    pps::PicParameterSet,
    slice::{FieldPic, SliceHeader},
    sps::{ChromaFormat, FrameMbsFlags, SeqParameterSet},
};

use super::ParseError;

/// Splits an annex-b stream into the byte ranges of its NALs, without start codes.
///
/// Handles both 3- and 4-byte start codes and strips trailing zero padding from each NAL.
/// Anything but zero padding before the first start code is an error, it usually means
/// the data is length-prefixed (AVCC) instead of annex-b.
pub fn nal_ranges(data: &[u8]) -> Result<Vec<Range<usize>>, ParseError> {
    let mut ranges = Vec::new();

    // Start of the NAL following the most recent start code, if any.
    let mut nal_start = None;

    let mut close_nal = |nal_start: Option<usize>, end: usize| {
        if let Some(start) = nal_start {
            // Zero padding after a NAL is either alignment/`cabac_zero_words`
            // or the leading zero of a 4-byte start code. Neither belongs to the NAL.
            let end = data[start..end]
                .iter()
                .rposition(|&byte| byte != 0)
                .map_or(start, |last_non_zero| start + last_non_zero + 1);
            if end > start {
                ranges.push(start..end);
            }
        }
    };

    let mut i = 0;
    while i + 2 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            close_nal(nal_start, i);
            if nal_start.is_none() && data[..i].iter().any(|&byte| byte != 0) {
                return Err(ParseError::NotAnnexB);
            }
            nal_start = Some(i + 3);
            i += 3;
        } else if data[i + 2] > 1 {
            // This byte can be part of no start code, skip past it.
            i += 3;
        } else {
            i += 1;
        }
    }
    close_nal(nal_start, data.len());

    if nal_start.is_none() && data.iter().any(|&byte| byte != 0) {
        return Err(ParseError::NotAnnexB);
    }

    Ok(ranges)
}

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

    if !matches!(u8::from(sps.profile_idc), 66 | 77 | 100) {
        return Err(ParseError::Unsupported(
            "profile (only Baseline, Main, and High)",
        ));
    }
    if !matches!(sps.frame_mbs_flags, FrameMbsFlags::Frames) {
        return Err(ParseError::Unsupported(
            "interlaced video (frame_mbs_only_flag == 0)",
        ));
    }
    if sps.chroma_info.chroma_format != ChromaFormat::YUV420 {
        return Err(ParseError::Unsupported(
            "chroma subsampling other than 4:2:0",
        ));
    }
    if sps.chroma_info.bit_depth_luma_minus8 != 0 || sps.chroma_info.bit_depth_chroma_minus8 != 0 {
        return Err(ParseError::Unsupported("bit depth other than 8"));
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

#[cfg(test)]
mod tests {
    use super::nal_ranges;
    use crate::vulkan::h264::ParseError;

    #[test]
    fn nal_ranges_start_codes() {
        // 3- and 4-byte start codes, trailing zero padding, empty input.
        assert_eq!(
            nal_ranges(&[]).unwrap(),
            Vec::<std::ops::Range<usize>>::new()
        );
        assert_eq!(nal_ranges(&[0, 0, 1, 0x65, 0xff]).unwrap(), vec![3..5]);
        assert_eq!(nal_ranges(&[0, 0, 0, 1, 0x65, 0xff]).unwrap(), vec![4..6]);
        assert_eq!(
            nal_ranges(&[0, 0, 1, 0x67, 0x42, 0, 0, 0, 1, 0x68, 0xce]).unwrap(),
            vec![3..5, 9..11]
        );
        // Trailing zeros after the last NAL are stripped.
        assert_eq!(
            nal_ranges(&[0, 0, 1, 0x65, 0xff, 0, 0]).unwrap(),
            vec![3..5]
        );
        // A NAL that is all zeros is dropped entirely.
        assert_eq!(
            nal_ranges(&[0, 0, 1, 0, 0]).unwrap(),
            Vec::<std::ops::Range<usize>>::new()
        );
    }

    #[test]
    fn nal_ranges_rejects_non_annexb() {
        // Length-prefixed (AVCC) data has no leading start code.
        assert!(matches!(
            nal_ranges(&[0, 0, 0, 2, 0x65, 0xff]),
            Err(ParseError::NotAnnexB)
        ));
        assert!(matches!(
            nal_ranges(&[0x65, 0xff]),
            Err(ParseError::NotAnnexB)
        ));
    }
}
