use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use ahash::HashMap;
use serde::{Deserialize, Serialize};

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

pub struct LeRobotDataset {}

#[derive(Debug)]
pub struct LeRobotDatasetMetadata {
    pub info: LeRobotDatasetInfo,
    pub episodes: Vec<LeRobotDatasetEpisode>,
    pub tasks: Vec<LeRobotDatasetTask>,
}

impl LeRobotDatasetMetadata {
    pub fn load(root: impl AsRef<Path>) -> std::io::Result<Self> {
        let metadir = root.as_ref().join("meta");
        let info_filepath = metadir.join("info.json");
        let info_file = File::open(info_filepath)?;
        let reader = BufReader::new(info_file);

        let info: LeRobotDatasetInfo = serde_json::from_reader(reader)?;

        Ok(Self {
            info,
            episodes: Vec::new(),
            tasks: Vec::new(),
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

#[derive(Serialize, Deserialize, Debug)]
pub struct LeRobotDatasetEpisode {
    pub index: u32,
    pub tasks: Vec<String>,
    pub length: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LeRobotDatasetTask {
    pub task_index: u32,
    pub task: String,
}
