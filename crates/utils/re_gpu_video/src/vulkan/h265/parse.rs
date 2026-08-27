//! Glue around the `cros-codecs` H.265 parser: NAL construction and the checks
//! that keep unsupported streams out.
//!
//! `cros-codecs` parses NALs found by its own cursor, whose offsets are relative
//! to each NAL rather than to the pushed buffer. The access unit handling needs
//! absolute ranges, so the NALs come from [`crate::vulkan::annexb::nal_ranges`]
//! and are handed to the parser one at a time.

use std::borrow::Cow;
use std::ops::Range;

use cros_codecs::codec::h264::nalu::Header as _;
use cros_codecs::codec::h265::parser::{Nalu, NaluHeader, Parser, SliceHeader, Sps};

use crate::ParseError;

/// Builds a `cros-codecs` NAL over one byte range of the pushed buffer.
pub fn nalu<'a>(data: &'a [u8], range: &Range<usize>) -> Result<Nalu<'a>, ParseError> {
    let bytes = &data[range.clone()];
    let header = NaluHeader::parse(&mut std::io::Cursor::new(bytes))
        .map_err(|err| ParseError::nal("NAL header", err))?;
    if bytes.len() < header.len() {
        return Err(ParseError::Invalid("NAL unit is truncated"));
    }
    Ok(Nalu {
        header,
        data: Cow::Borrowed(bytes),
        size: bytes.len(),
        offset: 0,
    })
}

/// Parses a VPS, returning its id.
pub fn parse_vps(parser: &mut Parser, nalu: &Nalu<'_>) -> Result<u8, ParseError> {
    let vps = parser
        .parse_vps(nalu)
        .map_err(|err| ParseError::nal("VPS", err))?;
    Ok(vps.video_parameter_set_id)
}

/// Parses an SPS and rejects everything the backend can't decode.
pub fn parse_sps<'a>(parser: &'a mut Parser, nalu: &Nalu<'_>) -> Result<&'a Sps, ParseError> {
    let sps = parser
        .parse_sps(nalu)
        .map_err(|err| ParseError::nal("SPS", err))?;

    // 4:2:0 8-bit progressive only, matching the probed decode profile.
    if sps.chroma_format_idc != 1 || sps.separate_colour_plane_flag {
        return Err(ParseError::Unsupported("chroma format other than 4:2:0"));
    }
    if sps.bit_depth_luma_minus8 != 0 || sps.bit_depth_chroma_minus8 != 0 {
        return Err(ParseError::Unsupported("bit depth other than 8"));
    }
    if sps.scc_extension_flag {
        return Err(ParseError::Unsupported("screen content coding extensions"));
    }
    if sps.range_extension_flag {
        return Err(ParseError::Unsupported("range extensions"));
    }
    if sps.pcm_enabled_flag {
        return Err(ParseError::Unsupported("pulse code modulation samples"));
    }
    if !sps.profile_tier_level.general_progressive_source_flag
        && sps.profile_tier_level.general_interlaced_source_flag
    {
        return Err(ParseError::Unsupported("interlaced video (field pictures)"));
    }

    Ok(sps)
}

/// Parses a PPS, returning its id.
pub fn parse_pps(parser: &mut Parser, nalu: &Nalu<'_>) -> Result<u8, ParseError> {
    let pps = parser
        .parse_pps(nalu)
        .map_err(|err| ParseError::nal("PPS", err))?;

    if pps.scc_extension_flag {
        return Err(ParseError::Unsupported("screen content coding extensions"));
    }

    Ok(pps.pic_parameter_set_id)
}

/// One slice segment NAL with its parsed header.
pub struct ParsedSlice {
    /// The slice NAL bytes (without start code) in the pushed access unit.
    pub range: Range<usize>,

    pub header: SliceHeader,

    /// `nuh_temporal_id`, the sub-layer this slice belongs to.
    pub temporal_id: u8,

    pub is_idr: bool,

    /// An intra random access point: IDR, BLA, or CRA.
    pub is_irap: bool,

    /// A random access skipped leading picture: it precedes its random access
    /// point in presentation order and predicts from pictures before it.
    pub is_rasl: bool,

    /// The picture may never anchor the picture order counts of the following
    /// ones: a RASL, RADL, or sub-layer non-reference picture.
    pub is_rasl_radl_or_slnr: bool,
}

pub fn parse_slice(
    parser: &mut Parser,
    data: &[u8],
    range: Range<usize>,
) -> Result<ParsedSlice, ParseError> {
    let nalu = nalu(data, &range)?;
    let nal_type = nalu.header.type_;
    let temporal_id = nalu.header.nuh_temporal_id();
    if nalu.header.nuh_layer_id != 0 {
        return Err(ParseError::Unsupported("layered extensions (SHVC/MV-HEVC)"));
    }

    let slice = parser
        .parse_slice_header(nalu)
        .map_err(|err| ParseError::nal("slice header", err))?;

    Ok(ParsedSlice {
        range,
        temporal_id,
        is_idr: nal_type.is_idr(),
        is_irap: nal_type.is_irap(),
        is_rasl: nal_type.is_rasl(),
        is_rasl_radl_or_slnr: nal_type.is_rasl() || nal_type.is_radl() || nal_type.is_slnr(),
        header: slice.header,
    })
}
