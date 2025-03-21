//! Video demultiplexing.
//!
//! Parses a video file into a raw [`VideoData`] struct, which contains basic metadata and a list of [`GroupOfPictures`]s.
//!
//! The entry point is [`VideoData::load_from_bytes`]
//! which produces an instance of [`VideoData`] from any supported video container.

pub mod mp4;

use std::{collections::BTreeMap, ops::Range};

use bit_vec::BitVec;
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
    ///
    /// In MP4, one sample is one frame.
    pub samples: Vec<Sample>,

    /// Meta information about the samples.
    pub samples_statistics: SamplesStatistics,

    /// All the tracks in the mp4; not just the video track.
    ///
    /// Can be nice to show in a UI.
    pub mp4_tracks: BTreeMap<TrackId, Option<TrackKind>>,
}

/// Meta informationa about the video samples.
#[derive(Clone, Debug)]
pub struct SamplesStatistics {
    /// Whether all decode timestamps are equal to presentation timestamps.
    ///
    /// If true, the video typically has no B-frames as those require frame reordering.
    pub dts_always_equal_pts: bool,

    /// If `dts_always_equal_pts` is false, then this gives for each sample whether its PTS is the highest seen so far.
    /// If `dts_always_equal_pts` is true, then this is left as `None`.
    /// This is used for optimizing PTS search.
    pub has_sample_highest_pts_so_far: Option<BitVec>,
}

impl SamplesStatistics {
    pub fn new(samples: &[Sample]) -> Self {
        re_tracing::profile_function!();

        let dts_always_equal_pts = samples
            .iter()
            .all(|s| s.decode_timestamp == s.presentation_timestamp);

        let mut biggest_pts_so_far = Time::MIN;
        let has_sample_highest_pts_so_far = (!dts_always_equal_pts).then(|| {
            samples
                .iter()
                .map(move |sample| {
                    if sample.presentation_timestamp > biggest_pts_so_far {
                        biggest_pts_so_far = sample.presentation_timestamp;
                        true
                    } else {
                        false
                    }
                })
                .collect()
        });

        Self {
            dts_always_equal_pts,
            has_sample_highest_pts_so_far,
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
        self.duration.duration(self.timescale)
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
    pub fn frame_timestamps_nanos(&self) -> impl Iterator<Item = i64> + '_ {
        // Segments are guaranteed to be sorted among each other, but within a segment,
        // presentation timestamps may not be sorted since this is sorted by decode timestamps.
        self.gops.iter().flat_map(|seg| {
            self.samples[seg.sample_range_usize()]
                .iter()
                .map(|sample| sample.presentation_timestamp)
                .sorted()
                .map(|pts| pts.into_nanos(self.timescale))
        })
    }

    /// For a given decode (!) timestamp, returns the index of the first sample whose
    /// decode timestamp is lesser than or equal to the given timestamp.
    fn latest_sample_index_at_decode_timestamp(
        samples: &[Sample],
        decode_time: Time,
    ) -> Option<usize> {
        latest_at_idx(samples, |sample| sample.decode_timestamp, &decode_time)
    }

    /// See [`Self::latest_sample_index_at_presentation_timestamp`], split out for testing purposes.
    fn latest_sample_index_at_presentation_timestamp_internal(
        samples: &[Sample],
        sample_statistics: &SamplesStatistics,
        presentation_timestamp: Time,
    ) -> Option<usize> {
        // Find the latest sample where `decode_timestamp <= presentation_timestamp`.
        // Because `decode <= presentation`, we never have to look further backwards in the
        // video than this.
        let decode_sample_idx =
            Self::latest_sample_index_at_decode_timestamp(samples, presentation_timestamp)?;

        // It's very common that dts==pts in which case we're done!
        let Some(has_sample_highest_pts_so_far) =
            sample_statistics.has_sample_highest_pts_so_far.as_ref()
        else {
            debug_assert!(sample_statistics.dts_always_equal_pts);
            return Some(decode_sample_idx);
        };
        debug_assert!(has_sample_highest_pts_so_far.len() == samples.len());

        // Search backwards, starting at `decode_sample_idx`, looking for
        // the first sample where `sample.presentation_timestamp <= presentation_timestamp`.
        // I.e. the sample with the biggest PTS that is smaller or equal to the requested PTS.
        //
        // The tricky part is that we can't just take the first sample with a presentation timestamp that matches
        // since smaller presentation timestamps may still show up further back!
        let mut best_index = usize::MAX;
        let mut best_pts = Time::MIN;
        for sample_idx in (0..=decode_sample_idx).rev() {
            let sample = &samples[sample_idx];

            if sample.presentation_timestamp == presentation_timestamp {
                // Clean hit. Take this one, no questions asked :)
                // (assuming that each PTS is unique!)
                return Some(sample_idx);
            }

            if sample.presentation_timestamp < presentation_timestamp
                && sample.presentation_timestamp > best_pts
            {
                best_pts = sample.presentation_timestamp;
                best_index = sample_idx;
            }

            if best_pts != Time::MIN && has_sample_highest_pts_so_far[sample_idx] {
                // We won't see any bigger PTS values anymore, meaning we're as close as we can get to the requested PTS!
                return Some(best_index);
            }
        }

        None
    }

    /// For a given presentation timestamp, return the index of the first sample
    /// whose presentation timestamp is lesser than or equal to the given timestamp.
    ///
    /// Remember that samples after (i.e. with higher index) may have a *lower* presentation time
    /// if the stream has sample reordering!
    pub fn latest_sample_index_at_presentation_timestamp(
        &self,
        presentation_timestamp: Time,
    ) -> Option<usize> {
        Self::latest_sample_index_at_presentation_timestamp_internal(
            &self.samples,
            &self.samples_statistics,
            presentation_timestamp,
        )
    }

    /// For a given decode (!) timestamp, return the index of the group of pictures (GOP) index containing the given timestamp.
    pub fn gop_index_containing_decode_timestamp(&self, decode_time: Time) -> Option<usize> {
        latest_at_idx(&self.gops, |gop| gop.decode_start_time, &decode_time)
    }

    /// For a given presentation timestamp, return the index of the group of pictures (GOP) index containing the given timestamp.
    pub fn gop_index_containing_presentation_timestamp(
        &self,
        presentation_timestamp: Time,
    ) -> Option<usize> {
        let requested_sample_index =
            self.latest_sample_index_at_presentation_timestamp(presentation_timestamp)?;

        // Do a binary search through GOPs by the decode timestamp of the found sample
        // to find the GOP that contains the sample.
        self.gop_index_containing_decode_timestamp(
            self.samples[requested_sample_index].decode_timestamp,
        )
    }
}

/// A Group of Pictures (GOP) always starts with an I-frame, followed by delta-frames.
///
/// See <https://en.wikipedia.org/wiki/Group_of_pictures> for more.
#[derive(Debug, Clone)]
pub struct GroupOfPictures {
    /// Decode timestamp of the first sample in this GOP, in time units.
    pub decode_start_time: Time,

    /// Range of samples contained in this GOP.
    pub sample_range: Range<u32>,
}

impl GroupOfPictures {
    /// The GOP's `sample_range` mapped to `usize` for slicing.
    pub fn sample_range_usize(&self) -> Range<usize> {
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
    /// Is this the start of a new [`GroupOfPictures`]?
    ///
    /// This probably means this is a _keyframe_, and that and entire frame
    /// can be decoded from only this one sample (though I'm not 100% sure).
    pub is_sync: bool,

    /// Which sample is this in the video?
    ///
    /// This is the order of which the samples appear in the container,
    /// which is usually ordered by [`Self::decode_timestamp`].
    pub sample_idx: usize,

    /// Which frame does this sample belong to?
    ///
    /// This is on the assumption that each sample produces a single frame,
    /// which is true for MP4.
    ///
    /// This is the index of samples ordered by [`Self::presentation_timestamp`].
    pub frame_nr: usize,

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
            sample_idx: self.sample_idx,
            frame_nr: self.frame_nr,
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

/// Returns the index of:
/// - The index of `needle` in `v`, if it exists
/// - The index of the first element in `v` that is lesser than `needle`, if it exists
/// - `None`, if `v` is empty OR `needle` is greater than all elements in `v`
pub fn latest_at_idx<T, K: Ord>(v: &[T], key: impl Fn(&T) -> K, needle: &K) -> Option<usize> {
    if v.is_empty() {
        return None;
    }

    let idx = v.partition_point(|x| key(x) <= *needle);

    if idx == 0 {
        // If idx is 0, then all elements are greater than the needle
        if &key(&v[0]) > needle {
            return None;
        }
    }

    Some(idx.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latest_at_idx() {
        let v = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(latest_at_idx(&v, |v| *v, &0), None);
        assert_eq!(latest_at_idx(&v, |v| *v, &1), Some(0));
        assert_eq!(latest_at_idx(&v, |v| *v, &2), Some(1));
        assert_eq!(latest_at_idx(&v, |v| *v, &3), Some(2));
        assert_eq!(latest_at_idx(&v, |v| *v, &4), Some(3));
        assert_eq!(latest_at_idx(&v, |v| *v, &5), Some(4));
        assert_eq!(latest_at_idx(&v, |v| *v, &6), Some(5));
        assert_eq!(latest_at_idx(&v, |v| *v, &7), Some(6));
        assert_eq!(latest_at_idx(&v, |v| *v, &8), Some(7));
        assert_eq!(latest_at_idx(&v, |v| *v, &9), Some(8));
        assert_eq!(latest_at_idx(&v, |v| *v, &10), Some(9));
        assert_eq!(latest_at_idx(&v, |v| *v, &11), Some(9));
        assert_eq!(latest_at_idx(&v, |v| *v, &1000), Some(9));
    }

    #[test]
    fn test_latest_sample_index_at_presentation_timestamp() {
        // This is a snippet of real world data!
        let pts = [
            0, 1024, 512, 256, 768, 2048, 1536, 1280, 1792, 3072, 2560, 2304, 2816, 4096, 3584,
            3328, 3840, 4864, 4352, 4608, 5888, 5376, 5120, 5632, 6912, 6400, 6144, 6656, 7936,
            7424, 7168, 7680, 8960, 8448, 8192, 8704, 9984, 9472, 9216, 9728, 11008, 10496, 10240,
            10752, 12032, 11520, 11264, 11776, 13056, 12544,
        ];
        let dts = [
            -512, -256, 0, 256, 512, 768, 1024, 1280, 1536, 1792, 2048, 2304, 2560, 2816, 3072,
            3328, 3584, 3840, 4096, 4352, 4608, 4864, 5120, 5376, 5632, 5888, 6144, 6400, 6656,
            6912, 7168, 7424, 7680, 7936, 8192, 8448, 8704, 8960, 9216, 9472, 9728, 9984, 10240,
            10496, 10752, 11008, 11264, 11520, 11776, 12032,
        ];

        // Checking our basic assumptions about this data:
        assert_eq!(pts.len(), dts.len());
        assert!(pts.iter().zip(dts.iter()).all(|(pts, dts)| dts <= pts));

        // Create fake samples from this.
        let samples = pts
            .into_iter()
            .zip(dts)
            .enumerate()
            .map(|(sample_idx, (pts, dts))| Sample {
                is_sync: false,
                sample_idx,
                frame_nr: 0, // unused
                decode_timestamp: Time(dts),
                presentation_timestamp: Time(pts),
                duration: Time(1),
                byte_offset: 0,
                byte_length: 0,
            })
            .collect::<Vec<_>>();

        let sample_statistics = SamplesStatistics::new(&samples);
        assert!(!sample_statistics.dts_always_equal_pts);

        // Test queries on the samples.
        let query_pts = |pts| {
            VideoData::latest_sample_index_at_presentation_timestamp_internal(
                &samples,
                &sample_statistics,
                pts,
            )
        };

        // Check that query for all exact positions works as expected using brute force search as the reference.
        for (idx, sample) in samples.iter().enumerate() {
            assert_eq!(Some(idx), query_pts(sample.presentation_timestamp));
        }

        // Check that for slightly offsetted positions the query is still correct.
        // This works because for this dataset we know the minimum presentation timesetampe distance is always 256.
        for (idx, sample) in samples.iter().enumerate() {
            assert_eq!(
                Some(idx),
                query_pts(sample.presentation_timestamp + Time(1))
            );
            assert_eq!(
                Some(idx),
                query_pts(sample.presentation_timestamp + Time(255))
            );
        }

        // A few hardcoded cases - both for illustrative purposes and to make sure the generic tests above are correct.

        // Querying before the first sample.
        assert_eq!(None, query_pts(Time(-1)));
        assert_eq!(None, query_pts(Time(-123)));

        // Querying for the first sample
        assert_eq!(Some(0), query_pts(Time(0)));
        assert_eq!(Some(0), query_pts(Time(1)));
        assert_eq!(Some(0), query_pts(Time(88)));
        assert_eq!(Some(0), query_pts(Time(255)));

        // The next sample is a jump in index!
        assert_eq!(Some(3), query_pts(Time(256)));
        assert_eq!(Some(3), query_pts(Time(257)));
        assert_eq!(Some(3), query_pts(Time(400)));
        assert_eq!(Some(3), query_pts(Time(511)));

        // And the one after that should jump back again.
        assert_eq!(Some(2), query_pts(Time(512)));
        assert_eq!(Some(2), query_pts(Time(513)));
        assert_eq!(Some(2), query_pts(Time(600)));
        assert_eq!(Some(2), query_pts(Time(767)));

        // And another one!
        assert_eq!(Some(4), query_pts(Time(768)));
        assert_eq!(Some(4), query_pts(Time(1023)));

        // Test way outside of the range.
        // (this is not the last element in the list since that one doesn't have the highest PTS)
        assert_eq!(Some(48), query_pts(Time(123123123123123123)));
    }
}
