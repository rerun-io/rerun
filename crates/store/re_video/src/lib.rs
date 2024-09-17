//! Video decoding library.
//!
//! The entry point is [`VideoData::load_from_bytes`]
//! which produces an instance of [`VideoData`] from any supported video container.

mod mp4;

use itertools::Itertools;
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

impl VideoData {
    /// Loads a video from the given data.
    ///
    /// TODO(andreas, jan): This should not copy the data, but instead store slices into a shared buffer.
    /// at the very least the should be a way to extract only metadata.
    pub fn load_from_bytes(data: &[u8], media_type: Option<&str>) -> Result<Self, VideoLoadError> {
        // Media type guessing here should be identical to `re_types::MediaType::guess_from_data`,
        // but we don't want to depend on `re_types` here.
        let media_type = if let Some(media_type) = media_type {
            media_type.to_owned()
        } else if mp4::is_mp4(data) {
            "video/mp4".to_owned()
        } else {
            // Technically this means that we failed to determine the media type altogether,
            // but we don't want to call it `FailedToDetermineMediaType` since the rest of Rerun has
            // access to `re_types::components::MediaType` which has a much wider range of media type detection.
            return Err(VideoLoadError::UnsupportedVideoType);
        };

        match media_type.as_str() {
            "video/mp4" => mp4::load_mp4(data),
            media_type => Err(VideoLoadError::UnsupportedMediaType(media_type.to_owned())),
        }
    }

    /// Determines the presentation timestamps of all frames inside a video, returning raw time values.
    ///
    /// Returned timestamps are in nanoseconds since start and are guaranteed to be monotonically increasing.
    pub fn frame_timestamps_ns(&self) -> impl Iterator<Item = i64> + '_ {
        // Segments are guaranteed to be sorted among each other, but within a segment,
        // presentation timestamps may not be sorted since this is sorted by decode timestamps.
        self.segments.iter().flat_map(|seg| {
            seg.samples
                .iter()
                .map(|sample| sample.timestamp.as_nanoseconds())
                .sorted()
        })
    }
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

    #[inline]
    pub fn new(ms: f64) -> Self {
        Self(OrderedFloat(ms))
    }

    #[inline]
    pub fn as_f64(&self) -> f64 {
        self.0.into_inner()
    }

    #[inline]
    pub fn as_nanoseconds(self) -> i64 {
        (self.0 * 1_000_000.0).round() as i64
    }
}

impl std::ops::Add<Self> for TimeMs {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub<Self> for TimeMs {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

/// Errors that can occur when loading a video.
#[derive(thiserror::Error, Debug)]
pub enum VideoLoadError {
    #[error("Failed to determine media type from data: {0}")]
    ParseMp4(#[from] ::mp4::Error),

    #[error("Video file has no video tracks")]
    NoVideoTrack,

    #[error("Video file track config is invalid")]
    InvalidConfigFormat,

    #[error("Video file has invalid sample entries")]
    InvalidSamples,

    #[error("Video file has unsupported media type {0}")]
    UnsupportedMediaType(String),

    #[error("Video file has unsupported format")]
    UnsupportedVideoType,

    #[error("Video file has unsupported codec {0}")]
    UnsupportedCodec(String),
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
