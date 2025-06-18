//! General H.264 utilities.

use h264_reader::nal;

use crate::{ChromaSubsamplingModes, VideoEncodingDetails};

/// Retrieve [`VideoEncodingDetails`] from a H.264 SPS.
pub fn encoding_details_from_h264_sps(
    sps: &nal::sps::SeqParameterSet,
) -> Result<VideoEncodingDetails, nal::sps::SpsError> {
    let bit_depth = sps.chroma_info.bit_depth_chroma_minus8 + 8;

    // Codec string defined by WebCodec points to various spec documents.
    // https://www.w3.org/TR/webcodecs-avc-codec-registration/#fully-qualified-codec-strings
    // Not having read those, this is what we use in `re_mp4` and it works fine.
    // Also as of writing, Claude 4 agrees and is able to nicely explain its meaning.
    let profile = u8::from(sps.profile_idc);
    let constraint = u8::from(sps.constraint_flags);
    let level = sps.level_idc;
    let codec_string = format!("avc1.{profile:02X}{constraint:02X}{level:02X}");

    // Calculating the dimensions of the frame in pixels from the SPS is quite complicated
    // as it has to take into account cropping and concepts like macro block sizes.
    // Luckily h264-reader has a utility for this!
    let coded_dimensions = sps.pixel_dimensions()?;
    let chroma_subsampling = match sps.chroma_info.chroma_format {
        nal::sps::ChromaFormat::Monochrome => Some(ChromaSubsamplingModes::Monochrome),
        nal::sps::ChromaFormat::YUV420 => Some(ChromaSubsamplingModes::Yuv420),
        nal::sps::ChromaFormat::YUV422 => Some(ChromaSubsamplingModes::Yuv422),
        nal::sps::ChromaFormat::YUV444 => Some(ChromaSubsamplingModes::Yuv444),
        nal::sps::ChromaFormat::Invalid(_) => {
            re_log::error_once!(
                "Invalid chroma format in H264 SPS: {:?}",
                sps.chroma_info.chroma_format
            );
            None
        }
    };

    Ok(VideoEncodingDetails {
        codec_string,
        coded_dimensions: [coded_dimensions.0 as _, coded_dimensions.1 as _],
        bit_depth: Some(bit_depth),
        chroma_subsampling,
        stsd: None,
    })
}
