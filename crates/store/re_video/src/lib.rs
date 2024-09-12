//! Video decoding library.
//!
//! The entry point is [`load_mp4`], which produces an instance of [`VideoData`].

mod mp4;
pub use mp4::load_mp4;
use vec1::Vec1;

// TODO: make this more friendly for the searching operations in video decoder
// more flat arrays

/// Decoded video data.
#[derive(Clone)]
pub struct VideoData {
    pub config: Config,

    /// How many time units are there per second.
    pub timescale: Timescale,

    /// Duration of the video, in time units.
    pub duration: Time,

    /// We split video into segments, each beginning with a key frame,
    /// followed by any number of delta frames.
    pub segments: Vec<Segment>,

    pub samples: Vec<Sample>,

    /// This array stores all data used by samples.
    pub data: Vec<u8>,
}

/// A segment of a video.
#[derive(Clone)]
pub struct Segment {
    pub start: Time,

    pub sample_range: (u32, u32),
}

impl Segment {
    pub fn decode_start_nanos(&self, timescale: Timescale) -> i64 {
        self.samples.first().decode_timestamp.into_nanos(timescale)
    }
}

/// A single sample in a video.
#[derive(Debug, Clone)]
pub struct Sample {
    /// Time at which this sample appears in the decoded bitstream, in time units.
    pub decode_timestamp: Time,

    /// Time at which this sample appears in the frame stream, in time units.
    pub composition_timestamp: Time,

    /// Duration of the sample, in time units.
    pub duration: Time,

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

/// A value in time units.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(u64);

impl Time {
    pub const ZERO: Self = Self(0);

    /// Create a new value in _time units_.
    ///
    /// ⚠️ Don't use this for regular timestamps in seconds/milliseconds/etc.,
    /// use the proper constructors for those instead!
    /// This only exists for cases where you already have a value expressed in time units,
    /// such as those received from the `WebCodecs` APIs.
    #[inline]
    pub fn new(v: u64) -> Self {
        Self(v)
    }

    #[inline]
    pub fn from_secs(v: f64, timescale: Timescale) -> Self {
        Self((v * timescale.0 as f64).round() as u64)
    }

    #[inline]
    pub fn from_millis(v: f64, timescale: Timescale) -> Self {
        Self::from_secs(v / 1e3, timescale)
    }

    #[inline]
    pub fn from_micros(v: f64, timescale: Timescale) -> Self {
        Self::from_secs(v / 1e6, timescale)
    }

    #[inline]
    pub fn from_nanos(v: i64, timescale: Timescale) -> Self {
        Self::from_secs(v as f64 / 1e9, timescale)
    }

    #[inline]
    pub fn into_secs(self, timescale: Timescale) -> f64 {
        self.0 as f64 / timescale.0 as f64
    }

    #[inline]
    pub fn into_millis(self, timescale: Timescale) -> f64 {
        self.into_secs(timescale) * 1e3
    }

    #[inline]
    pub fn into_micros(self, timescale: Timescale) -> f64 {
        self.into_secs(timescale) * 1e6
    }

    #[inline]
    pub fn into_nanos(self, timescale: Timescale) -> i64 {
        (self.into_secs(timescale) * 1e9).round() as i64
    }
}

/// The number of time units per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timescale(u64);

impl Timescale {
    pub(crate) fn new(v: u64) -> Self {
        Self(v)
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
            .field("timescale", &self.timescale)
            .field("duration", &self.duration)
            .field("segments", &self.segments)
            .finish()
    }
}

impl std::fmt::Debug for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Segment")
            .field("samples", &self.samples.len())
            .finish()
    }
}
