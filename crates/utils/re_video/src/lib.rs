//! Video decoding library.

mod decode;
mod demux;
mod h264;
mod stable_index_deque;
mod time;

pub use decode::{
    AsyncDecoder, Chunk, DecodeError, DecodeHardwareAcceleration, DecodeSettings,
    DetectGopStartError, Frame, FrameContent, FrameInfo, GopStartDetection, PixelFormat,
    Result as DecodeResult, YuvMatrixCoefficients, YuvPixelLayout, YuvRange, detect_gop_start,
    new_decoder,
};

#[cfg(with_ffmpeg)]
pub use decode::{FFmpegError, FFmpegVersion, FFmpegVersionParseError, ffmpeg_download_url};

pub use demux::{
    ChromaSubsamplingModes, GopIndex, GroupOfPictures, SampleIndex, SampleMetadata,
    SamplesStatistics, VideoCodec, VideoDataDescription, VideoEncodingDetails, VideoLoadError,
};
pub use stable_index_deque::StableIndexDeque;
pub use time::{Time, Timescale};

pub use re_mp4::{TrackId, TrackKind};

/// Returns information about this crate
pub fn build_info() -> re_build_info::BuildInfo {
    re_build_info::build_info!()
}
