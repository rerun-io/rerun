use crate::common::{LEROBOT_DATASET_IGNORED_COLUMNS, LeRobotDatasetOps, derive_timeline};
use crate::plan::{EpisodePlan, PlannedFeature, PlannedVideo};
use crate::streaming::EpisodeChunkIterator;
use crate::{DType, EpisodeIndex, Feature, LeRobotDatasetTask, LeRobotError, TaskIndex};

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use ahash::HashMap;
use anyhow::{Context as _, anyhow};
use arrow::array::{Int64Array, RecordBatch};
use itertools::Itertools as _;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{Chunk, TimeInt};

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
pub struct LeRobotDatasetV2 {
    pub path: PathBuf,
    pub metadata: LeRobotDatasetMetadata,
}

impl LeRobotDatasetV2 {
    /// Plans a single episode without decoding any feature data.
    ///
    /// Resolves everything version-specific — video file paths, task text — into a
    /// self-contained [`EpisodePlan`]. Turning the plan into chunks happens in
    /// [`crate::execute`], driven by [`EpisodeChunkIterator`].
    pub(crate) fn build_plan(&self, episode: EpisodeIndex) -> Result<EpisodePlan, LeRobotError> {
        let data = self.read_episode_data(episode)?;
        let (timeline, time_column) = derive_timeline(&data)?;
        let mut features = Vec::new();

        for (key, feature) in self
            .metadata
            .info
            .features
            .iter()
            .filter(|(key, _)| !LEROBOT_DATASET_IGNORED_COLUMNS.contains(&key.as_str()))
        {
            match feature.dtype {
                // v2 reads the whole video file per episode
                DType::Video => features.push(PlannedFeature::Video {
                    entity: key.clone(),
                    video: PlannedVideo::Asset {
                        file: self.path.join(self.metadata.info.video_path(key, episode)?),
                    },
                }),
                DType::Image => match feature.channel_dim() {
                    1 => features.push(PlannedFeature::DepthImage { key: key.clone() }),
                    3 => features.push(PlannedFeature::Image { key: key.clone() }),
                    num_channels => re_log::warn_once!(
                        "Unsupported channel count {num_channels} (shape: {:?}) for LeRobot dataset; Only 1- and 3-channel images are supported",
                        feature.shape
                    ),
                },
                DType::Int64 if key == "task_index" => features.push(PlannedFeature::Text {
                    entity: "task".to_owned(),
                    rows: self.resolve_task_rows(&data)?,
                }),
                DType::Float32 | DType::Float64 => features.push(PlannedFeature::Scalar {
                    key: key.clone(),
                    feature: feature.clone(),
                }),
                DType::Language => {
                    return Err(anyhow!(
                        "LeRobot dataset v2 importer does not support the `language` dtype (feature: {key})"
                    ).into());
                }
                DType::Int16 | DType::Int64 | DType::Bool | DType::String => {
                    re_log::warn_once!(
                        "Loading LeRobot feature ({key}) of dtype `{:?}` into Rerun is not yet implemented",
                        feature.dtype
                    );
                }
            }
        }
        Ok(EpisodePlan {
            timeline,
            time_column,
            parquet_data: data,
            features,
        })
    }

    fn resolve_task_rows(
        &self,
        data: &RecordBatch,
    ) -> Result<Vec<(TimeInt, String)>, LeRobotError> {
        let task_indices = data
            .column_by_name("task_index")
            .and_then(|c| c.downcast_array_ref::<Int64Array>())
            .with_context(|| "Failed to get task_index field from dataset!")?;

        let mut rows = Vec::new();
        let mut time_int = TimeInt::ZERO;
        for task_index in task_indices {
            if let Some(task) = task_index
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| self.task_by_index(TaskIndex(i)))
            {
                rows.push((time_int, task.task.clone()));
            }
            time_int = time_int.inc();
        }
        Ok(rows)
    }

    /// Loads a `LeRobotDataset` from a directory.
    ///
    /// This method initializes a dataset by reading its metadata from the `meta/` directory.
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
        if !self.metadata.episodes.contains_key(&episode) {
            return Err(LeRobotError::InvalidEpisodeIndex(episode));
        }

        let episode_data_path = self.metadata.info.episode_data_path(episode)?;
        let episode_parquet_file = self.path.join(episode_data_path);

        let file = File::open(&episode_parquet_file)
            .map_err(|err| LeRobotError::io(err, episode_parquet_file))?;
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
            std::fs::read(&videopath).map_err(|err| LeRobotError::io(err, videopath))?
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
/// `LeRobot` dataset. For more see [`LeRobotDatasetV2`].
#[derive(Debug, Clone)]
pub struct LeRobotDatasetMetadata {
    pub info: LeRobotDatasetInfo,
    pub episodes: BTreeMap<EpisodeIndex, LeRobotDatasetEpisode>,
    pub tasks: Vec<LeRobotDatasetTask>,
}

impl LeRobotDatasetMetadata {
    /// Get the number of episodes in the dataset.
    pub fn episode_count(&self) -> usize {
        self.episodes.len()
    }

    /// Get episode metadata by index.
    pub fn get_episode(&self, episode: EpisodeIndex) -> Option<&LeRobotDatasetEpisode> {
        self.episodes.get(&episode)
    }

    /// Iterate over the indices of all episodes in the dataset.
    pub fn iter_episode_indices(&self) -> impl Iterator<Item = EpisodeIndex> {
        self.episodes.keys().copied()
    }

    /// Loads all metadata files from the provided directory.
    ///
    /// This method reads dataset metadata from JSON and JSONL files stored in the `meta/` directory.
    /// It retrieves general dataset information, a list of recorded episodes, and defined tasks.
    pub fn load_from_directory(metadir: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let metadir = metadir.as_ref();

        let info = LeRobotDatasetInfo::load_from_json_file(metadir.join("info.json"))?;
        let mut episodes_vec: Vec<LeRobotDatasetEpisode> =
            load_jsonl_file(metadir.join("episodes.jsonl"))?;
        let mut tasks = load_jsonl_file(metadir.join("tasks.jsonl"))?;

        // Sort episodes by index to ensure consistent ordering when loading
        episodes_vec.sort_by_key(|e: &LeRobotDatasetEpisode| e.index);

        let episodes = episodes_vec
            .into_iter()
            .map(|episode| (episode.index, episode))
            .collect::<BTreeMap<EpisodeIndex, LeRobotDatasetEpisode>>();

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
    pub robot_type: Option<String>,

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
    pub fps: f32,

    /// A mapping of feature names to their respective [`Feature`] definitions.
    pub features: HashMap<String, Feature>,
}

impl LeRobotDatasetInfo {
    /// Loads `LeRobotDatasetInfo` from a JSON file.
    ///
    /// The `LeRobot` dataset info file is typically stored under `meta/info.json`.
    pub fn load_from_json_file(filepath: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let info_file = File::open(filepath.as_ref())
            .map_err(|err| LeRobotError::io(err, filepath.as_ref()))?;
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

// TODO(gijsd): Do we want to stream in episodes or tasks?
#[cfg(not(target_arch = "wasm32"))]
fn load_jsonl_file<D>(filepath: impl AsRef<Path>) -> Result<Vec<D>, LeRobotError>
where
    D: DeserializeOwned,
{
    use crate::LeRobotError;

    let entries = std::fs::read_to_string(filepath.as_ref())
        .map_err(|err| LeRobotError::io(err, filepath.as_ref()))?
        .lines()
        .map(|line| serde_json::from_str(line))
        .try_collect()?;

    Ok(entries)
}

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

impl LeRobotDatasetOps for LeRobotDatasetV2 {
    fn iter_episode_indices(&self) -> impl std::iter::Iterator<Item = EpisodeIndex> {
        self.metadata.iter_episode_indices()
    }

    fn load_episode_chunks(&self, episode: EpisodeIndex) -> Result<Vec<Chunk>, LeRobotError> {
        EpisodeChunkIterator::new(self.build_plan(episode)?).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use arrow::array::RecordBatchOptions;
    use arrow::datatypes::{DataType, Field as ArrowField, Schema};
    use parquet::arrow::ArrowWriter;

    /// Write a minimal single-column (`frame_index`) parquet file so `load_episode` can build a timeline.
    fn write_minimal_parquet(path: &Path) {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![ArrowField::new("frame_index", DataType::Int64, false)],
            Default::default(),
        ));
        let batch = RecordBatch::try_new_with_options(
            schema.clone(),
            vec![Arc::new(Int64Array::from(vec![0_i64, 1, 2]))],
            &RecordBatchOptions::default(),
        )
        .unwrap();

        let file = File::create(path).unwrap();
        let mut writer = ArrowWriter::try_new(file, schema, None).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
    }

    /// Build an in-memory v2 dataset with a single `language` feature, backed by a parquet file on disk.
    fn dataset_with_language_feature(dir: &Path) -> LeRobotDatasetV2 {
        let data_path = "episode_000000.parquet";
        write_minimal_parquet(&dir.join(data_path));

        let mut features = HashMap::default();
        features.insert(
            "instruction".to_owned(),
            Feature {
                dtype: DType::Language,
                shape: vec![1],
                names: None,
            },
        );

        let info = LeRobotDatasetInfo {
            robot_type: None,
            codebase_version: "v2.0".to_owned(),
            total_episodes: 1,
            total_frames: 3,
            total_tasks: 0,
            total_videos: 0,
            total_chunks: 1,
            chunks_size: 1,
            data_path: data_path.to_owned(),
            video_path: None,
            image_path: None,
            fps: 30.0,
            features,
        };

        let episodes = std::iter::once((
            EpisodeIndex(0),
            LeRobotDatasetEpisode {
                index: EpisodeIndex(0),
                tasks: vec![],
                length: 3,
            },
        ))
        .collect();

        LeRobotDatasetV2 {
            path: dir.to_path_buf(),
            metadata: LeRobotDatasetMetadata {
                info,
                episodes,
                tasks: vec![],
            },
        }
    }

    #[test]
    fn test_v2_language_dtype_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let dataset = dataset_with_language_feature(dir.path());

        let err = dataset
            .load_episode_chunks(EpisodeIndex(0))
            .expect_err("v2 importer must reject the `language` dtype");

        assert!(
            err.to_string().to_lowercase().contains("language"),
            "expected a `language`-related error, got: {err}"
        );
    }
}
