//! Video demultiplexing.
//!
//! Parses a video file into a raw [`VideoData`] struct, which contains basic metadata and a list of [`Segment`]s.

pub mod mp4;

use crate::TimeMs;

/// Decoded video data.
#[derive(Clone)]
pub struct VideoData {
    pub config: Config,

    /// Duration of the video, in milliseconds.
    pub duration: TimeMs,

    /// We split video into segments, each beginning with a key frame,
    /// followed by any number of delta frames.
    pub segments: Vec<Segment>,

    /// List of samples contained in this video.
    pub samples: Vec<Sample>,

    /// This array stores all data used by samples.
    pub data: Vec<u8>,
}

impl VideoData {
    pub fn get(&self, sample: &Sample) -> &[u8] {
        &self.data
            [sample.byte_offset as usize..sample.byte_offset as usize + sample.byte_length as usize]
    }
}

/// A segment of a video.
#[derive(Clone)]
pub struct Segment {
    /// Time of the first sample in this segment, in milliseconds.
    pub timestamp: TimeMs,

    pub start: usize,
    pub len: usize,
}

/// A single sample in a video.
#[derive(Debug, Clone)]
pub struct Sample {
    /// Time at which this sample appears, in milliseconds.
    pub timestamp: TimeMs,

    /// Duration of the sample, in milliseconds.
    pub duration: TimeMs,

    /// Offset into [`VideoData::data`]
    pub byte_offset: u32,

    /// Length of sample starting at [`Sample::byte_offset`].
    pub byte_length: u32,
}

/// Configuration of a video.
#[derive(Debug, Clone)]
pub struct Config {
    /// `FourCC` code identifying the codec.
    pub codec: Codec,

    /// String used to identify the codec and some of its configuration.
    pub codec_string: String,

    /// Codec-specific configuration.
    pub description: Vec<u8>,

    /// Natural height of the video.
    pub coded_height: u16,

    /// Natural width of the video.
    pub coded_width: u16,
}

#[derive(Debug, Clone, Copy)]
pub enum Codec {
    /// AV1
    Av01,

    /// H.264
    Avc1,

    /// H.265
    Hevc,

    /// VP8
    Vp08,

    /// VP9
    Vp09,
}

/// Errors that can occur when loading a video.
#[derive(Debug)]
pub enum VideoLoadError {
    ParseMp4(::mp4::Error),
    NoVideoTrack,
    InvalidConfigFormat,
    InvalidSamples,
    UnsupportedMediaType(String),
    UnknownMediaType,
    UnsupportedCodec(String),
}

impl std::fmt::Display for VideoLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseMp4(err) => write!(f, "failed to parse video: {err}"),
            Self::NoVideoTrack => write!(f, "video file has no video tracks"),
            Self::InvalidConfigFormat => write!(f, "video file track config is invalid"),
            Self::InvalidSamples => write!(f, "video file has invalid sample entries"),
            Self::UnsupportedMediaType(type_) => {
                write!(f, "unsupported media type {type_:?}")
            }
            Self::UnknownMediaType => write!(f, "unknown media type"),
            Self::UnsupportedCodec(codec) => write!(f, "unsupported codec {codec:?}"),
        }
    }
}

impl std::error::Error for VideoLoadError {}

impl From<::mp4::Error> for VideoLoadError {
    fn from(value: ::mp4::Error) -> Self {
        Self::ParseMp4(value)
    }
}

impl std::fmt::Debug for VideoData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Video")
            .field("config", &self.config)
            .field("duration", &self.duration)
            .field("segments", &self.segments)
            .field("data", &self.data.len())
            .finish()
    }
}

impl std::fmt::Debug for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Segment")
            .field("timestamp", &self.timestamp)
            .field("samples", &self.len)
            .finish()
    }
}
