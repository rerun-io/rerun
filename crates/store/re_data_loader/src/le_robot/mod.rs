use std::f64::EPSILON;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::{error::Error, fs::File};

use ahash::HashMap;
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
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub struct LeRobotDataset {}

#[derive(Debug)]
pub struct LeRobotDatasetMetadata {
    pub info: LeRobotDatasetInfo,
    pub episodes: Vec<LeRobotDatasetEpisode>,
    pub tasks: Vec<LeRobotDatasetTask>,
}

impl LeRobotDatasetMetadata {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_directory(root: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let metadir = root.as_ref().join("meta");

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
    robot_type: String,
    total_episodes: u32,
    total_frames: u32,
    total_tasks: u32,
    total_videos: u32,
    total_chunks: u32,
    chunks_size: u32,
    fps: u32,
    features: HashMap<String, Feature>,
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
    pub episode_index: u32,
    pub tasks: Vec<String>,
    pub length: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LeRobotDatasetTask {
    pub task_index: u32,
    pub task: String,
}
