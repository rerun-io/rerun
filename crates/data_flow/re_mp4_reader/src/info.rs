// no longer needed when https://github.com/rerun-io/quiver/pull/67 lands
#![expect(clippy::disallowed_methods)]

use itertools::{Itertools as _, MinMaxResult};
use quiver::{Column, DurationNanosecond};
use re_video::{VideoCodec, VideoDataDescription, VideoSource};

use crate::Mp4Error;

/// The samples of an mp4 video track: one row per sample, in decode order,
/// which is the order of the samples in the file.
///
/// The field names are the column names.
#[derive(Clone, Debug, quiver::Quiver)]
pub struct Mp4Samples {
    /// Index of the sample in presentation order.
    pub frame_nr: Column<i64>,

    /// Presentation time.
    ///
    /// Holds the same values as the timeline of a [`crate::Mode::Stream`] read
    /// that neither transcodes nor applies a time window.
    pub pts: Column<DurationNanosecond>,

    /// Decode time.
    ///
    /// Can be negative when samples are reordered for presentation.
    pub dts: Column<DurationNanosecond>,

    /// How long the sample is presented.
    pub duration: Column<DurationNanosecond>,

    /// Whether the sample starts a GOP.
    pub is_keyframe: Column<bool>,

    /// Offset of the sample's bytes in the file.
    pub byte_offset: Column<u64>,

    /// Length of the sample's bytes.
    pub byte_size: Column<u64>,
}

/// Facts about an mp4 file as stored, taken from its container metadata.
///
/// This describes the source file, not what [`crate::Mode::Stream`] emits:
/// a read that transcodes produces different samples, and its keyframes may differ.
#[derive(Clone, Debug)]
pub struct Mp4Info {
    /// Codec of the video track.
    pub codec: VideoCodec,

    /// Codec string as specified by the `WebCodecs` codec registry, e.g. `avc1.64001f`.
    pub codec_string: String,

    /// Width and height of the video in pixels.
    pub coded_dimensions: [u16; 2],

    /// Per color component bit depth, `None` if the metadata doesn't state it.
    pub bit_depth: Option<u8>,

    /// Number of samples, which is the number of rows of [`Self::samples`].
    pub num_frames: usize,

    /// Duration of the video track as declared by the container.
    ///
    /// Includes the time the last sample is presented, so it is longer than the span
    /// between the first and the last presentation time.
    /// It can be longer still when samples are reordered for presentation.
    pub duration_nanos: i64,

    /// Whether any sample's decode time differs from its presentation time.
    ///
    /// [`crate::Mode::Stream`] always transcodes such a file.
    pub has_b_frames: bool,

    /// Sample count of the smallest GOP, the last one included.
    ///
    /// Samples before the first keyframe belong to no GOP. `0` if there is no keyframe.
    pub gop_size_min: usize,

    /// Sample count of the largest GOP, the last one included.
    ///
    /// Samples before the first keyframe belong to no GOP. `0` if there is no keyframe.
    pub gop_size_max: usize,

    /// One row per sample, in decode order.
    pub samples: Mp4Samples,
}

impl Mp4Info {
    /// `desc` must come from the mp4 demuxer, which loads every sample together with
    /// its duration and its byte span.
    fn from_description(desc: &VideoDataDescription) -> Result<Self, Mp4Error> {
        re_tracing::profile_function!();

        let timescale = desc.timescale.ok_or(Mp4Error::NoTimescale)?;
        let duration = desc.duration().ok_or(Mp4Error::NoTimescale)?;
        let encoding_details = desc
            .encoding_details
            .as_ref()
            .ok_or(Mp4Error::NoEncodingDetails)?;

        let (gop_size_min, gop_size_max) = match (0..desc.keyframe_indices.len())
            .filter_map(|keyframe_idx| desc.gop_sample_range_for_keyframe(keyframe_idx))
            .map(|gop| gop.len)
            .minmax()
        {
            MinMaxResult::NoElements => (0, 0),
            MinMaxResult::OneElement(size) => (size, size),
            MinMaxResult::MinMax(min, max) => (min, max),
        };

        let samples = desc
            .samples
            .iter()
            .enumerate()
            .map(|(sample_idx, state)| {
                let missing = |what: &str| {
                    Mp4Error::SampleConversion(format!("sample {sample_idx} has no {what}"))
                };
                let sample = state.sample().ok_or_else(|| missing("metadata"))?;
                let duration = sample.duration.ok_or_else(|| missing("duration"))?;
                let VideoSource::Span(span) = sample.source else {
                    return Err(missing("byte span"));
                };
                Ok((sample, duration, span))
            })
            .collect::<Result<Vec<_>, Mp4Error>>()?;
        let samples = Mp4Samples {
            frame_nr: Mp4Samples::COLUMN_FRAME_NR.new_from_values(
                samples
                    .iter()
                    .map(|(sample, _, _)| i64::from(sample.frame_nr)),
            ),
            pts: Mp4Samples::COLUMN_PTS.new_from_values(
                samples
                    .iter()
                    .map(|(sample, _, _)| sample.presentation_timestamp.into_nanos(timescale)),
            ),
            dts: Mp4Samples::COLUMN_DTS.new_from_values(
                samples
                    .iter()
                    .map(|(sample, _, _)| sample.decode_timestamp.into_nanos(timescale)),
            ),
            duration: Mp4Samples::COLUMN_DURATION.new_from_values(
                samples
                    .iter()
                    .map(|(_, duration, _)| duration.into_nanos(timescale)),
            ),
            is_keyframe: Mp4Samples::COLUMN_IS_KEYFRAME
                .new_from_values(samples.iter().map(|(sample, _, _)| sample.is_sync)),
            byte_offset: Mp4Samples::COLUMN_BYTE_OFFSET
                .new_from_values(samples.iter().map(|(_, _, span)| span.start)),
            byte_size: Mp4Samples::COLUMN_BYTE_SIZE
                .new_from_values(samples.iter().map(|(_, _, span)| span.len)),
        };

        Ok(Self {
            codec: desc.codec.clone(),
            codec_string: encoding_details.codec_string.clone(),
            coded_dimensions: encoding_details.coded_dimensions,
            bit_depth: encoding_details.bit_depth,
            num_frames: samples.frame_nr.len(),
            duration_nanos: i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX),
            has_b_frames: !desc.samples_statistics.dts_always_equal_pts,
            gop_size_min,
            gop_size_max,
            samples,
        })
    }
}

/// Describe an mp4 file on disk without reading its sample data.
///
/// Only the container metadata is read, so the cost follows the size of the `moov` box
/// rather than the size of the file. VP8 and VP9 are the exception: their keyframe flags
/// are recovered from the bitstream, which reads every sample.
pub fn load_mp4_info(path: &std::path::Path) -> Result<Mp4Info, Mp4Error> {
    re_tracing::profile_function!();

    let file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut reader = std::io::BufReader::new(file);
    let desc =
        VideoDataDescription::load_mp4_from_reader(&mut reader, size, &path.display().to_string())?;
    Mp4Info::from_description(&desc)
}
