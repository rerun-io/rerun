//! Pure parser for v2 (and v2.1) `LeRobot` datasets into the version-free [`LeRobotDataset`].
//!
//! # `LeRobot` v2 dataset format
//!
//! The dataset follows a standardized directory layout, typically organized as follows:
//!
//! ```text
//! .
//! ├── README.md
//! ├── data
//! │  └── chunk-000
//! │      ├── episode_000000.parquet
//! │      ├── episode_000001.parquet
//! │      ├── …
//! ├── meta
//! │  ├── episodes.jsonl
//! │  ├── info.json
//! │  ├── stats.json
//! │  └── tasks.jsonl
//! └── videos
//!     └── chunk-000
//!         └── observation.image
//!             ├── episode_000000.mp4
//!             ├── episode_000001.mp4
//!             ├── …
//! ```
//!
//! ## File layout
//!
//! - `data/`: Stores episode data in Parquet format, organized in chunks.
//! - `meta/`: Contains metadata files:
//!   - `info.json`: General dataset metadata (robot type, number of episodes, etc.).
//!   - `episodes.jsonl`: Episode-specific metadata (tasks, number of frames, etc.).
//!   - `tasks.jsonl`: Task definitions for episodes.
//!   - `stats.json`: Summary statistics of dataset features.
//! - `videos/`: Optional directory storing video observations for episodes, organized similarly to `data/`.
//!
//! Each episode is identified by a unique index and mapped to its corresponding chunk, based on the number of episodes
//! per chunk (which can be found in `meta/info.json`).

use crate::dataset::{EpisodeAddress, EpisodeIndex, LeRobotDataset, TaskIndex, Tasks, VideoSource};
use crate::error::LeRobotError;
use crate::features::{DType, Feature, FeatureKey};
use crate::version::LeRobotDatasetVersion;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use ahash::HashMap;
use itertools::Itertools as _;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Parse the v2 dataset at `path`.
///
/// Reads the `meta/` directory only, and eagerly resolves every episode's data and video
/// file addresses, so every parsed episode streams under a valid config.
pub fn parse(path: &Path) -> Result<LeRobotDataset, LeRobotError> {
    let metadata = LeRobotDatasetV2Metadata::load_from_directory(path.join("meta"))?;

    if let Some(key) = metadata
        .info
        .features
        .iter()
        .find_map(|(key, feature)| (feature.dtype == DType::Language).then_some(key))
    {
        return Err(LeRobotError::UnsupportedFeatureDtype {
            key: key.clone(),
            dtype: DType::Language,
            version: LeRobotDatasetVersion::V2,
        });
    }

    let video_keys: Vec<&FeatureKey> = metadata
        .info
        .features
        .iter()
        .filter_map(|(key, feature)| (feature.dtype == DType::Video).then_some(key))
        .collect();

    let mut episodes = BTreeMap::new();
    for &index in metadata.episodes.keys() {
        let mut videos = HashMap::default();
        for &key in &video_keys {
            let file = path.join(metadata.info.video_path(key, index)?);
            videos.insert(key.clone(), VideoSource::Asset { file });
        }
        episodes.insert(
            index,
            EpisodeAddress {
                data_file: path.join(metadata.info.episode_data_path(index)?),
                rows: None, // v2: one file per episode
                videos,
            },
        );
    }

    // v2 data files are not opened at parse time, so the feature list decides the timeline.
    let has_frame_index = metadata
        .info
        .features
        .keys()
        .any(|key| key.as_str() == crate::emits::FRAME_INDEX_COLUMN);

    let tasks = Tasks {
        tasks: metadata
            .tasks
            .into_iter()
            .map(|task| (task.index, task.task))
            .collect(),
        subtasks: HashMap::default(),
    };
    Ok(LeRobotDataset::new(
        path.to_path_buf(),
        LeRobotDatasetVersion::V2,
        metadata.info.features,
        episodes,
        tasks,
        f64::from(metadata.info.fps),
        has_frame_index,
    ))
}

/// Metadata for a v2 `LeRobot` dataset, as read from the files in its `meta` directory.
struct LeRobotDatasetV2Metadata {
    info: LeRobotDatasetV2Info,
    episodes: BTreeMap<EpisodeIndex, LeRobotDatasetV2Episode>,
    tasks: Vec<LeRobotDatasetV2Task>,
}

impl LeRobotDatasetV2Metadata {
    /// Loads all metadata files from the provided `meta/` directory.
    fn load_from_directory(metadir: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let metadir = metadir.as_ref();

        let info = LeRobotDatasetV2Info::load_from_json_file(metadir.join("info.json"))?;
        let episodes_vec: Vec<LeRobotDatasetV2Episode> =
            load_jsonl_file(metadir.join("episodes.jsonl"))?;
        let tasks = load_jsonl_file(metadir.join("tasks.jsonl"))?;

        // Key episodes by index; the ordered map makes every iteration ascending.
        let episodes = episodes_vec
            .into_iter()
            .map(|episode| (episode.index, episode))
            .collect::<BTreeMap<EpisodeIndex, LeRobotDatasetV2Episode>>();

        Ok(Self {
            info,
            episodes,
            tasks,
        })
    }
}

/// `LeRobot` dataset metadata, from `meta/info.json`.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LeRobotDatasetV2Info {
    /// The version of the `LeRobot` codebase the dataset was created for.
    codebase_version: String,

    /// The total number of unique episodes in the dataset.
    total_episodes: usize,

    /// The total number of unique chunks in the dataset.
    total_chunks: usize,

    /// The amount of episodes per chunk.
    ///
    /// This is used to determine the path to video and data files.
    chunks_size: usize,

    /// The path template for accessing episode data files.
    data_path: String,

    /// The path template for accessing video files for an episode.
    video_path: Option<String>,

    /// The frame rate of the recorded episode data.
    fps: f32,

    /// A mapping of feature names to their respective [`Feature`] definitions,
    /// ordered so the emitted chunks come out in a deterministic order.
    features: BTreeMap<FeatureKey, Feature>,
}

impl LeRobotDatasetV2Info {
    /// Loads `LeRobotDatasetInfo` from a JSON file.
    fn load_from_json_file(filepath: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let info_file = File::open(filepath.as_ref())
            .map_err(|err| LeRobotError::io(err, filepath.as_ref()))?;
        let reader = BufReader::new(info_file);

        serde_json::from_reader(reader).map_err(|err| LeRobotError::json(err, filepath.as_ref()))
    }

    /// Retrieve the metadata for a specific feature.
    fn feature(&self, feature_key: &str) -> Option<&Feature> {
        self.features.get(feature_key)
    }

    /// The storage chunk index holding this episode's files.
    fn chunk_index(&self, episode: EpisodeIndex) -> Result<usize, LeRobotError> {
        if episode.0 >= self.total_episodes {
            return Err(LeRobotError::InvalidEpisodeIndex(episode));
        }

        // chunk indices start at 0
        let chunk_idx = episode.0 / self.chunks_size;
        if chunk_idx < self.total_chunks {
            Ok(chunk_idx)
        } else {
            Err(LeRobotError::InvalidChunkIndex(chunk_idx))
        }
    }

    /// Generates the file path for a given episode's Parquet data.
    fn episode_data_path(&self, episode: EpisodeIndex) -> Result<PathBuf, LeRobotError> {
        let chunk = self.chunk_index(episode)?;

        // TODO(gijsd): Need a better way to handle this, as this only supports the default.
        Ok(self
            .data_path
            .replace("{episode_chunk:03d}", &format!("{chunk:03}"))
            .replace("{episode_index:06d}", &format!("{:06}", episode.0))
            .into())
    }

    /// Generates the file path for a video observation of a given episode.
    fn video_path(
        &self,
        feature_key: &FeatureKey,
        episode: EpisodeIndex,
    ) -> Result<PathBuf, LeRobotError> {
        let chunk = self.chunk_index(episode)?;
        let feature = self
            .feature(feature_key.as_str())
            .ok_or_else(|| LeRobotError::InvalidFeatureKey(feature_key.clone()))?;

        if feature.dtype != DType::Video {
            return Err(LeRobotError::InvalidFeatureDtype {
                key: feature_key.clone(),
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
                    .replace("{episode_index:06d}", &format!("{:06}", episode.0))
                    .replace("{video_key}", feature_key.as_str())
                    .into()
            })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_jsonl_file<D>(filepath: impl AsRef<Path>) -> Result<Vec<D>, LeRobotError>
where
    D: DeserializeOwned,
{
    let entries = std::fs::read_to_string(filepath.as_ref())
        .map_err(|err| LeRobotError::io(err, filepath.as_ref()))?
        .lines()
        .map(|line| {
            serde_json::from_str(line).map_err(|err| LeRobotError::json(err, filepath.as_ref()))
        })
        .try_collect()?;

    Ok(entries)
}

/// An episode in a `LeRobot` dataset.
///
/// Each episode contains its index, a list of associated tasks, and its total length in frames.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LeRobotDatasetV2Episode {
    #[serde(rename = "episode_index")]
    index: EpisodeIndex,
    tasks: Vec<String>,
    length: u32,
}

/// A task in a `LeRobot` dataset.
///
/// Each task consists of its index and a task description.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct LeRobotDatasetV2Task {
    #[serde(rename = "task_index")]
    index: TaskIndex,
    task: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a minimal v2 `meta/` directory whose only feature is a `language` one.
    fn write_meta_with_language_feature(root: &Path) {
        let metadir = root.join("meta");
        std::fs::create_dir_all(&metadir).unwrap();

        let mut features = BTreeMap::new();
        features.insert(
            FeatureKey::from("instruction"),
            Feature {
                dtype: DType::Language,
                shape: vec![1],
                names: None,
                info: None,
            },
        );
        let info = LeRobotDatasetV2Info {
            codebase_version: "v2.0".to_owned(),
            total_episodes: 1,
            total_chunks: 1,
            chunks_size: 1,
            data_path: "episode_000000.parquet".to_owned(),
            video_path: None,
            fps: 30.0,
            features,
        };

        std::fs::write(
            metadir.join("info.json"),
            serde_json::to_string(&info).unwrap(),
        )
        .unwrap();
        std::fs::write(
            metadir.join("episodes.jsonl"),
            r#"{"episode_index":0,"tasks":[],"length":3}"#,
        )
        .unwrap();
        std::fs::write(metadir.join("tasks.jsonl"), "").unwrap();
    }

    /// v2 has no language support, and the open contract is eager: the dataset must be
    /// rejected at parse time, not per episode.
    #[test]
    fn v2_language_dtype_is_rejected_at_parse() {
        let dir = tempfile::tempdir().unwrap();
        write_meta_with_language_feature(dir.path());

        let Err(err) = parse(dir.path()) else {
            panic!("v2 parsing must reject the `language` dtype")
        };

        assert!(
            err.to_string().to_lowercase().contains("language"),
            "expected a `language`-related error, got: {err}"
        );
    }
}
