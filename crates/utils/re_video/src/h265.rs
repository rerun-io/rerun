//! General H.265 utilities.
//!
use crate::{
    ChromaSubsamplingModes, Chunk, DetectGopStartError, GopStartDetection, VideoEncodingDetails,
    nalu::{
        ANNEXB_NAL_START_CODE, AnnexBStreamState, AnnexBStreamWriteError,
        write_length_prefixed_nalus_to_annexb_stream,
    },
};
use cros_codecs::codec::h265::parser::{Nalu, NaluType, Parser, Sps};

/// Retrieve [`VideoEncodingDetails`] from an H.265 SPS.
pub fn encoding_details_from_h265_sps(sps: &Sps) -> VideoEncodingDetails {
    let profile_idc = sps.profile_tier_level.general_profile_idc;
    let level_idc: u8 = sps.profile_tier_level.general_level_idc as u8;

    // WebCodecs HEVC strings are usually "hvc1.<profile>.<level>" (with optional flags)
    let codec_string = format!("hvc1.{profile_idc:02X}.L{level_idc:02}");

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

pub fn detect_h265_annexb_gop(data: &[u8]) -> Result<GopStartDetection, DetectGopStartError> {
    let mut parser = Parser::default();
    let mut details: Option<VideoEncodingDetails> = None;
    let mut idr_found = false;
    let mut cursor = std::io::Cursor::new(data);

    while let Ok(nalu) = Nalu::next(&mut cursor) {
        match nalu.header.type_ {
            NaluType::SpsNut if details.is_none() => {
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

pub fn write_hevc_chunk_to_nalu_stream(
    hvcc: &re_mp4::HevcBox,
    nalu_stream: &mut dyn std::io::Write,
    chunk: &Chunk,
    state: &mut AnnexBStreamState,
) -> Result<(), AnnexBStreamWriteError> {
    if chunk.is_sync && !state.previous_frame_was_idr {
        let mut hvcc_ps: Vec<Vec<u8>> = Vec::new();

        for arr in &hvcc.hvcc.arrays {
            if let Ok(nalu_type) = NaluType::try_from(arr.nal_unit_type as u32) {
                if matches!(
                    nalu_type,
                    NaluType::VpsNut | NaluType::SpsNut | NaluType::PpsNut
                ) {
                    for nalu in &arr.nalus {
                        hvcc_ps.push(nalu.data.clone());
                    }
                }
            }
        }

        for ps in &hvcc_ps {
            nalu_stream
                .write_all(ANNEXB_NAL_START_CODE)
                .map_err(AnnexBStreamWriteError::FailedToWriteToStream)?;
            nalu_stream
                .write_all(ps)
                .map_err(AnnexBStreamWriteError::FailedToWriteToStream)?;
        }
        state.previous_frame_was_idr = true;
    } else {
        state.previous_frame_was_idr = false;
    }

    // Each NAL unit in mp4 is prefixed with a length prefix.
    // In Annex B this doesn't exist.
    let length_prefix_size = (hvcc.hvcc.contents.length_size_minus_one as usize & 0x03) + 1;

    write_length_prefixed_nalus_to_annexb_stream(nalu_stream, &chunk.data, length_prefix_size)
}
