//! Video decoding library.

mod av1;
mod decode;
mod demux;
mod gop_detection;
mod h264;
mod h265;
mod nalu;
mod stable_index_deque;
mod time;

pub use av1::{AV1_TEST_INTER_FRAME, AV1_TEST_KEYFRAME};
pub use decode::{
    AsyncDecoder, Chunk, DecodeError, DecodeHardwareAcceleration, DecodeSettings, Frame,
    FrameContent, FrameInfo, FrameResult, PixelFormat, Result as DecodeResult,
    YuvMatrixCoefficients, YuvPixelLayout, YuvRange, new_decoder,
};
pub use demux::{
    ChromaSubsamplingModes, KeyframeIndex, SampleIndex, SampleMetadata, SampleMetadataState,
    SamplesStatistics, VideoCodec, VideoDataDescription, VideoDeliveryMethod, VideoEncodingDetails,
    VideoLoadError,
};
pub use gop_detection::{DetectGopStartError, GopStartDetection, detect_gop_start};
// AnnexB conversions are useful for testing.
pub use h264::{write_avc_chunk_to_annexb, write_avc_chunk_to_nalu_stream};
pub use h265::{write_hevc_chunk_to_annexb, write_hevc_chunk_to_nalu_stream};
pub use nalu::AnnexBStreamState;
// Re-export:
#[doc(no_inline)]
pub use {
    re_mp4::{TrackId, TrackKind},
    re_quota_channel::{Receiver, Sender, TryRecvError},
    re_span::Span,
    stable_index_deque::StableIndexDeque,
    time::{Time, Timescale},
};

#[cfg(with_ffmpeg)]
pub use self::decode::{FFmpegError, FFmpegVersion, FFmpegVersionParseError, ffmpeg_download_url};

// TODO(isse): When video player is moved to `re_video`, move this into the
// player module.
/// Severity of a video player error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoPlaybackIssueSeverity {
    /// The video can't be played back due to a proper error.
    ///
    /// E.g. invalid data provided, decoder problems, not supported etc.
    Error,

    /// The video can't be played back right now, but it may not actually be an error.
    Informational,

    /// We're still missing required data before we're able to play this video.
    Loading,
}

impl VideoPlaybackIssueSeverity {
    pub fn loading_to_informational(self) -> Self {
        match self {
            Self::Loading => Self::Informational,
            other => other,
        }
    }
}

pub fn enabled_features() -> &'static [&'static str] {
    &[
        #[cfg(feature = "av1")]
        "av1",
        #[cfg(feature = "ffmpeg")]
        "ffmpeg",
        #[cfg(feature = "nasm")]
        "nasm",
    ]
}

/// Create a channel for use with [`new_decoder`].
pub fn channel<T>(
    debug_name: impl Into<String>,
) -> (re_quota_channel::Sender<T>, re_quota_channel::Receiver<T>) {
    let max_bytes_in_flight = 512 * 1024 * 1024; // TODO(emilk): make configurable
    re_quota_channel::channel(debug_name, max_bytes_in_flight)
}
