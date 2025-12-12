//! A module for loading and working with `LeRobot` datasets.
//!
//! This module provides functionality to identify and parse `LeRobot` datasets,
//! which consist of metadata and episode data stored in a structured format.
//!
//! # Important
//!
//! This module supports v2 and v3 `LeRobot` datasets!
//!
//! See [`datasetv2::LeRobotDatasetV2`] and [`datasetv3::LeRobotDatasetV3`] for more information on the dataset formats.
pub mod common;
pub mod datasetv2;
pub mod datasetv3;

use std::{fmt, path::Path};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, SeqAccess, Visitor},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeRobotDatasetVersion {
    V1,
    V2,
    V3,
}

impl LeRobotDatasetVersion {
    pub fn find_version(path: impl AsRef<Path>) -> Option<Self> {
        let path = path.as_ref();

        if is_v3_lerobot_dataset(path) {
            Some(Self::V3)
        } else if is_v2_lerobot_dataset(path) {
            Some(Self::V2)
        } else if is_v1_lerobot_dataset(path) {
            Some(Self::V1)
        } else {
            None
        }
    }
}

/// Check whether the provided path contains a `LeRobot` dataset.
pub fn is_lerobot_dataset(path: impl AsRef<Path>) -> bool {
    is_v1_lerobot_dataset(path.as_ref())
        || is_v2_lerobot_dataset(path.as_ref())
        || is_v3_lerobot_dataset(path.as_ref())
}

/// Check whether the provided path contains a v3 `LeRobot` dataset.
fn is_v3_lerobot_dataset(_path: impl AsRef<Path>) -> bool {
    let path = _path.as_ref();

    if !path.is_dir() {
        return false;
    }

    // v3 `LeRobot` datasets have per episode metadata stored under `meta/episodes/`
    has_sub_directories(&["meta", "data"], path) && path.join("meta").join("episodes").is_dir()
}

/// Check whether the provided path contains a v2 `LeRobot` dataset.
fn is_v2_lerobot_dataset(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();

    if !path.is_dir() {
        return false;
    }

    // v2 `LeRobot` datasets store the metadata in a `meta` directory,
    // instead of the `meta_data` directory used in v1 datasets.
    has_sub_directories(&["meta", "data"], path)
}

/// Check whether the provided path contains a v1 `LeRobot` dataset.
fn is_v1_lerobot_dataset(path: impl AsRef<Path>) -> bool {
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
        if let Some(names) = &self.names
            && let Some(channel_idx) = names.0.iter().position(|name| name == "channels")
        {
            // If channel_idx is within bounds of shape, return that dimension
            if channel_idx < self.shape.len() {
                return self.shape[channel_idx];
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
/// - A map with a string array value (e.g., `{ "motors": ["motor_0", "motor_1", …] }` or `{ "axes": ["x", "y", "z"] }`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Names(pub(super) Vec<String>);

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

/// Newtype wrapper for episode indices.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct EpisodeIndex(pub usize);

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

#[cfg(test)]
mod tests {
    use serde_json;

    use super::*;

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
