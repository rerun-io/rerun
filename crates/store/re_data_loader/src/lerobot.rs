//! A module for loading and working with `LeRobot` datasets.
//!
//! This module provides functionality to identify and parse `LeRobot` datasets,
//! which consist of metadata and episode data stored in a structured format.
//!
//! # Important
//!
//! Currently this only supports v2 `LeRobot` datasets!
//!
//! See [`LeRobotDataset`] for more information on the dataset format.

use std::borrow::Cow;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use ahash::HashMap;
use arrow::array::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::de::{DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

/// Check whether the provided path contains a `LeRobot` dataset.
pub fn is_lerobot_dataset(path: impl AsRef<Path>) -> bool {
    is_v1_lerobot_dataset(path.as_ref()) || is_v2_lerobot_dataset(path.as_ref())
}

/// Check whether the provided path contains a v2 `LeRobot` dataset.
pub fn is_v2_lerobot_dataset(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();

    if !path.is_dir() {
        return false;
    }

    // v2 `LeRobot` datasets store the metadata in a `meta` directory,
    // instead of the `meta_data` directory used in v1 datasets.
    has_sub_directories(&["meta", "data"], path)
}

/// Check whether the provided path contains a v1 `LeRobot` dataset.
pub fn is_v1_lerobot_dataset(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();

    if !path.is_dir() {
        return false;
    }

    // v1 `LeRobot` datasets stored the metadata in a `meta_data` directory,
    // instead of the `meta` directory used in v2 datasets.
    has_sub_directories(&["meta_data", "data"], path)
}

fn has_sub_directories(directories: &[&str], path: impl AsRef<Path>) -> bool {
    directories.iter().all(|subdir| {
        let subpath = path.as_ref().join(subdir);

        // check that the sub directory exists and is not empty
        subpath.is_dir()
            && subpath
                .read_dir()
                .is_ok_and(|mut contents| contents.next().is_some())
    })
}

/// Errors that might happen when loading data through a [`crate::loader_lerobot::LeRobotDatasetLoader`].
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

    #[error("Invalid feature dtype, expected {key} to be of type {expected:?}, but got {actual:?}")]
    InvalidFeatureDtype {
        key: String,
        expected: DType,
        actual: DType,
    },

    #[error("Invalid chunk index: {0}")]
    InvalidChunkIndex(usize),

    #[error("Invalid episode index: {0:?}")]
    InvalidEpisodeIndex(EpisodeIndex),

    #[error("Episode {0:?} data file does not contain any records")]
    EmptyEpisode(EpisodeIndex),
}

/// A `LeRobot` dataset consists of structured metadata and recorded episode data stored in
/// Parquet files.
///
/// # `LeRobot` Dataset Format
///
/// The dataset follows a standardized directory layout, typically organized as follows:
///
/// ```text
/// .
/// ├── README.md
/// ├── data
/// │  └── chunk-000
/// │      ├── episode_000000.parquet
/// │      ├── episode_000001.parquet
/// │      ├── …
/// ├── meta
/// │  ├── episodes.jsonl
/// │  ├── info.json
/// │  ├── stats.json
/// │  └── tasks.jsonl
/// └── videos
///     └── chunk-000
///         └── observation.image
///             ├── episode_000000.mp4
///             ├── episode_000001.mp4
///             ├── …
/// ```
///
/// ## File layout
///
/// - `data/`: Stores episode data in Parquet format, organized in chunks.
/// - `meta/`: Contains metadata files:
///   - `info.json`: General dataset metadata (robot type, number of episodes, etc.).
///   - `episodes.jsonl`: Episode-specific metadata (tasks, number of frames, etc.).
///   - `tasks.jsonl`: Task definitions for episodes.
///   - `stats.json`: Summary statistics of dataset features.
/// - `videos/`: Optional directory storing video observations for episodes, organized similarly to `data/`.
///
/// Each episode is identified by a unique index and mapped to its corresponding chunk, based on the number of episodes
/// per chunk (which can be found in `meta/info.json`).
#[derive(Debug, Clone)]
pub struct LeRobotDataset {
    pub path: PathBuf,
    pub metadata: LeRobotDatasetMetadata,
}

impl LeRobotDataset {
    /// Loads a `LeRobotDataset` from a directory.
    ///
    /// This method initializes a dataset by reading its metadata from the `meta/` directory.
    ///
    /// # Important
    ///
    /// Currently, this only supports v2 `LeRobot` datasets.
    pub fn load_from_directory(path: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let path = path.as_ref();
        let metadatapath = path.join("meta");
        let metadata = LeRobotDatasetMetadata::load_from_directory(&metadatapath)?;

        Ok(Self {
            path: path.to_path_buf(),
            metadata,
        })
    }

    /// Read the Parquet data file for the provided episode.
    pub fn read_episode_data(&self, episode: EpisodeIndex) -> Result<RecordBatch, LeRobotError> {
        if self.metadata.episodes.get(episode.0).is_none() {
            return Err(LeRobotError::InvalidEpisodeIndex(episode));
        };

        let episode_data_path = self.metadata.info.episode_data_path(episode)?;
        let episode_parquet_file = self.path.join(episode_data_path);

        let file = File::open(&episode_parquet_file)
            .map_err(|err| LeRobotError::IO(err, episode_parquet_file))?;
        let mut reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;

        reader
            .next()
            .transpose()
            .map(|batch| batch.ok_or(LeRobotError::EmptyEpisode(episode)))
            .map_err(LeRobotError::Arrow)?
    }

    /// Read video feature for the provided episode.
    pub fn read_episode_video_contents(
        &self,
        observation_key: &str,
        episode: EpisodeIndex,
    ) -> Result<Cow<'_, [u8]>, LeRobotError> {
        let video_file = self.metadata.info.video_path(observation_key, episode)?;

        let videopath = self.path.join(video_file);

        let contents = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(&videopath).map_err(|err| LeRobotError::IO(err, videopath))?
        };

        Ok(Cow::Owned(contents))
    }

    /// Retrieve the task using the provided task index.
    pub fn task_by_index(&self, task: TaskIndex) -> Option<&LeRobotDatasetTask> {
        self.metadata.tasks.get(task.0)
    }
}

/// Metadata for a `LeRobot` dataset.
///
/// This is a wrapper struct for the metadata files in the `meta` directory of a
/// `LeRobot` dataset. For more see [`LeRobotDataset`].
#[derive(Debug, Clone)]
#[allow(dead_code)] // TODO(gijsd): The list of tasks is not used yet!
pub struct LeRobotDatasetMetadata {
    pub info: LeRobotDatasetInfo,
    pub episodes: Vec<LeRobotDatasetEpisode>,
    pub tasks: Vec<LeRobotDatasetTask>,
}

impl LeRobotDatasetMetadata {
    /// Loads all metadata files from the provided directory.
    ///
    /// This method reads dataset metadata from JSON and JSONL files stored in the `meta/` directory.
    /// It retrieves general dataset information, a list of recorded episodes, and defined tasks.
    pub fn load_from_directory(metadir: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let metadir = metadir.as_ref();

        let info = LeRobotDatasetInfo::load_from_json_file(metadir.join("info.json"))?;
        let mut episodes = load_jsonl_file(metadir.join("episodes.jsonl"))?;
        let mut tasks = load_jsonl_file(metadir.join("tasks.jsonl"))?;

        episodes.sort_by_key(|e: &LeRobotDatasetEpisode| e.index);
        tasks.sort_by_key(|e: &LeRobotDatasetTask| e.index);

        Ok(Self {
            info,
            episodes,
            tasks,
        })
    }
}

/// `LeRobot` dataset metadata.
///
/// This struct contains the metadata for a `LeRobot` dataset, and is loaded from the `meta/info.json` file
/// of the dataset.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LeRobotDatasetInfo {
    /// The type of the robot.
    pub robot_type: String,

    /// The version of the `LeRobot` codebase the dataset was created for.
    pub codebase_version: String,

    /// The total number of unique episodes in the dataset.
    pub total_episodes: usize,

    /// The total number of unique frames in the dataset.
    pub total_frames: usize,

    /// The total number of unique tasks in the dataset.
    pub total_tasks: usize,

    /// The total amount of videos in the dataset.
    pub total_videos: usize,

    /// The total number of unique chunks in the dataset.
    pub total_chunks: usize,

    /// The amount of episodes per chunk.
    ///
    /// This is used to determine the path to video and data files.
    pub chunks_size: usize,

    /// The path template for accessing episode data files.
    pub data_path: String,

    /// The path template for accessing video files for an episode.
    pub video_path: Option<String>,

    /// The path template for accessing image files for an episode.
    pub image_path: Option<String>,

    /// The frame rate of the recorded episode data.
    pub fps: usize,

    /// A mapping of feature names to their respective [`Feature`] definitions.
    pub features: HashMap<String, Feature>,
}

impl LeRobotDatasetInfo {
    /// Loads `LeRobotDatasetInfo` from a JSON file.
    ///
    /// The `LeRobot` dataset info file is typically stored under `meta/info.json`.
    pub fn load_from_json_file(filepath: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let info_file = File::open(filepath.as_ref())
            .map_err(|err| LeRobotError::IO(err, filepath.as_ref().to_owned()))?;
        let reader = BufReader::new(info_file);

        serde_json::from_reader(reader).map_err(|err| err.into())
    }

    /// Retrieve the metadata for a specific feature.
    pub fn feature(&self, feature_key: &str) -> Option<&Feature> {
        self.features.get(feature_key)
    }

    /// Computes the storage chunk index for a given episode.
    ///
    /// Episodes are organized into chunks to optimize storage and retrieval. This method determines
    /// which chunk a specific episode belongs to based on the dataset's chunk size.
    pub fn chunk_index(&self, episode: EpisodeIndex) -> Result<usize, LeRobotError> {
        if episode.0 > self.total_episodes {
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
    pub fn episode_data_path(&self, episode: EpisodeIndex) -> Result<PathBuf, LeRobotError> {
        let chunk = self.chunk_index(episode)?;

        // TODO(gijsd): Need a better way to handle this, as this only supports the default.
        Ok(self
            .data_path
            .replace("{episode_chunk:03d}", &format!("{chunk:03}"))
            .replace("{episode_index:06d}", &format!("{:06}", episode.0))
            .into())
    }

    /// Generates the file path for a video observation of a given episode.
    pub fn video_path(
        &self,
        feature_key: &str,
        episode: EpisodeIndex,
    ) -> Result<PathBuf, LeRobotError> {
        let chunk = self.chunk_index(episode)?;
        let feature = self
            .feature(feature_key)
            .ok_or(LeRobotError::InvalidFeatureKey(feature_key.to_owned()))?;

        if feature.dtype != DType::Video {
            return Err(LeRobotError::InvalidFeatureDtype {
                key: feature_key.to_owned(),
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
                    .replace("{video_key}", feature_key)
                    .into()
            })
    }
}

/// Feature definition for a `LeRobot` dataset.
///
/// Each feature represents a data stream recorded during an episode, of a specific data type (`dtype`)
/// and dimensionality (`shape`).
///
/// For example, a shape of `[3, 224, 224]` for a [`DType::Image`] feature denotes a 3-channel (e.g. RGB)
/// image with a height and width of 224 pixels each.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Feature {
    pub dtype: DType,
    pub shape: Vec<usize>,
    pub names: Option<Names>,
}

impl Feature {
    /// Get the channel dimension for this [`Feature`].
    ///
    /// Returns the number of channels in the feature's data representation.
    ///
    /// # Note
    ///
    /// This is primarily intended for [`DType::Image`] and [`DType::Video`] features,
    /// where it represents color channels (e.g., 3 for RGB, 4 for RGBA).
    /// For other feature types, this function returns the size of the last dimension
    /// from the feature's shape.
    pub fn channel_dim(&self) -> usize {
        // first check if there's a "channels" name, if there is we can use that index.
        if let Some(names) = &self.names {
            if let Some(channel_idx) = names.0.iter().position(|name| name == "channels") {
                // If channel_idx is within bounds of shape, return that dimension
                if channel_idx < self.shape.len() {
                    return self.shape[channel_idx];
                }
            }
        }

        // Default to the last dimension if no channels name is found
        // or if the found index is out of bounds
        self.shape.last().copied().unwrap_or(0)
    }
}

/// Data types supported for features in a `LeRobot` dataset.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DType {
    Video,
    Image,
    Bool,
    Float32,
    Float64,
    Int16,
    Int64,
    String,
}

/// Name metadata for a feature in the `LeRobot` dataset.
///
/// The name metadata can consist of
/// - A flat list of names for each dimension of a feature (e.g., `["height", "width", "channel"]`).
/// - A nested list of names for each dimension of a feature (e.g., `[[""kLeftShoulderPitch", "kLeftShoulderRoll"]]`)
/// - A map with a string array value (e.g., `{ "motors": ["motor_0", "motor_1", ...] }` or `{ "axes": ["x", "y", "z"] }`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Names(Vec<String>);

impl Names {
    /// Retrieves the name corresponding to a specific index.
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn name_for_index(&self, index: usize) -> Option<&String> {
        self.0.get(index)
    }
}

/// Visitor implementation for deserializing the [`Names`] type.
///
/// Handles multiple representation formats:
/// - Flat string arrays: `["x", "y", "z"]`
/// - Nested string arrays: `[["motor_1", "motor_2"]]`
/// - Single-entry objects: `{"motors": ["motor_1", "motor_2"]}` or `{"axes": null}`
///
/// See the `Names` type documentation for more details on the supported formats.
struct NamesVisitor;

impl<'de> Visitor<'de> for NamesVisitor {
    type Value = Names;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "a flat string array, a nested string array, or a single-entry object with a string array or null value",
        )
    }

    /// Handle sequences:
    /// - Flat string arrays: `["x", "y", "z"]`
    /// - Nested string arrays: `[["motor_1", "motor_2"]]`
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        // Helper enum to deserialize sequence elements
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ListItem {
            Str(String),
            List(Vec<String>),
        }

        /// Enum to track the list type
        #[derive(PartialEq)]
        enum ListType {
            Undetermined,
            Flat,
            Nested,
        }

        let mut names = Vec::new();
        let mut determined_type = ListType::Undetermined;

        while let Some(item) = seq.next_element::<ListItem>()? {
            match item {
                ListItem::Str(s) => {
                    if determined_type == ListType::Nested {
                        return Err(serde::de::Error::custom(
                            "Cannot mix nested lists with flat strings within names array",
                        ));
                    }
                    determined_type = ListType::Flat;
                    names.push(s);
                }
                ListItem::List(list) => {
                    if determined_type == ListType::Flat {
                        return Err(serde::de::Error::custom(
                            "Cannot mix flat strings and nested lists within names array",
                        ));
                    }
                    determined_type = ListType::Nested;

                    // Flatten the nested list
                    names.extend(list);
                }
            }
        }

        Ok(Names(names))
    }

    /// Handle single-entry objects: `{"motors": ["motor_1", "motor_2"]}` or `{"axes": null}`
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut names_vec: Option<Vec<String>> = None;
        let mut entry_count = 0;

        // We expect exactly one entry.
        while let Some((_key, value)) = map.next_entry::<String, Option<Vec<String>>>()? {
            entry_count += 1;
            if entry_count > 1 {
                // Consume remaining entries to be a good citizen before erroring
                while map
                    .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
                    .is_some()
                {}

                return Err(serde::de::Error::invalid_length(
                    entry_count,
                    &"a Names object with exactly one entry.",
                ));
            }

            names_vec = Some(value.unwrap_or_default());
        }

        Ok(Names(names_vec.unwrap_or_default()))
    }
}

impl<'de> Deserialize<'de> for Names {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(NamesVisitor)
    }
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

/// Newtype wrapper for episode indices.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct EpisodeIndex(pub usize);

/// An episode in a `LeRobot` dataset.
///
/// Each episode contains its index, a list of associated tasks, and its total length in frames.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeRobotDatasetEpisode {
    #[serde(rename = "episode_index")]
    pub index: EpisodeIndex,
    pub tasks: Vec<String>,
    pub length: u32,
}

/// Newtype wrapper for task indices.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct TaskIndex(pub usize);

/// A task in a `LeRobot` dataset.
///
/// Each task consists of its index and a task description.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LeRobotDatasetTask {
    #[serde(rename = "task_index")]
    pub index: TaskIndex,
    pub task: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_deserialize_flat_list() {
        let json = r#"["a", "b", "c"]"#;
        let expected = Names(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_nested_list() {
        let json = r#"[["a", "b"], ["c"]]"#;
        let expected = Names(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_empty_nested_list() {
        let json = r#"[[], []]"#;
        let expected = Names(vec![]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_empty_list() {
        let json = r#"[]"#;
        let expected = Names(vec![]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_object_with_list() {
        let json = r#"{ "axes": ["x", "y", "z"] }"#;
        let expected = Names(vec!["x".to_owned(), "y".to_owned(), "z".to_owned()]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_object_with_empty_list() {
        let json = r#"{ "motors": [] }"#;
        let expected = Names(vec![]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_object_with_null() {
        let json = r#"{ "axes": null }"#;
        let expected = Names(vec![]); // Null results in an empty list
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_empty_object() {
        // Empty object results in empty list.
        let json = r#"{}"#;
        let expected = Names(vec![]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_error_mixed_list() {
        let json = r#"["a", ["b"]]"#; // Mixed flat and nested
        let result: Result<Names, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot mix flat strings and nested lists")
        );
    }

    #[test]
    fn test_deserialize_error_object_multiple_entries() {
        let json = r#"{ "axes": ["x"], "motors": ["m"] }"#;
        let result: Result<Names, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("a Names object with exactly one entry")
        );
    }
}
