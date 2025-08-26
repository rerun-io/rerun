//! Video decoding library.

mod decode;
mod demux;
mod gop_detection;
mod h264;
mod h265;
mod nalu;
mod stable_index_deque;
mod time;

pub use decode::{
    AsyncDecoder, Chunk, DecodeError, DecodeHardwareAcceleration, DecodeSettings, Frame,
    FrameContent, FrameInfo, FrameResult, PixelFormat, Result as DecodeResult,
    YuvMatrixCoefficients, YuvPixelLayout, YuvRange, new_decoder,
};
pub use gop_detection::{DetectGopStartError, GopStartDetection, detect_gop_start};

#[cfg(with_ffmpeg)]
pub use self::decode::{FFmpegError, FFmpegVersion, FFmpegVersionParseError, ffmpeg_download_url};

pub use demux::{
    ChromaSubsamplingModes, GopIndex, GroupOfPictures, SampleIndex, SampleMetadata,
    SamplesStatistics, VideoCodec, VideoDataDescription, VideoDeliveryMethod, VideoEncodingDetails,
    VideoLoadError,
};

// AnnexB conversions are useful for testing.
pub use h264::write_avc_chunk_to_nalu_stream;
pub use h265::write_hevc_chunk_to_nalu_stream;
pub use nalu::AnnexBStreamState;

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
