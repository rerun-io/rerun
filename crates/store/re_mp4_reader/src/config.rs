use re_chunk::TimePoint;
use re_log_types::TimelineName;

/// Configuration for [`crate::load_mp4_from_bytes`].
#[derive(Clone, Debug)]
pub struct Mp4Config {
    /// What kind of chunks to produce.
    pub mode: Mode,

    /// Name of the timeline used for the `VideoFrameReference` index chunk in
    /// asset mode (and, in later versions of this crate, for stream-mode samples).
    ///
    /// Defaults to `"video"`.
    pub timeline_name: TimelineName,
}

impl Default for Mp4Config {
    fn default() -> Self {
        Self {
            mode: Mode::Asset {
                timepoint: TimePoint::default(),
            },
            timeline_name: "video".into(),
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
}
