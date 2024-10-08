//! Video demultiplexing.
//!
//! Parses a video file into a raw [`VideoData`] struct, which contains basic metadata and a list of [`GroupOfPicture`]s.
//!
//! The entry point is [`VideoData::load_from_bytes`]
//! which produces an instance of [`VideoData`] from any supported video container.

pub mod mp4;

use std::{collections::BTreeMap, ops::Range};

use itertools::Itertools as _;

use super::{Time, Timescale};

use crate::{Chunk, TrackId, TrackKind};

/// Decoded video data.
#[derive(Clone)]
pub struct VideoData {
    pub config: Config,

    /// How many time units are there per second.
    pub timescale: Timescale,

    /// Duration of the video, in time units.
    pub duration: Time,

    /// We split video into GOPs, each beginning with a key frame,
    /// followed by any number of delta frames.
    pub gops: Vec<GroupOfPictures>,

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
    pub fn load_from_bytes(data: &[u8], media_type: &str) -> Result<Self, VideoLoadError> {
        match media_type {
            "video/mp4" => Self::load_mp4(data),

            media_type => {
                if media_type.starts_with("video/") {
                    Err(VideoLoadError::UnsupportedMimeType {
                        provided_or_detected_media_type: media_type.to_owned(),
                    })
                } else {
                    Err(VideoLoadError::MimeTypeIsNotAVideo {
                        provided_or_detected_media_type: media_type.to_owned(),
                    })
                }
            }
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
        self.gops.iter().flat_map(|seg| {
            self.samples[seg.range()]
                .iter()
                .map(|sample| sample.composition_timestamp.into_nanos(self.timescale))
                .sorted()
        })
    }

    /// Returns `None` if the sample is invalid/out-of-range.
    pub fn get(&self, sample: &Sample) -> Option<Chunk> {
        let byte_offset = sample.byte_offset as usize;
        let byte_length = sample.byte_length as usize;

        if self.data.len() < byte_offset + byte_length {
            None
        } else {
            let data = &self.data[byte_offset..byte_offset + byte_length];

            Some(Chunk {
                data: data.to_vec(),
                timestamp: sample.decode_timestamp,
                duration: sample.duration,
            })
        }
    }
}

/// A Group of Pictures (GOP) always starts with an I-frame, followed by delta-frames.
///
/// See <https://en.wikipedia.org/wiki/Group_of_pictures> for more.
#[derive(Debug, Clone)]
pub struct GroupOfPictures {
    /// Decode timestamp of the first sample in this GOP, in time units.
    pub start: Time,

    /// Range of samples contained in this GOP.
    pub sample_range: Range<u32>,
}

impl GroupOfPictures {
    /// The GOP's `sample_range` mapped to `usize` for slicing.
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
    ///
    /// Samples should be decoded in this order.
    ///
    /// `decode_timestamp <= composition_timestamp`
    pub decode_timestamp: Time,

    /// Time at which this sample appears in the frame stream, in time units.
    ///
    /// The frame should be shown at this time.
    ///
    /// `decode_timestamp <= composition_timestamp`
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
    ///
    /// e.g. "av01.0.05M.08" (AV1)
    pub codec: String,

    /// Codec-specific configuration.
    pub description: Vec<u8>,

    /// Natural height of the video.
    pub coded_height: u16,

    /// Natural width of the video.
    pub coded_width: u16,
}

impl Config {
    pub fn is_av1(&self) -> bool {
        self.codec.starts_with("av01")
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

    #[error("The media type of the blob is not a video: {provided_or_detected_media_type}")]
    MimeTypeIsNotAVideo {
        provided_or_detected_media_type: String,
    },

    #[error("MIME type '{provided_or_detected_media_type}' is not supported for videos")]
    UnsupportedMimeType {
        provided_or_detected_media_type: String,
    },

    /// Not used in `re_video` itself, but useful for media type detection ahead of calling [`VideoData::load_from_bytes`].
    #[error("Could not detect MIME type from the video contents")]
    UnrecognizedMimeType,

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
            .field("gops", &self.gops)
            .field(
                "samples",
                &self.samples.iter().enumerate().collect::<Vec<_>>(),
            )
            .field("data", &self.data.len())
            .finish()
    }
}
