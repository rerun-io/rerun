//! Video decoding library.
//!
//! The entry point is [`load_mp4`], which produces an instance of [`VideoData`].

mod mp4;
pub use mp4::load_mp4;
use ordered_float::OrderedFloat;

/// Decoded video data.
#[derive(Clone)]
pub struct VideoData {
    pub config: Config,

    /// Duration of the video, in milliseconds.
    pub duration: TimeMs,

    /// We split video into segments, each beginning with a key frame,
    /// followed by any number of delta frames.
    pub segments: Vec<Segment>,

    /// This array stores all data used by samples.
    pub data: Vec<u8>,
}

/// A segment of a video.
#[derive(Clone)]
pub struct Segment {
    /// Time of the first sample in this segment, in milliseconds.
    pub timestamp: TimeMs,

    /// List of samples contained in this segment.
    /// At least one sample per segment is guaranteed,
    /// and the first sample is always a key frame.
    pub samples: Vec<Sample>,
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
    /// String used to identify the codec and some of its configuration.
    pub codec: String,

    /// Codec-specific configuration.
    pub description: Vec<u8>,

    /// Natural height of the video.
    pub coded_height: u16,

    /// Natural width of the video.
    pub coded_width: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeMs(OrderedFloat<f64>);

impl TimeMs {
    pub const ZERO: Self = Self(OrderedFloat(0.0));

    pub fn new(ms: f64) -> Self {
        Self(OrderedFloat(ms))
    }

    pub fn as_f64(&self) -> f64 {
        self.0.into_inner()
    }
}

impl std::ops::Add<Self> for TimeMs {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub<Self> for TimeMs {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
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
    UnsupportedCodec,
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
            Self::UnsupportedCodec => write!(f, "unsupported codec"),
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
            .field("samples", &self.samples.len())
            .finish()
    }
}
