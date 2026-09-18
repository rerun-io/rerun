//! Error type for `LeRobot` dataset loading.

use crate::dataset::EpisodeIndex;
use crate::features::{DType, FeatureKey};
use crate::version::LeRobotDatasetVersion;

/// Errors that might happen when loading data from a `LeRobot` dataset.
#[derive(thiserror::Error, Debug)]
pub enum LeRobotError {
    #[error("Failed to read: {source}\nFile path: {path}")]
    IO {
        #[source]
        source: std::io::Error,
        path: std::path::PathBuf,
    },

    #[error("Failed to parse JSON: {source}\nFile path: {path}")]
    Json {
        #[source]
        source: serde_json::Error,
        path: std::path::PathBuf,
    },

    #[error("Failed to read a metadata table: {source}\nFile path: {path}")]
    MetadataRead {
        #[source]
        source: parquet::errors::ParquetError,
        path: std::path::PathBuf,
    },

    #[error("Failed to read data file footer: {source}\nFile path: {path}")]
    DataFileFooter {
        #[source]
        source: parquet::errors::ParquetError,
        path: std::path::PathBuf,
    },

    #[error(
        "The data file has neither a `frame_index` nor a `timestamp` column\nFile path: {path}"
    )]
    MissingTimeline { path: std::path::PathBuf },

    #[error("Invalid feature key: {0}")]
    InvalidFeatureKey(FeatureKey),

    #[error("Missing dataset info: {0}")]
    MissingDatasetInfo(String),

    #[error("Invalid dataset info: {0}")]
    InvalidDatasetInfo(String),

    #[error("The dataset contains no loadable episodes\nPath: {path}")]
    NoEpisodes { path: std::path::PathBuf },

    #[error("Invalid feature dtype, expected {key} to be of type {expected:?}, but got {actual:?}")]
    InvalidFeatureDtype {
        key: FeatureKey,
        expected: DType,
        actual: DType,
    },

    #[error(
        "The language feature `{column}` must hold a list of annotation rows, but its column type is `{datatype}`"
    )]
    InvalidLanguageColumn {
        column: String,
        datatype: arrow::datatypes::DataType,
    },

    #[error("Invalid chunk index: {0}")]
    InvalidChunkIndex(usize),

    #[error("Invalid episode index: {}", .0.0)]
    InvalidEpisodeIndex(EpisodeIndex),

    #[error("The `{dtype:?}` dtype is not supported for {version:?} datasets (feature: {key})")]
    UnsupportedFeatureDtype {
        key: FeatureKey,
        dtype: DType,
        version: LeRobotDatasetVersion,
    },

    #[error("Unsupported or unrecognized LeRobot dataset version\nPath: {path}")]
    UnsupportedVersion { path: std::path::PathBuf },

    /// Building a chunk from values we constructed ourselves failed — an internal
    /// invariant break, so call sites have no useful context to add.
    #[error(transparent)]
    Chunk(#[from] re_chunk::ChunkError),

    /// Serializing component values we constructed ourselves failed — an internal
    /// invariant break, so call sites have no useful context to add.
    #[error("Failed to build a component column: {0}")]
    Serialization(#[from] re_sdk_types::SerializationError),

    #[error("Failed to read episode data: {source}\nFile path: {path}")]
    EpisodeDataRead {
        #[source]
        source: re_parquet::ParquetError,
        path: std::path::PathBuf,
    },

    #[error("Failed to read video: {source}\nFile path: {path}")]
    Video {
        #[source]
        source: re_mp4_reader::Mp4Error,
        path: std::path::PathBuf,
    },

    #[error(
        "Streaming this video requires an ffmpeg transcode: {source}\n\
         Install ffmpeg, or use `VideoMode::Skip` to omit videos.\nFile path: {path}"
    )]
    VideoTranscode {
        #[source]
        source: re_mp4_reader::Mp4Error,
        path: std::path::PathBuf,
    },

    /// Building lenses from selectors and emits we constructed ourselves failed — an
    /// internal invariant break, so call sites have no useful context to add.
    #[error("Failed to build the feature lenses: {0}")]
    LensBuild(#[from] re_lenses::LensBuilderError),

    /// Applying the feature lenses failed for some columns.
    ///
    /// Carries the runtime errors as one formatted string rather than a typed source:
    /// one apply can fail with several errors, which a single `source()` chain cannot
    /// carry, and [`re_lenses::LensError`] must be consumed to release its partial
    /// chunk. Each formatted error names its entity and component.
    #[error("Failed to apply the feature lenses: {0}")]
    Lens(String),
}

impl LeRobotError {
    /// Create an IO error with the given source and path.
    pub fn io(source: std::io::Error, path: impl Into<std::path::PathBuf>) -> Self {
        Self::IO {
            source,
            path: path.into(),
        }
    }

    pub fn json(source: serde_json::Error, path: impl Into<std::path::PathBuf>) -> Self {
        Self::Json {
            source,
            path: path.into(),
        }
    }

    pub fn metadata_read(
        source: parquet::errors::ParquetError,
        path: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self::MetadataRead {
            source,
            path: path.into(),
        }
    }

    pub fn episode_data_read(
        source: re_parquet::ParquetError,
        path: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self::EpisodeDataRead {
            source,
            path: path.into(),
        }
    }
}
