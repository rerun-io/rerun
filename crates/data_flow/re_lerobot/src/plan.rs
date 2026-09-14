use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::RecordBatch;
use re_chunk::{TimeColumn, TimeInt, Timeline};

use crate::Feature;

/// Self-contained plan for one episode.
pub(crate) struct EpisodePlan {
    pub timeline: Timeline,

    /// On `timeline`, with one entry per row of `parquet_data`.
    pub time_column: TimeColumn,

    /// The episode's raw record batch, exactly as read from parquet.
    pub parquet_data: RecordBatch,

    /// Every `Scalar`, `Image`, and `DepthImage` key names a column of `parquet_data`;
    /// `Text` and `Video` carry their own resolved data and never read it.
    pub features: Vec<PlannedFeature>,
}

/// Resolved text rows for one entity: `(time, text)` pairs.
pub(crate) type TextRows = Vec<(TimeInt, String)>;

/// One feature of an episode, in the form [`crate::execute`] consumes it.
///
/// `key` is both the feature's column name in [`EpisodePlan::parquet_data`]
/// and its entity path.
pub(crate) enum PlannedFeature {
    Scalar {
        key: String,
        feature: Feature,
    },
    Image {
        key: String,
    },
    DepthImage {
        key: String,
    },

    /// Task, subtask, or natural-language instruction.
    Text {
        entity: String,
        rows: TextRows,
    },
    Video {
        entity: String,
        video: PlannedVideo,
    },
}

/// The two video shapes an episode can plan.
///
/// v2 stores one file per episode: the path is planned, the file is read whole at execute
/// time and logged as an `AssetVideo` asset. v3 stores files shared across episodes: the
/// bytes are resolved at plan time through the dataset's refcounted video cache, and the
/// episode's timestamp range is logged as a `VideoStream`.
pub(crate) enum PlannedVideo {
    Asset {
        file: PathBuf,
    },
    Stream {
        bytes: Arc<[u8]>,
        from_ts: f64,
        to_ts: f64,
    },
}
