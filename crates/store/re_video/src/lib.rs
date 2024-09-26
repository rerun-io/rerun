//! Video decoding library.
//!
//! The entry point is [`VideoData::load_from_bytes`]
//! which produces an instance of [`VideoData`] from any supported video container.

mod mp4;

use std::{collections::BTreeMap, ops::Range};

use itertools::Itertools;

pub use re_mp4::{TrackId, TrackKind};

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

    /// Samples contain the byte offsets into `data` for each frame.
    ///
    /// This list is sorted in ascending order of decode timestamps.
    ///
    /// Samples must be decoded in decode-timestamp order,
    /// and should be presented in composition-timestamp order.
    pub samples: Vec<Sample>,

    /// This array stores all data used by samples.
    pub data: Vec<u8>,

    /// All the tracks in the mp4; not just the video track.
    ///
    /// Can be nice to show in a UI.
    pub mp4_tracks: BTreeMap<TrackId, Option<TrackKind>>,
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

    /// Duration of the video, in seconds.
    #[inline]
    pub fn duration_sec(&self) -> f64 {
        self.duration.into_secs(self.timescale)
    }

    /// Duration of the video, in milliseconds.
    #[inline]
    pub fn duration_ms(&self) -> f64 {
        self.duration.into_millis(self.timescale)
    }

    /// Natural width of the video.
    #[inline]
    pub fn width(&self) -> u32 {
        self.config.coded_width as u32
    }

    /// Natural height of the video.
    #[inline]
    pub fn height(&self) -> u32 {
        self.config.coded_height as u32
    }

    /// The codec used to encode the video.
    #[inline]
    pub fn codec(&self) -> &str {
        &self.config.codec
    }

    /// The number of samples in the video.
    #[inline]
    pub fn num_samples(&self) -> usize {
        self.samples.len()
    }

    /// Determines the presentation timestamps of all frames inside a video, returning raw time values.
    ///
    /// Returned timestamps are in nanoseconds since start and are guaranteed to be monotonically increasing.
    pub fn frame_timestamps_ns(&self) -> impl Iterator<Item = i64> + '_ {
        // Segments are guaranteed to be sorted among each other, but within a segment,
        // presentation timestamps may not be sorted since this is sorted by decode timestamps.
        self.segments.iter().flat_map(|seg| {
            self.samples[seg.range()]
                .iter()
                .map(|sample| sample.composition_timestamp.into_nanos(self.timescale))
                .sorted()
        })
    }
}

/// A segment of a video.
#[derive(Debug, Clone)]
pub struct Segment {
    /// Decode timestamp of the first sample in this segment, in time units.
    pub start: Time,

    /// Range of samples contained in this segment.
    pub sample_range: Range<u32>,
}

impl Segment {
    /// The segment's `sample_range` mapped to `usize` for slicing.
    pub fn range(&self) -> Range<usize> {
        Range {
            start: self.sample_range.start as usize,
            end: self.sample_range.end as usize,
        }
    }
}

/// A single sample in a video.
#[derive(Debug, Clone)]
pub struct Sample {
    /// Time at which this sample appears in the decoded bitstream, in time units.
    pub decode_timestamp: Time,

    /// Time at which this sample appears in the frame stream, in time units.
    ///
    /// `composition >= decode`
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
#[derive(thiserror::Error, Debug)]
pub enum VideoLoadError {
    #[error("Failed to determine media type from data: {0}")]
    ParseMp4(#[from] re_mp4::Error),

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

    // `FourCC`'s debug impl doesn't quote the result
    #[error("Video track uses unsupported codec \"{0}\"")] // NOLINT
    UnsupportedCodec(re_mp4::FourCC),
}

impl std::fmt::Debug for VideoData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Video")
            .field("config", &self.config)
            .field("timescale", &self.timescale)
            .field("duration", &self.duration)
            .field("segments", &self.segments)
            .field(
                "samples",
                &self.samples.iter().enumerate().collect::<Vec<_>>(),
            )
            .field("data", &self.data.len())
            .finish()
    }
}
