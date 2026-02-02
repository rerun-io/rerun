//! General H.265 utilities.
//!
use cros_codecs::codec::h264::nalu::Header as _;
use cros_codecs::codec::h265::parser::{Nalu, NaluType, Parser, ProfileTierLevel, Sps};

use crate::nalu::{
    ANNEXB_NAL_START_CODE, AnnexBStreamState, AnnexBStreamWriteError,
    write_length_prefixed_nalus_to_annexb_stream,
};
use crate::{
    ChromaSubsamplingModes, Chunk, DetectGopStartError, GopStartDetection, VideoEncodingDetails,
};

/// Retrieve [`VideoEncodingDetails`] from an H.265 SPS.
pub fn encoding_details_from_h265_sps(sps: &Sps) -> VideoEncodingDetails {
    let codec_string = hevc_codec_string(&sps.profile_tier_level);
    let width = sps.width();
    let height = sps.height();
    let coded_dimensions = [width, height];

    let bit_depth = Some(sps.bit_depth_luma_minus8 + 8);

    let chroma_subsampling = match sps.chroma_format_idc {
        0 => Some(ChromaSubsamplingModes::Monochrome),
        1 => Some(ChromaSubsamplingModes::Yuv420),
        2 => Some(ChromaSubsamplingModes::Yuv422),
        3 => Some(ChromaSubsamplingModes::Yuv444),
        _ => None,
    };

    VideoEncodingDetails {
        codec_string,
        coded_dimensions,
        bit_depth,
        chroma_subsampling,
        stsd: None,
    }
}

/// Builds a codec string for HEVC.
fn hevc_codec_string(profile_tier_level: &ProfileTierLevel) -> String {
    // See <https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/codecs_parameter#hevc_mp4_quicktime_matroska>
    // Codec string has the form hvc1[.A.B.C.D]
    let mut codec = "hvc1.".to_owned();

    // .A: The general_profile_space. This is encoded as one or two characters.
    match profile_tier_level.general_profile_space {
        1 => codec.push('A'),
        2 => codec.push('B'),
        3 => codec.push('C'),
        _ => {}
    }
    codec.push_str(&format!("{}", profile_tier_level.general_profile_idc));

    // .B: A 32-bit value representing one or more general profile compatibility flags.
    let mut reversed = 0;
    for i in 0..32 {
        reversed |= profile_tier_level.general_profile_compatibility_flag[i] as u32;
        if i != 31 {
            reversed <<= 1;
        }
    }
    codec.push_str(format!(".{reversed:X}").trim_end_matches('0'));

    // .C: The general_tier_flag, encoded as L (general_tier_flag === 0) or H (general_tier_flag === 1), followed by the general_level_idc, encoded as a decimal number.
    if profile_tier_level.general_tier_flag {
        codec.push_str(".H");
    } else {
        codec.push_str(".L");
    }
    codec.push_str(&format!("{}", profile_tier_level.general_level_idc as u8));

    // .D: One or more 6-byte constraint flags. Note that each flag is encoded as a hexadecimal number, and separated by an additional period; trailing bytes that are zero may be omitted.
    let mut constraints = [0u8; 2];
    // Build constraint indicators from individual constraint flags
    // Everything is in reverse order!
    constraints[1] |= (profile_tier_level.general_progressive_source_flag as u8) << 7;
    constraints[1] |= (profile_tier_level.general_interlaced_source_flag as u8) << 6;
    constraints[1] |= (profile_tier_level.general_non_packed_constraint_flag as u8) << 5;
    constraints[1] |= (profile_tier_level.general_frame_only_constraint_flag as u8) << 4;
    constraints[1] |= (profile_tier_level.general_max_12bit_constraint_flag as u8) << 3;
    constraints[1] |= (profile_tier_level.general_max_10bit_constraint_flag as u8) << 2;
    constraints[1] |= (profile_tier_level.general_max_8bit_constraint_flag as u8) << 1;
    constraints[1] |= profile_tier_level.general_max_422chroma_constraint_flag as u8;

    constraints[0] |= (profile_tier_level.general_max_420chroma_constraint_flag as u8) << 7;
    constraints[0] |= (profile_tier_level.general_max_monochrome_constraint_flag as u8) << 6;
    constraints[0] |= (profile_tier_level.general_intra_constraint_flag as u8) << 5;
    constraints[0] |= (profile_tier_level.general_one_picture_only_constraint_flag as u8) << 4;
    constraints[0] |= (profile_tier_level.general_lower_bit_rate_constraint_flag as u8) << 3;
    constraints[0] |= (profile_tier_level.general_max_14bit_constraint_flag as u8) << 2;

    let mut has_byte = false;
    for constraint in constraints {
        if constraint > 0 || has_byte {
            codec.push_str(&format!(".{constraint:X}"));
            has_byte = true;
        }
    }

    codec
}

pub fn detect_h265_annexb_gop(data: &[u8]) -> Result<GopStartDetection, DetectGopStartError> {
    let mut parser = Parser::default();
    let mut details: Option<VideoEncodingDetails> = None;
    let mut idr_found = false;
    let mut cursor = std::io::Cursor::new(data);

    while let Ok(nalu) = Nalu::next(&mut cursor) {
        match nalu.header.type_ {
            NaluType::SpsNut if details.is_none() => {
                if nalu.as_ref().len() < nalu.header.len() {
                    // Prevent panic inside of `parse_sps`.
                    return Err(DetectGopStartError::FailedToExtractEncodingDetails(
                        "SPS NALU is incomplete".to_owned(),
                    ));
                }

                // parse_sps returns &Sps, so bind to a reference
                let sps_ref: &Sps = parser
                    .parse_sps(&nalu)
                    .map_err(DetectGopStartError::FailedToExtractEncodingDetails)?;

                // convert into your VideoEncodingDetails
                details = Some(encoding_details_from_h265_sps(sps_ref));
            }
            t if t.is_idr() => {
                idr_found = true;
            }
            _ => {}
        }
        if idr_found && details.is_some() {
            break;
        }
    }

    if idr_found {
        if let Some(ved) = details {
            Ok(GopStartDetection::StartOfGop(ved))
        } else {
            // saw IDR but no SPS → not useful
            Ok(GopStartDetection::NotStartOfGop)
        }
    } else {
        Ok(GopStartDetection::NotStartOfGop)
    }
}

/// Write an H.265 chunk to an Annex B stream without state tracking.
///
/// This is a fully re-entrant utility that allows explicit control over parameter set emission.
/// Typically you'd pass `chunk.is_sync` to emit parameter sets for IDR frames only.
pub fn write_hevc_chunk_to_annexb(
    hvcc: &re_mp4::HevcBox,
    nalu_stream: &mut dyn std::io::Write,
    emit_parameter_sets: bool,
    chunk: &Chunk,
) -> Result<(), AnnexBStreamWriteError> {
    // Emit VPS/SPS/PPS parameter sets if requested
    if emit_parameter_sets {
        for arr in &hvcc.hvcc.arrays {
            if let Ok(nalu_type) = NaluType::try_from(arr.nal_unit_type as u32)
                && matches!(
                    nalu_type,
                    NaluType::VpsNut | NaluType::SpsNut | NaluType::PpsNut
                )
            {
                for nalu in &arr.nalus {
                    nalu_stream
                        .write_all(ANNEXB_NAL_START_CODE)
                        .map_err(AnnexBStreamWriteError::FailedToWriteToStream)?;
                    nalu_stream
                        .write_all(&nalu.data)
                        .map_err(AnnexBStreamWriteError::FailedToWriteToStream)?;
                }
            }
        }
    }

    // Each NAL unit in mp4 is prefixed with a length prefix.
    // In Annex B this doesn't exist.
    let length_prefix_size = (hvcc.hvcc.contents.length_size_minus_one as usize & 0x03) + 1;

    write_length_prefixed_nalus_to_annexb_stream(nalu_stream, &chunk.data, length_prefix_size)
}

pub fn write_hevc_chunk_to_nalu_stream(
    hvcc: &re_mp4::HevcBox,
    nalu_stream: &mut dyn std::io::Write,
    chunk: &Chunk,
    state: &mut AnnexBStreamState,
) -> Result<(), AnnexBStreamWriteError> {
    let emit_parameter_sets = chunk.is_sync && !state.previous_frame_was_idr;

    write_hevc_chunk_to_annexb(hvcc, nalu_stream, emit_parameter_sets, chunk)?;
    state.previous_frame_was_idr = emit_parameter_sets;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::{GopStartDetection, detect_h265_annexb_gop};
    use crate::{ChromaSubsamplingModes, DetectGopStartError, VideoEncodingDetails};

    #[test]
    fn test_detect_h265_annexb_gop() {
        // Example H.265 Annex B encoded data containing VPS, SPS and IDR frame. (extracted from "tests/assets/video/Big_Buck_Bunny_1080_1s_h265.mp4")
        let sample_data = &[
            // VPS NAL unit (NAL type 32)
            0x00, 0x00, 0x00, 0x01, 0x40, 0x01, 0x0c, 0x01, 0xff, 0xff, 0x01, 0x60, 0x00, 0x00,
            0x03, 0x00, 0x90, 0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00, 0x78, 0x95, 0x98, 0x09,
            // SPS NAL unit (NAL type 33)
            0x00, 0x00, 0x00, 0x01, 0x42, 0x01, 0x01, 0x01, 0x60, 0x00, 0x00, 0x03, 0x00, 0x90,
            0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00, 0x78, 0xa0, 0x03, 0xc0, 0x80, 0x10, 0xe5,
            0x96, 0x56, 0x69, 0x24, 0xca, 0xf0, 0x16, 0x9c, 0x20, 0x00, 0x00, 0x03, 0x00, 0x20,
            0x00, 0x00, 0x03, 0x03, 0xc1, //
            // PPS NAL unit (NAL type 34)
            0x00, 0x00, 0x00, 0x01, 0x44, 0x01, 0xc1, 0x72, 0xb4, 0x62, 0x40,
            // IDR frame NAL unit (NAL type 19)
            0x00, 0x00, 0x00, 0x01, 0x26, 0x01, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54,
            0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A,
        ];
        let result = detect_h265_annexb_gop(sample_data);
        assert_eq!(
            result,
            Ok(GopStartDetection::StartOfGop(VideoEncodingDetails {
                codec_string: "hvc1.1.6.L120.90".to_owned(),
                coded_dimensions: [1920, 1080],
                bit_depth: Some(8),
                chroma_subsampling: Some(ChromaSubsamplingModes::Yuv420),
                stsd: None,
            }))
        );

        // Example H.265 Annex B encoded data containing broken SPS and IDR frame. (above example but messed with the SPS)
        let sample_data = &[
            // VPS NAL unit (NAL type 32)
            0x00, 0x00, 0x00, 0x01, 0x40, 0x01, 0x0C, 0x01, 0xFF, 0xFF, 0x01, 0x60, 0x00, 0x00,
            0x03, 0x00, 0x90, 0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00, 0x5D, 0x95, 0x98, 0x09,
            // Broken SPS NAL unit (NAL type 33)
            0x00, 0x00, 0x00, 0x01, 0x42, 0x00, 0x00, 0x01, 0x60, 0x00, 0x00, 0x03, 0x00, 0x90,
            0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00, 0x5D, 0xA0, 0x02, 0x80, 0x80, 0x2D, 0x1F,
            0xE5, 0x8E, 0x49, 0x24, 0x94, 0x92, 0x49, 0x24, 0x92, 0x49, 0x24, 0x94, 0x92, 0x49,
            // IDR frame NAL unit (NAL type 19)
            0x00, 0x00, 0x00, 0x01, 0x26, 0x01, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54,
            0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A,
        ];
        let result = detect_h265_annexb_gop(sample_data);
        assert_eq!(
            result,
            Err(DetectGopStartError::FailedToExtractEncodingDetails(
                "SPS NALU is incomplete".to_owned()
            ))
        );

        // Garbage data, still annex b shaped. (ai generated)
        let sample_data = &[
            0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84,
            0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11,
            0x80,
        ];
        let result = detect_h265_annexb_gop(sample_data);
        assert_eq!(result, Ok(GopStartDetection::NotStartOfGop));

        // Garbage data, no detectable nalu units.
        let sample_data = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A];
        let result = detect_h265_annexb_gop(sample_data);
        assert_eq!(result, Ok(GopStartDetection::NotStartOfGop));
    }
}
