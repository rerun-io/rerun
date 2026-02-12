//! Video demultiplexing.
//!
//! Parses a video file into a raw [`VideoDataDescription`] struct, which contains basic metadata and a list of keyframes.
//!
//! The entry point is [`VideoDataDescription::load_from_bytes`]
//! which produces an instance of [`VideoDataDescription`] from any supported video container.

pub mod mp4;

use std::collections::BTreeMap;

use bit_vec::BitVec;
use itertools::Itertools as _;
use re_span::Span;
use re_tuid::Tuid;
use web_time::Instant;

use super::{Time, Timescale};
use crate::nalu::AnnexBStreamWriteError;
use crate::{
    Chunk, StableIndexDeque, TrackId, TrackKind, write_avc_chunk_to_annexb,
    write_hevc_chunk_to_annexb,
};

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

/// Index used for referencing into [`VideoDataDescription::samples`].
pub type SampleIndex = usize;

/// An index into [`VideoDataDescription::keyframe_indices`], not stable between mutations.
pub type KeyframeIndex = usize;

/// Distinguishes static videos from potentially ongoing video streams.
#[derive(Clone)]
pub enum VideoDeliveryMethod {
    /// A static video with a fixed, known duration which won't be updated further.
    Static { duration: Time },

    /// A stream that *may* be periodically updated.
    ///
    /// Video streams may drop samples at the beginning and add new samples at the end.
    /// The last sample's duration is treated as unknown.
    /// However, it is typically assumed to be as long as the average sample duration.
    Stream {
        /// Last time we added/removed samples from the [`VideoDataDescription`].
        ///
        /// This is used solely as a heuristic input for how the player schedules work to decoders.
        /// For live streams, even those that stopped, this is expected to be wallclock time of when a sample was
        /// added do this datastructure. *Not* when the sample was first recorded.
        last_time_updated_samples: Instant,
    },
}

impl VideoDeliveryMethod {
    #[inline]
    pub fn new_stream() -> Self {
        Self::Stream {
            last_time_updated_samples: Instant::now(),
        }
    }
}

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

    /// Whether this is a finite video or a stream.
    pub delivery_method: VideoDeliveryMethod,

    /// A sorted list of all keyframe's sample indices in the video.
    pub keyframe_indices: Vec<SampleIndex>,

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
    pub samples: StableIndexDeque<SampleMetadataState>,

    /// Meta information about the samples.
    pub samples_statistics: SamplesStatistics,

    /// All the tracks in the mp4; not just the video track.
    ///
    /// Can be nice to show in a UI.
    pub mp4_tracks: BTreeMap<TrackId, Option<TrackKind>>,
}

impl re_byte_size::SizeBytes for VideoDataDescription {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            codec: _,
            encoding_details: _,
            timescale: _,
            delivery_method: _,
            keyframe_indices,
            samples,
            samples_statistics,
            mp4_tracks,
        } = self;

        keyframe_indices.heap_size_bytes()
            + samples.heap_size_bytes()
            + samples_statistics.heap_size_bytes()
            + mp4_tracks.len() as u64 * std::mem::size_of::<(TrackId, Option<TrackKind>)>() as u64
    }
}

impl VideoDataDescription {
    /// Get the group of pictures which use a keyframe, including the keyframe sample itself.
    pub fn gop_sample_range_for_keyframe(
        &self,
        keyframe_idx: usize,
    ) -> Option<std::ops::Range<SampleIndex>> {
        Some(
            *self.keyframe_indices.get(keyframe_idx)?
                ..self
                    .keyframe_indices
                    .get(keyframe_idx + 1)
                    .copied()
                    .unwrap_or_else(|| self.samples.next_index()),
        )
    }

    /// Checks various invariants that the video description should always uphold.
    ///
    /// Violation of any of these variants is **not** a user(-data) error, but instead an
    /// implementation bug of any code manipulating the video description.
    /// Vice versa, all code using `VideoDataDescription` can assume that these invariants hold.
    ///
    /// It's recommended to run these sanity check only in debug builds as they may be expensive for
    /// large videos.
    ///
    /// Check implementation for details.
    pub fn sanity_check(&self) -> Result<(), String> {
        self.sanity_check_keyframes()?;
        self.sanity_check_samples()?;

        // If an STSD box is present, then its content type must match with the internal codec.
        if let Some(stsd) = self.encoding_details.as_ref().and_then(|e| e.stsd.as_ref()) {
            let stsd_codec = match &stsd.contents {
                re_mp4::StsdBoxContent::Av01(_) => crate::VideoCodec::AV1,
                re_mp4::StsdBoxContent::Avc1(_) => crate::VideoCodec::H264,
                re_mp4::StsdBoxContent::Hvc1(_) | re_mp4::StsdBoxContent::Hev1(_) => {
                    crate::VideoCodec::H265
                }
                re_mp4::StsdBoxContent::Vp08(_) => crate::VideoCodec::VP8,
                re_mp4::StsdBoxContent::Vp09(_) => crate::VideoCodec::VP9,
                _ => {
                    return Err(format!(
                        "STSD box content type {:?} doesn't have a supported codec.",
                        stsd.contents
                    ));
                }
            };
            if stsd_codec != self.codec {
                return Err(format!(
                    "STSD box content type {:?} does not match with the internal codec {:?}.",
                    stsd.contents, self.codec
                ));
            }
        }

        Ok(())
    }

    fn sanity_check_keyframes(&self) -> Result<(), String> {
        if !self.keyframe_indices.is_sorted() {
            return Err("Keyframes aren't sorted".to_owned());
        }

        for &keyframe in &self.keyframe_indices {
            if keyframe < self.samples.min_index() {
                return Err(format!(
                    "Keyframe {keyframe} refers to sample to the left of the list of samples.",
                ));
            }

            if keyframe >= self.samples.next_index() {
                return Err(format!(
                    "Keyframe {keyframe} refers to sample to the right of the list of samples.",
                ));
            }

            match &self.samples[keyframe] {
                SampleMetadataState::Present(sample_metadata) => {
                    // All samples at the beginning of a GOP are marked with `is_sync==true`
                    if !sample_metadata.is_sync {
                        return Err(format!("Keyframe {keyframe} is not marked with `is_sync`."));
                    }
                }
                SampleMetadataState::Unloaded(_) => {
                    return Err(format!("Keyframe {keyframe} refers to an unloaded sample"));
                }
            }
        }

        // Make sure all keyframes are tracked.
        let mut keyframes = self.keyframe_indices.iter().copied();
        for (sample_idx, sample) in self
            .samples
            .iter_indexed()
            .filter_map(|(idx, s)| Some((idx, s.sample()?)))
        {
            if sample.is_sync && keyframes.next().is_none_or(|idx| idx != sample_idx) {
                return Err(format!("Not tracking the keyframe {sample_idx}."));
            }
        }
        Ok(())
    }

    fn sanity_check_samples(&self) -> Result<(), String> {
        // Decode timestamps are monotonically increasing.
        for (a, b) in self.samples.iter().tuple_windows() {
            if let SampleMetadataState::Present(a) = a
                && let SampleMetadataState::Present(b) = b
                && a.decode_timestamp > b.decode_timestamp
            {
                return Err(format!(
                    "Decode timestamps are not monotonically increasing: {:?} {:?}",
                    a.decode_timestamp, b.decode_timestamp
                ));
            }
        }

        // Sample statistics are consistent with the samples.
        let expected_statistics = SamplesStatistics::new(&self.samples);
        if expected_statistics != self.samples_statistics {
            return Err(format!(
                "Sample statistics are not consistent with the samples.\nExpected: {:?}\nActual: {:?}",
                expected_statistics, self.samples_statistics
            ));
        }

        Ok(())
    }

    /// Returns the encoded bytes for a sample in the format expected by [`VideoCodec`].
    ///
    /// * H.264/H.265: MP4 stores samples using AVCC/HVCC length-prefixed NALs and relies on container
    ///   metadata for SPS/PPS/VPS. This method makes sure to unpack this.
    /// * AV1 samples are stored as-is.
    /// * VP8/VP9: Not yet supported
    pub fn sample_data_in_stream_format(
        &self,
        chunk: &crate::Chunk,
    ) -> Result<Vec<u8>, SampleConversionError> {
        match self.codec {
            VideoCodec::AV1 => Ok(chunk.data.clone()),
            VideoCodec::H264 => {
                let stsd = self
                    .encoding_details
                    .as_ref()
                    .ok_or(SampleConversionError::MissingEncodingDetails(self.codec))?
                    .stsd
                    .as_ref()
                    .ok_or(SampleConversionError::MissingStsd(self.codec))?;

                let re_mp4::StsdBoxContent::Avc1(avc1_box) = &stsd.contents else {
                    return Err(SampleConversionError::UnexpectedStsdContent {
                        codec: self.codec,
                        found: format!("{:?}", stsd.contents),
                    });
                };

                let mut output = Vec::new();
                write_avc_chunk_to_annexb(avc1_box, &mut output, chunk.is_sync, chunk)
                    .map_err(SampleConversionError::AnnexB)?;
                Ok(output)
            }
            VideoCodec::H265 => {
                let stsd = self
                    .encoding_details
                    .as_ref()
                    .ok_or(SampleConversionError::MissingEncodingDetails(self.codec))?
                    .stsd
                    .as_ref()
                    .ok_or(SampleConversionError::MissingStsd(self.codec))?;

                let hvcc_box = match &stsd.contents {
                    re_mp4::StsdBoxContent::Hvc1(hvc1_box)
                    | re_mp4::StsdBoxContent::Hev1(hvc1_box) => hvc1_box,
                    other => {
                        return Err(SampleConversionError::UnexpectedStsdContent {
                            codec: self.codec,
                            found: format!("{other:?}"),
                        });
                    }
                };

                let mut output = Vec::new();
                write_hevc_chunk_to_annexb(hvcc_box, &mut output, chunk.is_sync, chunk)
                    .map_err(SampleConversionError::AnnexB)?;
                Ok(output)
            }
            VideoCodec::VP8 | VideoCodec::VP9 => {
                // TODO(#10186): Support VP8/VP9 for the `VideoStream` archetype
                Err(SampleConversionError::UnsupportedCodec(self.codec))
            }
        }
    }
}

/// Errors converting [`VideoDataDescription`] samples into the format expected by the decoder.
#[derive(thiserror::Error, Debug)]
pub enum SampleConversionError {
    #[error("Missing encoding details for codec {0:?}")]
    MissingEncodingDetails(VideoCodec),

    #[error("Missing stsd box for codec {0:?}")]
    MissingStsd(VideoCodec),

    #[error("Unexpected stsd contents for codec {codec:?}: {found}")]
    UnexpectedStsdContent { codec: VideoCodec, found: String },

    #[error("Failed converting sample to Annex-B: {0}")]
    AnnexB(#[from] AnnexBStreamWriteError),

    #[error("Unsupported codec {0:?}")]
    UnsupportedCodec(VideoCodec),
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

/// Meta information about the video samples.
#[derive(Clone, Debug, PartialEq, Eq)]
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

impl re_byte_size::SizeBytes for SamplesStatistics {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            dts_always_equal_pts: _,
            has_sample_highest_pts_so_far,
        } = self;
        has_sample_highest_pts_so_far
            .as_ref()
            .map_or(0, |bitvec| bitvec.capacity() as u64 / 8)
    }
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

    pub fn new(samples: &StableIndexDeque<SampleMetadataState>) -> Self {
        re_tracing::profile_function!();

        let dts_always_equal_pts = samples
            .iter()
            .filter_map(|s| s.sample())
            .all(|s| s.decode_timestamp == s.presentation_timestamp);

        let mut biggest_pts_so_far = Time::MIN;
        let has_sample_highest_pts_so_far = (!dts_always_equal_pts).then(|| {
            samples
                .iter()
                .map(move |sample| {
                    sample.sample().is_some_and(|sample| {
                        if sample.presentation_timestamp > biggest_pts_so_far {
                            biggest_pts_so_far = sample.presentation_timestamp;
                            true
                        } else {
                            false
                        }
                    })
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
    /// Does not copy any sample data, but instead stores offsets into the buffer.
    pub fn load_from_bytes(
        data: &[u8],
        media_type: &str,
        debug_name: &str,
        source_id: Tuid,
    ) -> Result<Self, VideoLoadError> {
        if data.is_empty() {
            return Err(VideoLoadError::ZeroBytes);
        }

        re_tracing::profile_function!();
        match media_type {
            "video/mp4" => Self::load_mp4(data, debug_name, source_id),

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
    ///
    /// Video containers and codecs like talking about samples or chunks rather than frames, but for how we define a chunk today,
    /// a frame is always a single chunk of data is always a single sample, see [`crate::decode::Chunk`].
    /// So for all practical purposes the sample count _is_ the number of frames, at least how we use it today.
    #[inline]
    pub fn num_samples(&self) -> usize {
        self.samples.num_elements()
    }

    /// Duration of all present samples.
    ///
    /// Returns `None` iff the video has no timescale.
    /// Other special cases like zero samples or single sample with unknown duration will return a zero duration.
    ///
    /// Since this is only about present samples and not historical, future or missing data,
    /// the duration may shrink as samples are dropped and grow as new samples are added.
    // TODO(andreas): This makes it somewhat unsuitable for various usecases in the viewer. We should probably accumulate the max duration somewhere.
    pub fn duration(&self) -> Option<std::time::Duration> {
        let timescale = self.timescale?;

        Some(match &self.delivery_method {
            VideoDeliveryMethod::Static { duration } => duration.duration(timescale),

            VideoDeliveryMethod::Stream { .. } => match self.samples.num_elements() {
                0 => std::time::Duration::ZERO,
                1 => {
                    let first = self.samples.iter().find_map(|s| s.sample())?;
                    first
                        .duration
                        .map(|d| d.duration(timescale))
                        .unwrap_or(std::time::Duration::ZERO)
                }
                _ => {
                    // TODO(#10090): This is only correct because there's no b-frames on streams right now.
                    // If there are b-frames determining the last timestamp is a bit more complicated.
                    let first = self.samples.iter().find_map(|s| s.sample())?;
                    let last = self.samples.iter().rev().find_map(|s| s.sample())?;

                    let last_sample_duration = last.duration.map_or_else(
                        || {
                            // Use average duration of all samples so far.
                            (last.presentation_timestamp - first.presentation_timestamp)
                                .duration(timescale)
                                / (last.frame_nr - first.frame_nr)
                        },
                        |d| d.duration(timescale),
                    );

                    (last.presentation_timestamp - first.presentation_timestamp).duration(timescale)
                        + last_sample_duration
                }
            },
        })
    }

    /// `num_frames / duration`.
    ///
    /// Note that the video could have a variable framerate!
    #[inline]
    pub fn average_fps(&self) -> Option<f32> {
        self.duration().map(|duration| {
            let num_frames = self.num_samples();

            // NOTE: the video duration includes the duration of the final frame too,
            // so we don't have a fence-post problem here!
            num_frames as f32 / duration.as_secs_f32()
        })
    }

    /// Determines the video timestamps of all present frames inside a video, returning raw time values.
    /// Reserved sample has no timestamp information and are thus ignored.
    ///
    /// Returns None if the video has no timescale.
    /// Returned timestamps are in nanoseconds since start and are guaranteed to be monotonically increasing.
    pub fn frame_timestamps_nanos(&self) -> Option<impl Iterator<Item = i64> + '_> {
        let timescale = self.timescale?;

        Some(
            self.samples
                .iter()
                .filter_map(|sample| Some(sample.sample()?.presentation_timestamp))
                .sorted()
                .map(move |pts| pts.into_nanos(timescale)),
        )
    }

    /// For a given decode (!) timestamp, returns the index of the first sample whose
    /// decode timestamp is lesser than or equal to the given timestamp.
    fn latest_sample_index_at_decode_timestamp(
        keyframes: &[KeyframeIndex],
        samples: &StableIndexDeque<SampleMetadataState>,
        decode_time: Time,
    ) -> Option<SampleIndex> {
        // First find what keyframe this decode timestamp is in, as an optimization since
        // we can't efficiently binary search the sample list with possible gaps.
        //
        // Keyframes will always be [`SampleMetadataState::Present`] and
        // have a decode timestamp we can compare against.
        let keyframe_idx = keyframes
            .partition_point(|p| {
                samples
                    .get(*p)
                    .map(|s| s.sample())
                    .inspect(|_s| {
                        debug_assert!(_s.is_some(), "Keyframes mentioned in the keyframe lookup list should always be loaded");
                    })
                    .flatten()
                    .is_some_and(|s| s.decode_timestamp <= decode_time)
            })
            .checked_sub(1)?;

        let start = *keyframes.get(keyframe_idx)?;
        let end = keyframes
            .get(keyframe_idx + 1)
            .copied()
            .unwrap_or_else(|| samples.next_index());

        // Within that keyframe's range, find the most suitable frame for the given decode time.
        let range = start..end;

        let mut found_sample_idx = None;
        for (idx, sample) in samples.iter_index_range_clamped(&range) {
            let Some(s) = sample.sample() else {
                continue;
            };

            if s.decode_timestamp <= decode_time {
                found_sample_idx = Some(idx);
            } else {
                break;
            }
        }

        found_sample_idx
    }

    /// See [`Self::latest_sample_index_at_presentation_timestamp`], split out for testing purposes.
    ///
    /// The returned sample index is guaranteed to be [`SampleMetadataState::Present`].
    fn latest_sample_index_at_presentation_timestamp_internal(
        keyframes: &[KeyframeIndex],
        samples: &StableIndexDeque<SampleMetadataState>,
        sample_statistics: &SamplesStatistics,
        presentation_timestamp: Time,
    ) -> Option<SampleIndex> {
        // Find the latest sample where `decode_timestamp <= presentation_timestamp`.
        // Because `decode <= presentation`, we never have to look further backwards in the
        // video than this.
        let decode_sample_idx = Self::latest_sample_index_at_decode_timestamp(
            keyframes,
            samples,
            presentation_timestamp,
        );

        let decode_sample_idx = decode_sample_idx?;

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
        for sample_idx in (samples.min_index()..=decode_sample_idx).rev() {
            let Some(sample) = samples[sample_idx].sample() else {
                continue;
            };

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
            &self.keyframe_indices,
            &self.samples,
            &self.samples_statistics,
            presentation_timestamp,
        )
    }

    /// Returns the sample presenteed directly prior to the given sample.
    ///
    /// Remember that samples are ordered in decode timestamp order,
    /// and that sample presented immediately prior to the given sample may have a higher decode timestamp.
    /// Therefore, this may be a jump on sample index.
    pub fn previous_presented_sample(&self, sample: &SampleMetadata) -> Option<&SampleMetadata> {
        let idx = Self::latest_sample_index_at_presentation_timestamp_internal(
            &self.keyframe_indices,
            &self.samples,
            &self.samples_statistics,
            sample.presentation_timestamp - Time::new(1),
        )?;
        match self.samples.get(idx) {
            Some(SampleMetadataState::Present(sample)) => Some(sample),
            None | Some(_) => unreachable!(),
        }
    }

    /// Returns the index of the keyframe for a specific sample.
    pub fn sample_keyframe_idx(&self, sample_idx: SampleIndex) -> Option<KeyframeIndex> {
        self.keyframe_indices
            .partition_point(|idx| *idx <= sample_idx)
            .checked_sub(1)
    }

    fn find_keyframe_index(
        &self,
        cmp_time: impl Fn(&SampleMetadata) -> bool,
    ) -> Option<KeyframeIndex> {
        self.keyframe_indices
            .partition_point(|sample_idx| {
                if let Some(sample) = self.samples[*sample_idx].sample() {
                    cmp_time(sample)
                } else {
                    debug_assert!(false, "[DEBUG]: keyframe indices should always be valid");

                    false
                }
            })
            .checked_sub(1)
    }

    /// For a given decode (!) timestamp, return the index of the keyframe index containing the given timestamp.
    pub fn decode_time_keyframe_index(&self, decode_time: Time) -> Option<KeyframeIndex> {
        self.find_keyframe_index(|t| t.decode_timestamp <= decode_time)
    }

    /// For a given presentation timestamp, return the index of the keyframe index containing the given timestamp.
    pub fn presentation_time_keyframe_index(&self, pts: Time) -> Option<KeyframeIndex> {
        self.find_keyframe_index(|t| t.presentation_timestamp <= pts)
    }
}

/// The state of the current sample.
///
/// When the source is loaded, all of its samples will be either `Present` or `Skip`.
#[derive(Debug, Clone)]
pub enum SampleMetadataState {
    /// Sample is present and contains video data.
    Present(SampleMetadata),

    /// The source for this sample hasn't arrived yet.
    Unloaded(Tuid),
}

impl SampleMetadataState {
    pub fn sample(&self) -> Option<&SampleMetadata> {
        match self {
            Self::Present(sample_metadata) => Some(sample_metadata),
            Self::Unloaded(_) => None,
        }
    }

    pub fn sample_mut(&mut self) -> Option<&mut SampleMetadata> {
        match self {
            Self::Present(sample_metadata) => Some(sample_metadata),
            Self::Unloaded(_) => None,
        }
    }

    pub fn source_id(&self) -> Tuid {
        match self {
            Self::Present(sample) => sample.source_id,
            Self::Unloaded(id) => *id,
        }
    }

    pub fn source_id_mut(&mut self) -> &mut Tuid {
        match self {
            Self::Present(sample) => &mut sample.source_id,
            Self::Unloaded(id) => id,
        }
    }
}

impl re_byte_size::SizeBytes for SampleMetadataState {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Present(sample_metadata) => sample_metadata.heap_size_bytes(),
            Self::Unloaded(c) => c.heap_size_bytes(),
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
pub struct SampleMetadata {
    /// Is this the start of a new (closed) group of pictures?
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

    /// The chunk this sample comes from.
    pub source_id: Tuid,

    /// Offset and length within a data buffer indicated by [`SampleMetadata::source_id`].
    pub byte_span: Span<u32>,
}

impl re_byte_size::SizeBytes for SampleMetadata {
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    fn is_pod() -> bool {
        true
    }
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
    pub fn get<'a>(
        &self,
        get_buffer: &dyn Fn(Tuid) -> &'a [u8],
        sample_idx: SampleIndex,
    ) -> Option<Chunk> {
        let buffer = get_buffer(self.source_id);
        let data = buffer.get(self.byte_span.range_usize())?.to_vec();

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
    #[error("The video file is empty (zero bytes)")]
    ZeroBytes,

    #[error("MP4 error: {0}")]
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

impl re_byte_size::SizeBytes for VideoLoadError {
    fn heap_size_bytes(&self) -> u64 {
        0 // close enough
    }
}

impl std::fmt::Debug for VideoDataDescription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Video")
            .field("codec", &self.codec)
            .field("encoding_details", &self.encoding_details)
            .field("timescale", &self.timescale)
            .field("keyframe_indices", &self.keyframe_indices)
            .field("samples", &self.samples.iter_indexed().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nalu::ANNEXB_NAL_START_CODE;

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
            .map(|(pts, dts)| {
                SampleMetadataState::Present(SampleMetadata {
                    is_sync: true,
                    frame_nr: 0, // unused
                    decode_timestamp: Time(dts),
                    presentation_timestamp: Time(pts),
                    duration: Some(Time(1)),
                    source_id: Tuid::new(),
                    byte_span: Default::default(),
                })
            })
            .collect::<StableIndexDeque<_>>();
        let keyframe_indices: Vec<SampleIndex> =
            (samples.min_index()..samples.next_index()).collect();

        let sample_statistics = SamplesStatistics::new(&samples);
        assert!(!sample_statistics.dts_always_equal_pts);

        // Test queries on the samples.
        let query_pts = |pts| {
            VideoDataDescription::latest_sample_index_at_presentation_timestamp_internal(
                &keyframe_indices,
                &samples,
                &sample_statistics,
                pts,
            )
        };

        // Check that query for all exact positions works as expected using brute force search as the reference.
        for (idx, sample) in samples.iter_indexed() {
            assert_eq!(
                Some(idx),
                query_pts(sample.sample().unwrap().presentation_timestamp)
            );
        }

        // Check that for slightly offsetted positions the query is still correct.
        // This works because for this dataset we know the minimum presentation timesetampe distance is always 256.
        for (idx, sample) in samples.iter_indexed() {
            assert_eq!(
                Some(idx),
                query_pts(sample.sample().unwrap().presentation_timestamp + Time(1))
            );
            assert_eq!(
                Some(idx),
                query_pts(sample.sample().unwrap().presentation_timestamp + Time(255))
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

    /// Helper function to check if data contains Annex B start codes
    fn has_annexb_start_codes(data: &[u8]) -> bool {
        data.windows(4).any(|w| w == ANNEXB_NAL_START_CODE)
    }

    fn video_test_file_mp4(codec: VideoCodec, need_dts_equal_pts: bool) -> std::path::PathBuf {
        let workspace_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .unwrap()
            .to_path_buf();

        let codec_str = match codec {
            VideoCodec::H264 => "h264",
            VideoCodec::H265 => "h265",
            VideoCodec::VP9 => "vp9",
            VideoCodec::VP8 => {
                panic!("We don't have test data for vp8, because Mp4 doesn't support vp8.")
            }
            VideoCodec::AV1 => "av1",
        };

        if need_dts_equal_pts && (codec == VideoCodec::H264 || codec == VideoCodec::H265) {
            // Only H264 and H265 have DTS != PTS when b-frames are present.
            workspace_dir.join(format!(
                "tests/assets/video/Big_Buck_Bunny_1080_1s_{codec_str}_nobframes.mp4",
            ))
        } else {
            workspace_dir.join(format!(
                "tests/assets/video/Big_Buck_Bunny_1080_1s_{codec_str}.mp4",
            ))
        }
    }

    /// Helper function to test video sampling for a specific codec
    fn test_video_codec_sampling(codec: VideoCodec, need_dts_equal_pts: bool) {
        let video_path = video_test_file_mp4(codec, need_dts_equal_pts);
        let data = std::fs::read(&video_path).unwrap();
        let video_data = VideoDataDescription::load_from_bytes(
            &data,
            "video/mp4",
            &format!("test_{codec:?}_video_sampling"),
            Tuid::new(),
        )
        .unwrap();

        let mut idr_count = 0;
        let mut non_idr_count = 0;

        for (sample_idx, sample) in video_data.samples.iter_indexed() {
            let chunk = sample
                .sample()
                .unwrap()
                .get(&|_| &data, sample_idx)
                .unwrap();
            let converted = video_data.sample_data_in_stream_format(&chunk).unwrap();

            if chunk.is_sync {
                idr_count += 1;

                // IDR frame should have SPS/PPS (only for H.264)
                if codec == VideoCodec::H264 {
                    let has_sps = converted
                        .windows(5)
                        .any(|w| w[0..4] == *ANNEXB_NAL_START_CODE && (w[4] & 0x1F) == 7);
                    assert!(has_sps, "IDR frame at index {sample_idx} should have SPS");
                }
            } else {
                non_idr_count += 1;
            }

            // All frames should have Annex B start codes (only for H.264/H.265)
            if codec == VideoCodec::H264 || codec == VideoCodec::H265 {
                assert!(
                    has_annexb_start_codes(&converted),
                    "Frame at index {sample_idx} should have Annex B start codes",
                );
            }
        }

        assert!(idr_count > 0, "Should have at least one IDR frame");
        assert!(non_idr_count > 0, "Should have at least one non-IDR frame");
    }

    #[test]
    fn test_full_video_sampling_all_codecs() {
        // TODO(#10186): Add VP9 once we have it.
        for codec in [VideoCodec::H264, VideoCodec::H265, VideoCodec::AV1] {
            test_video_codec_sampling(codec, false);
        }
    }
}
