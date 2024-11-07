//! Video demultiplexing.
//!
//! Parses a video file into a raw [`VideoData`] struct, which contains basic metadata and a list of [`GroupOfPictures`]s.
//!
//! The entry point is [`VideoData::load_from_bytes`]
//! which produces an instance of [`VideoData`] from any supported video container.

pub mod mp4;

use std::{collections::BTreeMap, ops::Range};

use itertools::Itertools as _;

use super::{Time, Timescale};

use crate::{Chunk, TrackId, TrackKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromaSubsamplingModes {
    /// No subsampling.
    Yuv444,

    /// Subsampling in X only.
    Yuv422,

    /// Subsampling in both X and Y.
    Yuv420,
}

impl std::fmt::Display for ChromaSubsamplingModes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Yuv444 => write!(f, "4:4:4"),
            Self::Yuv422 => write!(f, "4:2:2"),
            Self::Yuv420 => write!(f, "4:2:0"),
        }
    }
}

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

    /// Meta information about the samples.
    pub sample_statistics: SampleStatistics,

    /// All the tracks in the mp4; not just the video track.
    ///
    /// Can be nice to show in a UI.
    pub mp4_tracks: BTreeMap<TrackId, Option<TrackKind>>,
}

/// Meta informationa about the video samples.
#[derive(Clone, Debug)]
pub struct SampleStatistics {
    /// The smallest presentation timestamp observed in this video.
    ///
    /// This is typically 0, but in the presence of B-frames, it may be non-zero.
    /// In fact, many formats don't require this to be zero, but video players typically
    /// normalize the shown time to start at zero.
    /// Note that timestamps in the [`Sample`]s are *not* automatically adjusted with this value.
    // This is roughly equivalent to FFmpeg's internal `min_corrected_pts`
    // https://github.com/FFmpeg/FFmpeg/blob/4047b887fc44b110bccb1da09bcb79d6e454b88b/libavformat/isom.h#L202
    // (unlike us, this handles a bunch more edge cases but it fulfills the same role)
    // To learn more about this I recommend reading the patch that introduced this in FFmpeg:
    // https://patchwork.ffmpeg.org/project/ffmpeg/patch/20170606181601.25187-1-isasi@google.com/#12592
    pub minimum_presentation_timestamp: Time,

    /// Whether all decode timestamps are equal to presentation timestamps.
    ///
    /// If true, the video typically has no B-frames as those require frame reordering.
    pub dts_always_equal_pts: bool,
}

impl SampleStatistics {
    pub fn new(samples: &[Sample]) -> Self {
        re_tracing::profile_function!();

        let minimum_presentation_timestamp = samples
            .iter()
            .map(|s| s.presentation_timestamp)
            .min()
            .unwrap_or_default();
        let dts_always_equal_pts = samples
            .iter()
            .all(|s| s.decode_timestamp == s.presentation_timestamp);

        Self {
            minimum_presentation_timestamp,
            dts_always_equal_pts,
        }
    }
}

impl VideoData {
    /// Loads a video from the given data.
    ///
    /// TODO(andreas, jan): This should not copy the data, but instead store slices into a shared buffer.
    /// at the very least the should be a way to extract only metadata.
    pub fn load_from_bytes(data: &[u8], media_type: &str) -> Result<Self, VideoLoadError> {
        re_tracing::profile_function!();
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

    /// Length of the video.
    #[inline]
    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_nanos(self.duration.into_nanos(self.timescale) as _)
    }

    /// Natural width and height of the video
    #[inline]
    pub fn dimensions(&self) -> [u32; 2] {
        [self.width(), self.height()]
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
    pub fn human_readable_codec_string(&self) -> String {
        let human_readable = match &self.config.stsd.contents {
            re_mp4::StsdBoxContent::Av01(_) => "AV1",
            re_mp4::StsdBoxContent::Avc1(_) => "H.264",
            re_mp4::StsdBoxContent::Hvc1(_) => "H.265 HVC1",
            re_mp4::StsdBoxContent::Hev1(_) => "H.265 HEV1",
            re_mp4::StsdBoxContent::Vp08(_) => "VP8",
            re_mp4::StsdBoxContent::Vp09(_) => "VP9",
            re_mp4::StsdBoxContent::Mp4a(_) => "AAC",
            re_mp4::StsdBoxContent::Tx3g(_) => "TTXT",
            re_mp4::StsdBoxContent::Unknown(_) => "Unknown",
        };

        if let Some(codec) = self.config.stsd.contents.codec_string() {
            format!("{human_readable} ({codec})")
        } else {
            human_readable.to_owned()
        }
    }

    /// The number of samples in the video.
    #[inline]
    pub fn num_samples(&self) -> usize {
        self.samples.len()
    }

    /// Returns the subsampling mode of the video.
    ///
    /// Returns None if not detected or unknown.
    pub fn subsampling_mode(&self) -> Option<ChromaSubsamplingModes> {
        match &self.config.stsd.contents {
            re_mp4::StsdBoxContent::Av01(av01_box) => {
                // These are boolean options, see https://aomediacodec.github.io/av1-isobmff/#av1codecconfigurationbox-semantics
                match (
                    av01_box.av1c.chroma_subsampling_x != 0,
                    av01_box.av1c.chroma_subsampling_y != 0,
                ) {
                    (true, true) => Some(ChromaSubsamplingModes::Yuv420),
                    (true, false) => Some(ChromaSubsamplingModes::Yuv422),
                    (false, true) => None, // Downsampling in Y but not in X is unheard of!
                    // Either that or monochrome.
                    // See https://aomediacodec.github.io/av1-spec/av1-spec.pdf#page=131
                    (false, false) => Some(ChromaSubsamplingModes::Yuv444),
                }
            }
            re_mp4::StsdBoxContent::Avc1(_)
            | re_mp4::StsdBoxContent::Hvc1(_)
            | re_mp4::StsdBoxContent::Hev1(_) => {
                // Surely there's a way to get this!
                None
            }

            re_mp4::StsdBoxContent::Vp08(vp08_box) => {
                // Via https://www.ffmpeg.org/doxygen/4.3/vpcc_8c_source.html#l00116
                // enum VPX_CHROMA_SUBSAMPLING
                // {
                //     VPX_SUBSAMPLING_420_VERTICAL = 0,
                //     VPX_SUBSAMPLING_420_COLLOCATED_WITH_LUMA = 1,
                //     VPX_SUBSAMPLING_422 = 2,
                //     VPX_SUBSAMPLING_444 = 3,
                // };
                match vp08_box.vpcc.chroma_subsampling {
                    0 | 1 => Some(ChromaSubsamplingModes::Yuv420),
                    2 => Some(ChromaSubsamplingModes::Yuv422),
                    3 => Some(ChromaSubsamplingModes::Yuv444),
                    _ => None, // Unknown mode.
                }
            }
            re_mp4::StsdBoxContent::Vp09(vp09_box) => {
                // As above!
                match vp09_box.vpcc.chroma_subsampling {
                    0 | 1 => Some(ChromaSubsamplingModes::Yuv420),
                    2 => Some(ChromaSubsamplingModes::Yuv422),
                    3 => Some(ChromaSubsamplingModes::Yuv444),
                    _ => None, // Unknown mode.
                }
            }

            re_mp4::StsdBoxContent::Mp4a(_)
            | re_mp4::StsdBoxContent::Tx3g(_)
            | re_mp4::StsdBoxContent::Unknown(_) => None,
        }
    }

    /// Per color component bit depth.
    ///
    /// Usually 8, but 10 for HDR (for example).
    pub fn bit_depth(&self) -> Option<u8> {
        self.config.stsd.contents.bit_depth()
    }

    /// Returns None if the mp4 doesn't specify whether the video is monochrome or
    /// we haven't yet implemented the logic to determine this.
    pub fn is_monochrome(&self) -> Option<bool> {
        match &self.config.stsd.contents {
            re_mp4::StsdBoxContent::Av01(av01_box) => Some(av01_box.av1c.monochrome),
            re_mp4::StsdBoxContent::Avc1(_)
            | re_mp4::StsdBoxContent::Hvc1(_)
            | re_mp4::StsdBoxContent::Hev1(_) => {
                // It should be possible to extract this from the picture parameter set.
                None
            }
            re_mp4::StsdBoxContent::Vp08(_) | re_mp4::StsdBoxContent::Vp09(_) => {
                // Similar to AVC/HEVC, this information is likely accessible.
                None
            }

            re_mp4::StsdBoxContent::Mp4a(_)
            | re_mp4::StsdBoxContent::Tx3g(_)
            | re_mp4::StsdBoxContent::Unknown(_) => None,
        }
    }

    /// Determines the video timestamps of all frames inside a video, returning raw time values.
    ///
    /// Returned timestamps are in nanoseconds since start and are guaranteed to be monotonically increasing.
    /// These are *not* necessarily the same as the presentation timestamps, as the returned timestamps are
    /// normalized respect to the start of the video, see [`SampleStatistics::minimum_presentation_timestamp`].
    pub fn frame_timestamps_ns(&self) -> impl Iterator<Item = i64> + '_ {
        re_tracing::profile_function!();

        // Segments are guaranteed to be sorted among each other, but within a segment,
        // presentation timestamps may not be sorted since this is sorted by decode timestamps.
        self.gops.iter().flat_map(|seg| {
            self.samples[seg.range()]
                .iter()
                .map(|sample| sample.presentation_timestamp)
                .sorted()
                .map(|pts| {
                    (pts - self.sample_statistics.minimum_presentation_timestamp)
                        .into_nanos(self.timescale)
                })
        })
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
///
/// This is equivalent to MP4's definition of a single sample.
/// Note that in MP4, each sample is forms a single access unit,
/// see 3.1.1 [ISO_IEC_14496-14](https://ossrs.io/lts/zh-cn/assets/files/ISO_IEC_14496-14-MP4-2003-9a3eb04879ded495406399602ff2e587.pdf):
/// > 3.1.1 Elementary Stream Data
/// > To maintain the goals of streaming protocol independence, the media data is stored in its most ‘natural’ format,
/// > and not fragmented. This enables easy local manipulation of the media data. Therefore media-data is stored
/// > as access units, a range of contiguous bytes for each access unit (a single access unit is the definition of a
/// > ‘sample’ for an MPEG-4 media stream).
///
/// Access units in H.264/H.265 are always yielding a single frame upon decoding,
/// see <https://en.wikipedia.org/wiki/Network_Abstraction_Layer#Access_Units/>:
/// > A set of NAL units in a specified form is referred to as an access unit.
/// > The decoding of each access unit results in one decoded picture.
#[derive(Debug, Clone)]
pub struct Sample {
    /// Is t his the start of a new [`GroupOfPictures`]?
    pub is_sync: bool,

    /// Time at which this sample appears in the decoded bitstream, in time units.
    ///
    /// Samples should be decoded in this order.
    ///
    /// `decode_timestamp <= presentation_timestamp`
    pub decode_timestamp: Time,

    /// Time at which this sample appears in the frame stream, in time units.
    /// Often synonymous with `presentation_timestamp`.
    ///
    /// The frame should be shown at this time.
    ///
    /// `decode_timestamp <= presentation_timestamp`
    pub presentation_timestamp: Time,

    /// Duration of the sample, in time units.
    pub duration: Time,

    /// Offset into the video data.
    pub byte_offset: u32,

    /// Length of sample starting at [`Sample::byte_offset`].
    pub byte_length: u32,
}

impl Sample {
    /// Read the sample from the video data.
    ///
    /// Note that `data` _must_ be a reference to the original MP4 file
    /// from which the [`VideoData`] was loaded.
    ///
    /// Returns `None` if the sample is out of bounds, which can only happen
    /// if `data` is not the original video data.
    pub fn get(&self, data: &[u8]) -> Option<Chunk> {
        let data = data
            .get(self.byte_offset as usize..(self.byte_offset + self.byte_length) as usize)?
            .to_vec();
        Some(Chunk {
            data,
            decode_timestamp: self.decode_timestamp,
            presentation_timestamp: self.presentation_timestamp,
            duration: self.duration,
            is_sync: self.is_sync,
        })
    }
}

/// Configuration of a video.
#[derive(Debug, Clone)]
pub struct Config {
    /// Contains info about the codec, bit depth, etc.
    pub stsd: re_mp4::StsdBox,

    /// Codec-specific configuration.
    pub description: Vec<u8>,

    /// Natural height of the video.
    pub coded_height: u16,

    /// Natural width of the video.
    pub coded_width: u16,
}

impl Config {
    pub fn is_av1(&self) -> bool {
        matches!(self.stsd.contents, re_mp4::StsdBoxContent::Av01 { .. })
    }

    pub fn is_h264(&self) -> bool {
        matches!(self.stsd.contents, re_mp4::StsdBoxContent::Avc1 { .. })
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
            .finish()
    }
}
