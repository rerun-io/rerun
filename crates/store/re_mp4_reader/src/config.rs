use re_chunk::TimePoint;
use re_log_types::{TimeType, TimelineName};
use re_video::Mp4TranscodeOptions;

/// Configuration for [`crate::load_mp4_from_bytes`].
#[derive(Clone, Debug)]
pub struct Mp4Config {
    /// What kind of chunks to produce.
    pub mode: Mode,

    /// Name of the timeline used for stream-mode samples and for the
    /// `VideoFrameReference` index chunk in asset mode.
    ///
    /// Defaults to `"video"`.
    pub timeline_name: TimelineName,

    /// How to interpret the timeline values. Applies to the stream-mode sample
    /// timeline and to the asset-mode `VideoFrameReference` index timeline.
    ///
    /// [`TimeType::DurationNs`] interprets the PTS as a duration since the start
    /// of the video (the mp4 default); [`TimeType::TimestampNs`] as wall-clock
    /// nanoseconds since the Unix epoch. The emitted values are identical either
    /// way — only the timeline's declared type differs. Pair `TimestampNs` with
    /// a downstream retag step that supplies the real wall-clock times.
    pub timeline_type: TimeType,
}

impl Default for Mp4Config {
    fn default() -> Self {
        Self {
            mode: Mode::Stream {
                chunk_by_gop: true,
                transcode: Mp4TranscodeOptions::default(),
            },
            timeline_name: "video".into(),
            timeline_type: TimeType::DurationNs,
        }
    }
}

/// Output mode for [`crate::load_mp4_from_bytes`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Emit an `AssetVideo` blob chunk plus a `VideoFrameReference` index chunk.
    ///
    /// `timepoint` is placed on the `AssetVideo` blob chunk. Public callers
    /// generally pass `TimePoint::default()` (static); the file importer passes
    /// its enriched timepoint with a zero-duration cell on the `video`
    /// timeline plus any `created_at` / `modified_at` cells.
    Asset { timepoint: TimePoint },

    /// Emit a static `VideoStream(codec=…)` chunk followed by per-sample
    /// `VideoSample` chunks at PTS.
    ///
    /// The timeline used for the samples is named [`Mp4Config::timeline_name`]
    /// and typed [`Mp4Config::timeline_type`].
    ///
    /// A source containing h264/h265 B-frames — or any source for which [`Mp4TranscodeOptions`]
    /// requests a transform (a different output codec, a GOP size) — is transcoded
    /// with ffmpeg into an equivalent B-frame-free stream before emission, because
    /// the `VideoStream` archetype cannot yet model differing DTS/PTS. Transcoding
    /// requires an `ffmpeg` executable.
    // TODO(#10090): emit B-frames directly once `VideoStream` can model DTS != PTS.
    Stream {
        /// Should the samples be grouped into one Rerun chunk per GOP?
        ///
        /// If `true`, groups samples into one Rerun chunk per GOP (keyframe through the sample just
        /// before the next keyframe). Otherwise, emits one Rerun chunk per sample.
        chunk_by_gop: bool,

        /// How to transcode the stream (output codec, GOP size, GPU acceleration).
        ///
        /// The default is a no-op: a B-frame-free source is read directly without
        /// invoking ffmpeg.
        transcode: Mp4TranscodeOptions,
    },
}
