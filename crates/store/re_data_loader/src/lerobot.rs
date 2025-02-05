use std::borrow::Cow;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use ahash::HashMap;
use arrow::array::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Check whether the provided path contains a Le Robot dataset.
pub fn is_le_robot_dataset(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();

    if !path.is_dir() {
        return false;
    }

    ["meta", "data"].iter().all(|subdir| {
        let subpath = path.join(subdir);

        subpath.is_dir()
    })
}

/// Errors that might happen when loading data through a [`super::LeRobotDataset`].
#[derive(thiserror::Error, Debug)]
pub enum LeRobotError {
    #[error("IO error occurred on path: {1}")]
    IO(#[source] std::io::Error, std::path::PathBuf),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Parquet(#[from] parquet::errors::ParquetError),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Invalid feature key: {0}")]
    InvalidFeatureKey(String),

    #[error("Missing dataset info: {0}")]
    MissingDatasetInfo(String),

    #[error(
        "Invalid feature dtype, expected {key} to be of type {expected:?}, but got {actual:?}"
    )]
    InvalidFeatureDtype {
        key: String,
        expected: DType,
        actual: DType,
    },

    #[error("Invalid chunk index: {0}")]
    InvalidChunkIndex(usize),

    #[error("Invalid episode index: {0}")]
    InvalidEpisodeIndex(usize),

    #[error("Episode {0} data file does not contain any records")]
    EmptyEpisode(usize),
}

#[derive(Debug)]
pub struct LeRobotDataset {
    pub path: PathBuf,
    pub metadata: LeRobotDatasetMetadata,
}

impl LeRobotDataset {
    pub fn load_from_directory(path: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let path = path.as_ref();
        let metadata = LeRobotDatasetMetadata::load_from_directory(path.join("meta"))?;

        Ok(Self {
            path: path.to_path_buf(),
            metadata,
        })
    }

    /// Read the parquet file for the provided episode index.
    pub fn read_episode_data(&self, episode_index: usize) -> Result<RecordBatch, LeRobotError> {
        if !self
            .metadata
            .episodes
            .iter()
            .any(|episode| episode.episode_index == episode_index)
        {
            return Err(LeRobotError::InvalidEpisodeIndex(episode_index));
        }

        let episode_data_path = self.metadata.info.episode_data_path(episode_index)?;
        let episode_parquet_file = self.path.join(episode_data_path);

        let file = File::open(&episode_parquet_file)
            .map_err(|err| LeRobotError::IO(err, episode_parquet_file))?;
        let mut reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;

        reader
            .next()
            .transpose()
            .map(|batch| batch.ok_or(LeRobotError::EmptyEpisode(episode_index)))
            .map_err(LeRobotError::Arrow)?
    }

    /// Read video feature for the provided episode.
    pub fn read_episode_video_contents(
        &self,
        observation_key: &str,
        episode_index: usize,
    ) -> Result<Cow<'_, [u8]>, LeRobotError> {
        let video_file = self
            .metadata
            .info
            .video_path(observation_key, episode_index)?;

        let videopath = self.path.join(video_file);

        let contents = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(&videopath).map_err(|err| LeRobotError::IO(err, videopath))?
        };

        Ok(Cow::Owned(contents))
    }
}

#[derive(Debug)]
pub struct LeRobotDatasetMetadata {
    pub info: LeRobotDatasetInfo,
    pub episodes: Vec<LeRobotDatasetEpisode>,
    pub tasks: Vec<LeRobotDatasetTask>,
}

impl LeRobotDatasetMetadata {
    pub fn load_from_directory(metadir: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let metadir = metadir.as_ref();

        let info = LeRobotDatasetInfo::load_from_file(metadir.join("info.json"))?;
        let episodes = load_jsonl_file(metadir.join("episodes.jsonl"))?;
        let tasks = load_jsonl_file(metadir.join("tasks.jsonl"))?;

        Ok(Self {
            info,
            episodes,
            tasks,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LeRobotDatasetInfo {
    pub robot_type: String,
    pub codebase_version: String,
    pub total_episodes: usize,
    pub total_frames: usize,
    pub total_tasks: usize,
    pub total_videos: usize,
    pub total_chunks: usize,
    pub chunks_size: usize,
    pub data_path: String,
    pub video_path: Option<String>,
    pub image_path: Option<String>,
    pub fps: usize,
    pub features: HashMap<String, Feature>,
}

impl LeRobotDatasetInfo {
    pub fn load_from_file(filepath: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let info_file = File::open(filepath.as_ref())
            .map_err(|err| LeRobotError::IO(err, filepath.as_ref().to_owned()))?;
        let reader = BufReader::new(info_file);

        serde_json::from_reader(reader).map_err(|err| err.into())
    }

    pub fn feature(&self, feature_key: &str) -> Option<&Feature> {
        self.features.get(feature_key)
    }

    pub fn chunk_index(&self, episode_index: usize) -> Result<usize, LeRobotError> {
        if episode_index > self.total_episodes {
            return Err(LeRobotError::InvalidEpisodeIndex(episode_index));
        }

        // chunk indices start at 0
        let chunk_idx = episode_index / self.chunks_size;
        if chunk_idx < self.total_chunks {
            Ok(chunk_idx)
        } else {
            Err(LeRobotError::InvalidChunkIndex(chunk_idx))
        }
    }

    pub fn episode_data_path(&self, episode_index: usize) -> Result<PathBuf, LeRobotError> {
        let chunk = self.chunk_index(episode_index)?;

        // TODO(gijsd): Need a better way to handle this, as this only supports the default.
        Ok(self
            .data_path
            .replace("{episode_chunk:03d}", &format!("{chunk:03}"))
            .replace("{episode_index:06d}", &format!("{episode_index:06}"))
            .into())
    }

    /// Get the path to a video observation for a specific episode index.
    pub fn video_path(
        &self,
        observation_key: &str,
        episode_index: usize,
    ) -> Result<PathBuf, LeRobotError> {
        let chunk = self.chunk_index(episode_index)?;
        let feature = self
            .feature(observation_key)
            .ok_or(LeRobotError::InvalidFeatureKey(observation_key.to_owned()))?;

        if feature.dtype != DType::Video {
            return Err(LeRobotError::InvalidFeatureDtype {
                key: observation_key.to_owned(),
                expected: DType::Video,
                actual: feature.dtype,
            });
        }

        // TODO(gijsd): Need a better way to handle this, as this only supports the default.
        self.video_path
            .as_ref()
            .ok_or_else(|| LeRobotError::MissingDatasetInfo("video_path".to_owned()))
            .map(|path| {
                path.replace("{episode_chunk:03d}", &format!("{chunk:03}"))
                    .replace("{episode_index:06d}", &format!("{episode_index:06}"))
                    .replace("{video_key}", observation_key)
                    .into()
            })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Feature {
    pub dtype: DType,
    pub shape: Vec<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DType {
    Video,
    Image,
    Bool,
    Float32,
    Float64,
    Int64,
}

// TODO(gijsd): Do we want to stream in episodes or tasks?
#[cfg(not(target_arch = "wasm32"))]
fn load_jsonl_file<D>(filepath: impl AsRef<Path>) -> Result<Vec<D>, LeRobotError>
where
    D: DeserializeOwned,
{
    let entries = std::fs::read_to_string(filepath.as_ref())
        .map_err(|err| LeRobotError::IO(err, filepath.as_ref().to_owned()))?
        .lines()
        .map(|line| serde_json::from_str(line))
        .collect::<Result<Vec<D>, _>>()?;

    Ok(entries)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LeRobotDatasetEpisode {
    pub episode_index: usize,
    pub tasks: Vec<String>,
    pub length: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LeRobotDatasetTask {
    pub task_index: usize,
    pub task: String,
}
