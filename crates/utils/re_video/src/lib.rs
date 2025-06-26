//! Video decoding library.

mod decode;
mod demux;
mod h264;
mod stable_index_deque;
mod time;

pub use self::{
    decode::{
        AsyncDecoder, Chunk, DecodeError, DecodeHardwareAcceleration, DecodeSettings,
        DetectGopStartError, Frame, FrameContent, FrameInfo, GopStartDetection, PixelFormat,
        Result as DecodeResult, YuvMatrixCoefficients, YuvPixelLayout, YuvRange, detect_gop_start,
        new_decoder,
    },
    demux::{
        ChromaSubsamplingModes, GopIndex, GroupOfPictures, SampleIndex, SampleMetadata,
        SamplesStatistics, VideoCodec, VideoDataDescription, VideoEncodingDetails, VideoLoadError,
    },
};

#[cfg(with_ffmpeg)]
pub use self::decode::{FFmpegError, FFmpegVersion, FFmpegVersionParseError, ffmpeg_download_url};

// Re-export:
#[doc(no_inline)]
pub use {
    re_mp4::{TrackId, TrackKind},
    re_span::Span,
    stable_index_deque::StableIndexDeque,
    time::{Time, Timescale},
};

/// Returns information about this crate
pub fn build_info() -> re_build_info::BuildInfo {
    re_build_info::build_info!()
}
