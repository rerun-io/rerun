//! Video demultiplexing.
//!
//! Parses a video file into a raw [`VideoDataDescription`] struct, which contains basic metadata and a list of [`GroupOfPictures`]s.
//!
//! The entry point is [`VideoDataDescription::load_from_bytes`]
//! which produces an instance of [`VideoDataDescription`] from any supported video container.

pub mod mp4;

use std::{collections::BTreeMap, ops::Range};

use bit_vec::BitVec;
use itertools::Itertools as _;

use super::{Time, Timescale};

use crate::{Chunk, StableIndexDeque, TrackId, TrackKind};

/// Chroma subsampling mode.
///
/// Unlike [`crate::YuvPixelLayout`] this does not specify a certain planarity/layout of
/// the chroma components.
/// Instead, this is just a description whether any subsampling occurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromaSubsamplingModes {
    /// No subsampling at all, since the format is monochrome.
    Monochrome,

    /// No subsampling.
    ///
    /// Note that this also applies to RGB formats, not just YUV.
    /// (but for video data YUV is much more common regardless)
    Yuv444,

    /// Subsampling in X only.
    Yuv422,

    /// Subsampling in both X and Y.
    Yuv420,
}

impl std::fmt::Display for ChromaSubsamplingModes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Could also call this 4:0:0, but that's a fairly uncommon way to describe it.
            Self::Monochrome => write!(f, "monochrome"),
            Self::Yuv444 => write!(f, "4:4:4"),
            Self::Yuv422 => write!(f, "4:2:2"),
            Self::Yuv420 => write!(f, "4:2:0"),
        }
    }
}

/// The basic codec family used to encode the video.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    /// Advanced Video Coding (AVC/H.264)
    ///
    /// See <https://en.wikipedia.org/wiki/Advanced_Video_Coding>
    H264,

    /// High Efficiency Video Coding (HEVC/H.265)
    ///
    /// See <https://en.wikipedia.org/wiki/High_Efficiency_Video_Coding>
    H265,

    /// AOMedia Video 1 (AV1)
    ///
    /// See <https://en.wikipedia.org/wiki/AV1>
    AV1,

    /// VP8
    ///
    /// See <https://en.wikipedia.org/wiki/VP8>
    VP8,

    /// VP9
    ///
    /// See <https://en.wikipedia.org/wiki/VP9>
    VP9,
}

impl VideoCodec {
    /// Base part of the web codec string, without additional parameters.
    ///
    /// See <https://www.w3.org/TR/webcodecs-codec-registry/#video-codec-registry>
    pub fn base_webcodec_string(&self) -> &'static str {
        match self {
            // https://www.w3.org/TR/webcodecs-av1-codec-registration/#fully-qualified-codec-strings
            Self::AV1 => "av01",

            // https://www.w3.org/TR/webcodecs-avc-codec-registration/#fully-qualified-codec-strings
            // avc3 is valid as well.
            Self::H264 => "avc1",

            // https://www.w3.org/TR/webcodecs-hevc-codec-registration/#fully-qualified-codec-strings
            // hvc1 is valid as well.
            Self::H265 => "hev1",

            // https://www.w3.org/TR/webcodecs-vp8-codec-registration/#fully-qualified-codec-strings
            // Special! This *is* the fully qualified codec string.
            Self::VP8 => "vp8",

            // https://www.w3.org/TR/webcodecs-vp9-codec-registration/#fully-qualified-codec-strings
            Self::VP9 => "vp09",
        }
    }
}

/// Index used for referencing into [`VideoDataDescription::gops`].
pub type GopIndex = usize;

/// Index used for referencing into [`VideoDataDescription::samples`].
pub type SampleIndex = usize;

/// Description of video data.
///
/// Store various metadata about a video.
/// Doesn't contain the actual data, but rather refers to samples with a byte offset.
#[derive(Clone)]
pub struct VideoDataDescription {
    /// The codec used to encode the video.
    pub codec: VideoCodec,

    /// Various information about how the video was encoded.
    ///
    /// Should any of this change during the lifetime of a decoder, it has to be reset.
    ///
    /// For video streams this is derived on the fly, therefore it may arrive only with the first key frame.
    /// For mp4 this is read from the AVCC box.
    pub encoding_details: Option<VideoEncodingDetails>,

    /// How many time units are there per second.
    ///
    /// `None` if the time units used don't have a defined relationship to seconds.
    /// This happens for streams logged on a non-temporal timeline.
    pub timescale: Option<Timescale>,

    /// Duration of the video, in time units if known.
    ///
    /// For open ended video streams rather than video files this is generally unknown.
    pub duration: Option<Time>,

    /// We split video into GOPs, each beginning with a key frame,
    /// followed by any number of delta frames.
    ///
    /// To facilitate streaming, gops at the beginning of the queue may be discarded over time
    /// and new ones may be added. Also, the most recent gop may grow over time.
    pub gops: StableIndexDeque<GroupOfPictures>,

    /// Samples contain the byte offsets into `data` for each frame.
    ///
    /// This list is sorted in ascending order of decode timestamps.
    ///
    /// Samples must be decoded in decode-timestamp order,
    /// and should be presented in composition-timestamp order.
    ///
    /// We assume one sample yields exactly one frame from the decoder.
    ///
    /// To facilitate streaming, samples may be removed from the beginning and added at the end,
    /// but individual samples are never supposed to change.
    pub samples: StableIndexDeque<SampleMetadata>,

    /// Meta information about the samples.
    pub samples_statistics: SamplesStatistics,

    /// All the tracks in the mp4; not just the video track.
    ///
    /// Can be nice to show in a UI.
    pub mp4_tracks: BTreeMap<TrackId, Option<TrackKind>>,
}

/// Various information about how the video was encoded.
///
/// For video streams this is derived on the fly.
/// For mp4 this is read from the AVCC box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoEncodingDetails {
    /// Detailed codec string as specified by the `WebCodecs` codec registry.
    ///
    /// See <https://www.w3.org/TR/webcodecs-codec-registry/#video-codec-registry>
    pub codec_string: String,

    /// Encoded width & height.
    pub coded_dimensions: [u16; 2],

    /// Per color component bit depth.
    ///
    /// Usually 8, but 10 for HDR (for example).
    ///
    /// `None` if this couldn't be determined, either because of lack of implementation
    /// or missing information at this point.
    pub bit_depth: Option<u8>,

    /// Chroma subsampling mode.
    ///
    /// `None` if this couldn't be determined, either because of lack of implementation
    /// or missing information at this point.
    pub chroma_subsampling: Option<ChromaSubsamplingModes>,

    /// Optional mp4 stsd box from which this data was derived.
    ///
    /// Used by some decoders directly for configuration.
    /// For H.264 & H.265, its presence implies that the bitstream is in the AVCC format rather than Annex B.
    // TODO(andreas):
    // It would be nice to instead have an enum of all the actually needed descriptors.
    // We know for sure that H.264 & H.265 need an AVCC/HVCC box for data from mp4, since the stream
    // is otherwise not readable. But what about the other codecs? On Web we *do* pass additional information right now.
    pub stsd: Option<re_mp4::StsdBox>,
}

impl VideoEncodingDetails {
    /// Get the AVCC box from the stsd box if any.
    pub fn avcc(&self) -> Option<&re_mp4::Avc1Box> {
        self.stsd.as_ref().and_then(|stsd| match &stsd.contents {
            re_mp4::StsdBoxContent::Avc1(avc1) => Some(avc1),
            _ => None,
        })
    }
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
    ///
    /// TODO(andreas): We don't have a mechanism for shrinking this bitvec when dropping samples, i.e. it will keep growing.
    /// ([`StableIndexDeque`] makes sure that indices in the bitvec will still match up with the samples even when samples are dropped from the front.)
    pub has_sample_highest_pts_so_far: Option<BitVec>,
}

impl SamplesStatistics {
    /// Special case for videos that have no h264/h265 B-frames.
    ///
    /// This is the most common case for video streams.
    // TODO(andreas): so, av1 bframes are possible with this config, right?! confirm and then maybe come up with a better name.
    pub const NO_BFRAMES: Self = Self {
        dts_always_equal_pts: true,
        has_sample_highest_pts_so_far: None,
    };

    pub fn new(samples: &StableIndexDeque<SampleMetadata>) -> Self {
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

impl VideoDataDescription {
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

    /// Length of the video if known.
    ///
    /// For video streams (as opposed to video files) this is generally unknown.
    #[inline]
    pub fn duration(&self) -> Option<std::time::Duration> {
        let timescale = self.timescale?;
        let duration = self.duration?;
        Some(duration.duration(timescale))
    }

    /// The codec used to encode the video.
    #[inline]
    pub fn human_readable_codec_string(&self) -> String {
        let base_codec_string = match &self.codec {
            VideoCodec::AV1 => "AV1",
            VideoCodec::H264 => "H.264 AVC1",
            VideoCodec::H265 => "H.265 HEV1",
            VideoCodec::VP8 => "VP8",
            VideoCodec::VP9 => "VP9",
        }
        .to_owned();

        if let Some(encoding_details) = self.encoding_details.as_ref() {
            format!("{base_codec_string} ({})", encoding_details.codec_string)
        } else {
            base_codec_string
        }
    }

    /// The number of samples in the video.
    #[inline]
    pub fn num_samples(&self) -> usize {
        self.samples.num_elements()
    }

    /// Determines the video timestamps of all frames inside a video, returning raw time values.
    ///
    /// Returns None if the video has no timescale.
    /// Returned timestamps are in nanoseconds since start and are guaranteed to be monotonically increasing.
    pub fn frame_timestamps_nanos(&self) -> Option<impl Iterator<Item = i64> + '_> {
        let timescale = self.timescale?;

        // Segments are guaranteed to be sorted among each other, but within a segment,
        // presentation timestamps may not be sorted since this is sorted by decode timestamps.
        Some(self.gops.iter().flat_map(move |seg| {
            self.samples
                .iter_index_range_clamped(&seg.sample_range)
                .map(|sample| sample.presentation_timestamp)
                .sorted()
                .map(move |pts| pts.into_nanos(timescale))
        }))
    }

    /// For a given decode (!) timestamp, returns the index of the first sample whose
    /// decode timestamp is lesser than or equal to the given timestamp.
    fn latest_sample_index_at_decode_timestamp(
        samples: &StableIndexDeque<SampleMetadata>,
        decode_time: Time,
    ) -> Option<SampleIndex> {
        samples.latest_at_idx(|sample| sample.decode_timestamp, &decode_time)
    }

    /// See [`Self::latest_sample_index_at_presentation_timestamp`], split out for testing purposes.
    fn latest_sample_index_at_presentation_timestamp_internal(
        samples: &StableIndexDeque<SampleMetadata>,
        sample_statistics: &SamplesStatistics,
        presentation_timestamp: Time,
    ) -> Option<SampleIndex> {
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
        debug_assert!(has_sample_highest_pts_so_far.len() == samples.next_index());

        // Search backwards, starting at `decode_sample_idx`, looking for
        // the first sample where `sample.presentation_timestamp <= presentation_timestamp`.
        // I.e. the sample with the biggest PTS that is smaller or equal to the requested PTS.
        //
        // The tricky part is that we can't just take the first sample with a presentation timestamp that matches
        // since smaller presentation timestamps may still show up further back!
        let mut best_index = SampleIndex::MAX;
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
    ) -> Option<SampleIndex> {
        Self::latest_sample_index_at_presentation_timestamp_internal(
            &self.samples,
            &self.samples_statistics,
            presentation_timestamp,
        )
    }

    /// For a given decode (!) timestamp, return the index of the group of pictures (GOP) index containing the given timestamp.
    pub fn gop_index_containing_decode_timestamp(&self, decode_time: Time) -> Option<GopIndex> {
        self.gops.latest_at_idx(
            |gop| self.samples[gop.sample_range.start].decode_timestamp,
            &decode_time,
        )
    }

    /// For a given presentation timestamp, return the index of the group of pictures (GOP) index containing the given timestamp.
    pub fn gop_index_containing_presentation_timestamp(
        &self,
        presentation_timestamp: Time,
    ) -> Option<SampleIndex> {
        let requested_sample_index =
            self.latest_sample_index_at_presentation_timestamp(presentation_timestamp)?;

        // Do a binary search through GOPs by the decode timestamp of the found sample
        // to find the GOP that contains the sample.
        self.gop_index_containing_decode_timestamp(
            self.samples[requested_sample_index].decode_timestamp,
        )
    }
}

/// A Group of Pictures (GOP) always starts with an I(DR)-frame, followed by delta-frames.
///
/// See <https://en.wikipedia.org/wiki/Group_of_pictures> for more.
/// We generally refer to "closed GOPs" only, such that they are re-entrant for decoders
/// (as opposed to "open GOPs" which may refer to frames from other GOPs).
#[derive(Debug, Clone)]
pub struct GroupOfPictures {
    /// Range of samples contained in this GOP.
    // TODO(andreas): sample ranges between GOPs are guaranteed to be contiguous.
    // So we could actually just read the second part of the range by looking at the next gop.
    pub sample_range: Range<SampleIndex>,
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
pub struct SampleMetadata {
    /// Is this the start of a new (closed) [`GroupOfPictures`]?
    ///
    /// What this means in detail is dependent on the codec but they are generally
    /// at least I(DR)-frames and often have additional metadata such that
    /// a decoder can restart at this frame.
    pub is_sync: bool,

    /// Which frame does this sample belong to?
    ///
    /// This is on the assumption that each sample produces a single frame,
    /// which is true for MP4.
    ///
    /// This is the index of samples ordered by [`Self::presentation_timestamp`].
    ///
    /// Do **not** ever use this for indexing into the array of samples.
    pub frame_nr: u32,

    /// Time at which this sample appears in the decoded bitstream, in time units.
    ///
    /// Samples should be decoded in this order.
    ///
    /// `decode_timestamp <= presentation_timestamp`
    pub decode_timestamp: Time,

    /// Time at which this sample appears in the frame stream, in time units.
    ///
    /// The frame should be shown at this time.
    ///
    /// `decode_timestamp <= presentation_timestamp`
    pub presentation_timestamp: Time,

    /// Duration of the sample.
    ///
    /// Typically the time difference in presentation timestamp to the next sample.
    /// May be unknown if this is the last sample in an ongoing video stream.
    pub duration: Option<Time>,

    /// Index of the data buffer in which this sample is stored.
    pub buffer_index: usize,

    /// Offset within the data buffer addressed by [`SampleMetadata::buffer_index`].
    pub byte_offset: u32,

    /// Length of sample starting at [`SampleMetadata::byte_offset`].
    pub byte_length: u32,
}

impl SampleMetadata {
    /// Read the sample from the video data.
    ///
    /// For video assets, `data` _must_ be a reference to the original asset
    /// from which the [`VideoDataDescription`] was loaded.
    /// For video streams, `data` refers to the currently available data
    /// which is described by the [`VideoDataDescription`].
    ///
    /// Returns `None` if the sample is out of bounds, which can only happen
    /// if `data` is not the original video data.
    pub fn get(&self, buffers: &StableIndexDeque<&[u8]>, sample_idx: SampleIndex) -> Option<Chunk> {
        let buffer = *buffers.get(self.buffer_index)?;
        let data = buffer
            .get(self.byte_offset as usize..(self.byte_offset + self.byte_length) as usize)?
            .to_vec();

        Some(Chunk {
            data,
            sample_idx,
            frame_nr: self.frame_nr,
            decode_timestamp: self.decode_timestamp,
            presentation_timestamp: self.presentation_timestamp,
            duration: self.duration,
            is_sync: self.is_sync,
        })
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

    #[error(
        "Video file has no timescale, which is required to determine frame timestamps in time units"
    )]
    NoTimescale,

    #[error("The media type of the blob is not a video: {provided_or_detected_media_type}")]
    MimeTypeIsNotAVideo {
        provided_or_detected_media_type: String,
    },

    #[error("MIME type '{provided_or_detected_media_type}' is not supported for videos")]
    UnsupportedMimeType {
        provided_or_detected_media_type: String,
    },

    /// Not used in `re_video` itself, but useful for media type detection ahead of calling [`VideoDataDescription::load_from_bytes`].
    #[error("Could not detect MIME type from the video contents")]
    UnrecognizedMimeType,

    // `FourCC`'s debug impl doesn't quote the result
    #[error("Video track uses unsupported codec \"{0}\"")] // NOLINT
    UnsupportedCodec(re_mp4::FourCC),

    #[error("Unable to determine codec string from the video contents")]
    UnableToDetermineCodecString,

    #[error("Failed to parse H.264 SPS from mp4: {0:?}")]
    SpsParsingError(h264_reader::nal::sps::SpsError),
}

impl std::fmt::Debug for VideoDataDescription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Video")
            .field("codec", &self.codec)
            .field("encoding_details", &self.encoding_details)
            .field("timescale", &self.timescale)
            .field("duration", &self.duration)
            .field("gops", &self.gops)
            .field("samples", &self.samples.iter_indexed().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .map(|(pts, dts)| SampleMetadata {
                is_sync: false,
                frame_nr: 0, // unused
                decode_timestamp: Time(dts),
                presentation_timestamp: Time(pts),
                duration: Some(Time(1)),
                buffer_index: 0,
                byte_offset: 0,
                byte_length: 0,
            })
            .collect::<StableIndexDeque<_>>();

        let sample_statistics = SamplesStatistics::new(&samples);
        assert!(!sample_statistics.dts_always_equal_pts);

        // Test queries on the samples.
        let query_pts = |pts| {
            VideoDataDescription::latest_sample_index_at_presentation_timestamp_internal(
                &samples,
                &sample_statistics,
                pts,
            )
        };

        // Check that query for all exact positions works as expected using brute force search as the reference.
        for (idx, sample) in samples.iter_indexed() {
            assert_eq!(Some(idx), query_pts(sample.presentation_timestamp));
        }

        // Check that for slightly offsetted positions the query is still correct.
        // This works because for this dataset we know the minimum presentation timesetampe distance is always 256.
        for (idx, sample) in samples.iter_indexed() {
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
