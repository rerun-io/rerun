use crate::av1::detect_av1_keyframe_start;
use crate::h264::detect_h264_annexb_gop;
use crate::h265::detect_h265_annexb_gop;
use crate::{VideoCodec, VideoEncodingDetails};

/// Failure reason for [`detect_gop_start`].
#[derive(thiserror::Error, Debug)]
pub enum DetectGopStartError {
    #[error("Detection not supported for codec: {0:?}")]
    UnsupportedCodec(VideoCodec),

    #[error("NAL header error: {0:?}")]
    NalHeaderError(h264_reader::nal::NalHeaderError),

    #[error("AV1 parser error: {0}")]
    Av1ParserError(std::io::Error),

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
        VideoCodec::H265 => detect_h265_annexb_gop(sample_data),
        VideoCodec::AV1 => detect_av1_keyframe_start(sample_data),
        VideoCodec::VP8 => Err(DetectGopStartError::UnsupportedCodec(codec)),
        VideoCodec::VP9 => Err(DetectGopStartError::UnsupportedCodec(codec)),
    }
}
