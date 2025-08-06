//! General H.265 utilities.
//!
use crate::{ChromaSubsamplingModes, VideoEncodingDetails};
use cros_codecs::codec::h265::parser::Sps;

/// Retrieve [`VideoEncodingDetails`] from an H.265 SPS.
pub fn encoding_details_from_h265_sps(sps: &Sps) -> VideoEncodingDetails {
    // 1) Build the codec string from profile and level:
    let profile_idc = sps.profile_tier_level.general_profile_idc;
    let level_idc: u8 = sps.profile_tier_level.general_level_idc as u8;
    // WebCodecs HEVC strings are usually "hvc1.<profile>.<level>" (with optional flags)
    let codec_string = format!(
        "hvc1.{profile:02X}.L{level:02}",
        profile = profile_idc,
        level = level_idc
    );

    // 2) Dimensions (accounts for any conformance window):
    let width = sps.width();
    let height = sps.height();
    let coded_dimensions = [width, height];

    // 3) Bit depth = luma_minus8 + 8
    let bit_depth = Some(sps.bit_depth_luma_minus8 + 8);

    // 4) Chroma subsampling from chroma_format_idc: 0=mono,1=4:2:0,2=4:2:2,3=4:4:4
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
        stsd: None, // Annex B streams never include an MP4 stsd box
    }
}
