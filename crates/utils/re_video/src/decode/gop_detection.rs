use h264_reader::{
    annexb::AnnexBReader,
    nal::{self, Nal as _},
    push::NalInterest,
};

use crate::{ChromaSubsamplingModes, VideoCodec, VideoEncodingDetails};

/// Failure reason for [`is_sample_start_of_gop`].
#[derive(thiserror::Error, Debug)]
pub enum DetectGopStartError {
    #[error("Detection not supported for codec: {0:?}")]
    UnsupportedCodec(VideoCodec),

    #[error("NAL header error: {0:?}")]
    NalHeaderError(h264_reader::nal::NalHeaderError),

    #[error("Detected group of picture but failed to extract encoding details: {0:?}")]
    FailedToExtractEncodingDetails(String),
}

impl PartialEq<Self> for DetectGopStartError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::UnsupportedCodec(a), Self::UnsupportedCodec(b)) => a == b,
            (Self::NalHeaderError(_), Self::NalHeaderError(_)) => true, // `NalHeaderError` isn't implementing PartialEq, but there's only one variant.
            (Self::FailedToExtractEncodingDetails(a), Self::FailedToExtractEncodingDetails(b)) => {
                a == b
            }
            _ => false,
        }
    }
}

impl Eq for DetectGopStartError {}

/// Result of a successful GOP detection.
///
/// I.e. whether a sample is the start of a GOP and if so, encoding details we were able to extract from it.
#[derive(Default, PartialEq, Eq, Debug)]
pub enum GopStartDetection {
    /// The sample is the start of a GOP and encoding details have been extracted.
    StartOfGop(VideoEncodingDetails),

    /// The sample is not the start of a GOP.
    #[default]
    NotStartOfGop,
}

impl GopStartDetection {
    #[inline]
    pub fn is_start_of_gop(&self) -> bool {
        matches!(self, Self::StartOfGop(_))
    }
}

/// Try to determine whether a frame chunk is the start of a GOP.
///
/// This is a best effort attempt to determine this, but we won't always be able to.
#[inline]
pub fn detect_gop_start(
    sample_data: &[u8],
    codec: VideoCodec,
) -> Result<GopStartDetection, DetectGopStartError> {
    #[expect(clippy::match_same_arms)]
    match codec {
        VideoCodec::H264 => detect_h264_annexb_gop(sample_data),
        VideoCodec::H265 => Err(DetectGopStartError::UnsupportedCodec(codec)),
        VideoCodec::AV1 => Err(DetectGopStartError::UnsupportedCodec(codec)),
        VideoCodec::VP8 => Err(DetectGopStartError::UnsupportedCodec(codec)),
        VideoCodec::VP9 => Err(DetectGopStartError::UnsupportedCodec(codec)),
    }
}

#[derive(Default)]
struct H264GopDetectionState {
    coding_details_from_sps: Option<Result<VideoEncodingDetails, String>>,
    idr_frame_found: bool,
}

impl h264_reader::push::AccumulatedNalHandler for H264GopDetectionState {
    fn nal(&mut self, nal: nal::RefNal<'_>) -> NalInterest {
        let Ok(nal_header) = nal.header() else {
            return NalInterest::Ignore;
        };
        let nal_unit_type = nal_header.nal_unit_type();

        if nal_unit_type == nal::UnitType::SeqParameterSet {
            if !nal.is_complete() {
                // Want full SPS, not just a partial one in order to extract the encoding details.
                return NalInterest::Buffer;
            }

            // Note that if we find several SPS, we'll always use the latest one.
            self.coding_details_from_sps = Some(
                match nal::sps::SeqParameterSet::from_bits(nal.rbsp_bits())
                    .and_then(|sps| encoding_details_from_h264_sps(&sps))
                {
                    Ok(coding_details) => {
                        // A bit too much string concatenation something that frequent, better to enable this only for debug builds.
                        if cfg!(debug_assertions) {
                            re_log::trace!(
                                "Parsed SPS to coding details for video stream: {coding_details:?}"
                            );
                        }
                        Ok(coding_details)
                    }
                    Err(sps_error) => Err(format!("Failed reading SPS: {sps_error:?}")), // h264 errors don't implement display
                },
            );
        } else if nal_unit_type == nal::UnitType::SliceLayerWithoutPartitioningIdr {
            self.idr_frame_found = true;
        }

        NalInterest::Ignore
    }
}

fn encoding_details_from_h264_sps(
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

/// Try to determine whether a frame chunk is the start of a closed GOP in an h264 Annex B encoded stream.
fn detect_h264_annexb_gop(
    mut sample_data: &[u8],
) -> Result<GopStartDetection, DetectGopStartError> {
    let mut reader = AnnexBReader::accumulate(H264GopDetectionState::default());

    while !sample_data.is_empty() {
        // Don't parse everything at once.
        const MAX_CHUNK_SIZE: usize = 256;
        let chunk_size = MAX_CHUNK_SIZE.min(sample_data.len());

        reader.push(&sample_data[..chunk_size]);

        // In case of SPS parsing failure keep going.
        // It's unlikely, but maybe there's another SPS in the chunk that succeeds parsing.
        let handler = reader.nal_handler_ref();
        if let (true, Some(Ok(_))) = (handler.idr_frame_found, &handler.coding_details_from_sps) {
            break;
        }

        sample_data = &sample_data[chunk_size..];
    }

    let handler = reader.into_nal_handler();
    match handler.coding_details_from_sps {
        Some(Ok(decoding_details)) => {
            if handler.idr_frame_found {
                Ok(GopStartDetection::StartOfGop(decoding_details))
            } else {
                // In theory it could happen that we got an SPS but no IDR frame.
                // Arguably we should preserve the information from the the SPS, but practically it's not useful:
                // If we never hit an IDR frame, then we can't play the video and every IDR frame is supposed to have
                // the *same* SPS.
                Ok(GopStartDetection::NotStartOfGop)
            }
        }
        Some(Err(error)) => Err(DetectGopStartError::FailedToExtractEncodingDetails(error)),
        None => Ok(GopStartDetection::NotStartOfGop),
    }
}

#[cfg(test)]
mod test {
    use super::{GopStartDetection, detect_h264_annexb_gop};
    use crate::{ChromaSubsamplingModes, DetectGopStartError, VideoEncodingDetails};

    #[test]
    fn test_detect_h264_annexb_gop() {
        // Example H.264 Annex B encoded data containing SPS and IDR frame. (ai generated)
        let sample_data = &[
            // SPS NAL unit
            0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84,
            0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11,
            0x80, // IDR frame NAL unit
            0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54, 0x2B,
            0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92,
        ];
        let result = detect_h264_annexb_gop(sample_data);
        assert_eq!(
            result,
            Ok(GopStartDetection::StartOfGop(VideoEncodingDetails {
                codec_string: "avc1.64000A".to_owned(),
                coded_dimensions: [64, 64],
                bit_depth: Some(8),
                chroma_subsampling: Some(ChromaSubsamplingModes::Yuv420),
                stsd: None,
            }))
        );

        // Example H.264 Annex B encoded data containing broken SPS and IDR frame. (above example but messed with the SPS)
        let sample_data = &[
            // SPS NAL unit
            0x00, 0x00, 0x00, 0x01, 0x67, 0x00, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84,
            0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11,
            0x80, // IDR frame NAL unit
            0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54, 0x2B,
            0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92,
        ];
        let result = detect_h264_annexb_gop(sample_data);
        assert_eq!(
            result,
            Err(DetectGopStartError::FailedToExtractEncodingDetails(
                "Failed reading SPS: RbspReaderError(RemainingData)".to_owned()
            ))
        );

        // Garbage data, still annex b shaped. (ai generated)
        let sample_data = &[
            0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84,
            0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11,
            0x80,
        ];
        let result = detect_h264_annexb_gop(sample_data);
        assert_eq!(result, Ok(GopStartDetection::NotStartOfGop));

        // Garbage data, no detectable nalu units.
        let sample_data = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A];
        let result = detect_h264_annexb_gop(sample_data);
        assert_eq!(result, Ok(GopStartDetection::NotStartOfGop));
    }
}
