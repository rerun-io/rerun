use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use ahash::HashMap;
use anyhow::Context;
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

        return subpath.is_dir();
    })
}

/// Errors that might happen when loading data through a [`super::LeRobotDataset`].
#[derive(thiserror::Error, Debug)]
pub enum LeRobotError {
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Parquet(#[from] parquet::errors::ParquetError),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

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

    pub fn read_episode_data(&self, episode_index: usize) -> Result<RecordBatch, LeRobotError> {
        if !self
            .metadata
            .episodes
            .iter()
            .any(|episode| episode.episode_index == episode_index)
        {
            return Err(LeRobotError::InvalidEpisodeIndex(episode_index));
        }

        let episode_data_file = self
            .path
            .join("data")
            .join("chunk-000") // TODO: when does this change?
            .join(format!("episode_{episode_index:0>6}.parquet"));

        let file = File::open(episode_data_file)?;
        let mut reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;

        reader
            .next()
            .transpose()
            .map(|batch| batch.ok_or(LeRobotError::EmptyEpisode(episode_index)))
            .map_err(LeRobotError::Arrow)?
    }

    pub fn read_episode_video_contents(
        &self,
        episode_index: usize,
    ) -> Result<std::borrow::Cow<'_, [u8]>, LeRobotError> {
        let videopath = self
            .path
            .join("videos")
            .join("chunk-000")
            .join("observation.image")
            .join(format!("episode_{episode_index:0>6}.mp4"));

        re_tracing::profile_function!(videopath.display().to_string());

        let contents = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(&videopath)?
            // TODO: Look into using anyhow again?
            // .with_context(|| format!("Failed to read file {videopath:?}"))?
        };

        Ok(std::borrow::Cow::Owned(contents))
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

        let info = LeRobotDatasetInfo::load_from_file(&metadir.join("info.json"))?;
        let episodes = load_jsonl_file(&metadir.join("episodes.jsonl"))?;
        let tasks = load_jsonl_file(&metadir.join("tasks.jsonl"))?;

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
    pub total_episodes: u32,
    pub total_frames: u32,
    pub total_tasks: u32,
    pub total_videos: u32,
    pub total_chunks: u32,
    pub chunks_size: u32,
    pub fps: u32,
    pub features: HashMap<String, Feature>,
}

impl LeRobotDatasetInfo {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_file(filepath: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let info_file = File::open(filepath)?;
        let reader = BufReader::new(info_file);

        serde_json::from_reader(reader).map_err(|err| err.into())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Feature {
    dtype: DType,
    shape: Vec<f32>,
    names: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DType {
    Video,
    Image,
    Float32,
    Float64,
    Int64,
}

// TODO: Do we want to stream in episodes or tasks?
#[cfg(not(target_arch = "wasm32"))]
fn load_jsonl_file<D>(filepath: impl AsRef<Path>) -> Result<Vec<D>, LeRobotError>
where
    D: DeserializeOwned,
{
    let entries = std::fs::read_to_string(filepath)?
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
