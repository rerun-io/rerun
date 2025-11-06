use crate::lerobot::{DType, EpisodeIndex, Feature, LeRobotDatasetTask, LeRobotError, TaskIndex};

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use ahash::HashMap;
use anyhow::{Context as _, Context, anyhow};
use arrow::buffer::ScalarBuffer;
use arrow::{
    array::{
        ArrayRef, BinaryArray, FixedSizeListArray, Int64Array, RecordBatch, StringArray,
        StructArray,
    },
    compute::cast,
    datatypes::{DataType, Field},
};
use itertools::{Either, Itertools};
use parquet::arrow::arrow_reader::{ParquetRecordBatchReader, ParquetRecordBatchReaderBuilder};
use re_types::archetypes::VideoStream;
use re_types::components::VideoCodec;
use re_video::{GopStartDetection, SampleIndex, StableIndexDeque, VideoDataDescription};
use serde::de::{DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{
    ArrowArray, Chunk, ChunkId, EntityPath, RowId, TimeColumn, TimeInt, TimePoint, Timeline,
    TimelineName, external::nohash_hasher::IntMap,
};
use re_log_types::{ApplicationId, StoreId};
use re_types::{
    archetypes::{
        self, AssetVideo, DepthImage, EncodedImage, Scalars, TextDocument, VideoFrameReference,
    },
    components::VideoTimestamp,
};

use crate::{DataLoaderError, LoadedData, load_file::prepare_store_info};

use std::sync::Arc;

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
/// ├── data/
/// │   └── chunk-000/
/// │       ├── episode_000000.parquet
/// │       ├── episode_000001.parquet
/// │       └── …
/// ├── meta/
/// │   ├── episodes/
/// │   │   └── chunk-000/
/// │   │       ├── file-000.parquet
/// │   │       ├── file-001.parquet
/// │   │       └── …
/// │   ├── tasks.parquet
/// │   ├── stats.json
/// │   └── info.json
/// └── videos/
///     └── chunk-000/
///         └── observation.image/
///             ├── episode_000000.mp4
///             ├── episode_000001.mp4
///             └── …
/// ```
///
/// ## File layout
///
/// - `data/`: Stores episode data in Parquet format, organized in chunks.
/// - `meta/`: Contains metadata files:
///   - `episodes/`: Episode-specific metadata (tasks, number of frames, etc.).
///   - `info.json`: General dataset metadata (robot type, number of episodes, etc.).
///   - `tasks.parquet`: Task definitions for episodes.
///   - `stats.json`: Summary statistics of dataset features.
/// - `videos/`: Optional directory storing video observations for episodes, organized similarly to `data/`.
///
/// Each episode is identified by a unique index and mapped to its corresponding chunk, based on the number of episodes
/// per chunk (which can be found in `meta/info.json`).
pub struct LeRobotDatasetV3 {
    pub path: PathBuf,
    pub metadata: LeRobotDatasetMetadataV3,
}

impl LeRobotDatasetV3 {
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
        let metadata = LeRobotDatasetMetadataV3::load_from_directory(&metadatapath)?;

        Ok(Self {
            path: path.to_path_buf(),
            metadata,
        })
    }

    /// Read the Parquet data file for the provided episode.
    pub fn read_episode_data(&self, episode: EpisodeIndex) -> Result<RecordBatch, LeRobotError> {
        let episode_data = self
            .metadata
            .get_episode_data(episode)
            .ok_or(LeRobotError::InvalidEpisodeIndex(episode))?;

        let episode_data_path = self.metadata.info.episode_data_path(episode_data);
        println!("Reading episode data from path: {episode_data_path:?}");
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
        let episode_data = self
            .metadata
            .get_episode_data(episode)
            .ok_or(LeRobotError::InvalidEpisodeIndex(episode))?;

        let video_file = self
            .metadata
            .info
            .video_path(observation_key, episode_data)?;
        let videopath = self.path.join(video_file);

        let contents = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(&videopath).map_err(|err| LeRobotError::IO(err, videopath))?
        };

        Ok(Cow::Owned(contents))
    }

    /// Retrieve the task using the provided task index.
    pub fn task_by_index(&self, task: TaskIndex) -> Option<&LeRobotDatasetTask> {
        self.metadata.tasks.tasks.get(&task)
    }
}

/// Metadata for a `LeRobot` dataset.
///
/// This is a wrapper struct for the metadata files in the `meta` directory of a
/// `LeRobot` dataset. For more see [`LeRobotDataset`].
pub struct LeRobotDatasetMetadataV3 {
    pub info: LeRobotDatasetInfoV3,
    pub tasks: LeRobotDatasetV3Tasks,
    pub episodes: Vec<LeRobotEpisodeData>,
    // pub tasks: Vec<LeRobotDatasetTask>,
}

impl LeRobotDatasetMetadataV3 {
    /// Get the number of episodes in the dataset.
    pub fn episode_count(&self) -> usize {
        self.episodes.len()
    }

    /// Get episode data by index.
    pub fn get_episode_data(&self, episode: EpisodeIndex) -> Option<&LeRobotEpisodeData> {
        self.episodes.iter().find(|e| e.episode_index == episode)
    }

    /// Loads all metadata files from the provided directory.
    ///
    /// This method reads dataset metadata from JSON and JSONL files stored in the `meta/` directory.
    /// It retrieves general dataset information, a list of recorded episodes, and defined tasks.
    pub fn load_from_directory(metadir: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let metadir = metadir.as_ref();

        let episode_data = LeRobotEpisodeData::load_from_directory(metadir.join("episodes"))?;
        let info = LeRobotDatasetInfoV3::load_from_json_file(metadir.join("info.json"))?;
        let tasks = LeRobotDatasetV3Tasks::load_from_parquet_file(metadir.join("tasks.parquet"))?;
        Ok(Self {
            info,
            tasks,
            episodes: episode_data,
        })
    }
}

/// File metadata for a specific feature (video or image) in a LeRobot dataset.
///
/// In v3 datasets, each video/image feature can have its own chunk and file indices,
/// allowing multiple episodes to share the same video file efficiently.
#[derive(Debug, Clone)]
pub struct FeatureFileMetadata {
    /// Chunk index where the feature's file is located
    pub chunk_index: usize,
    /// File index within the chunk
    pub file_index: usize,
    /// Start timestamp for the feature data in this file
    #[allow(dead_code)]
    pub from_timestamp: Option<f64>,
    /// End timestamp for the feature data in this file
    #[allow(dead_code)]
    pub to_timestamp: Option<f64>,
}

/// Episode metadata for a LeRobot v3 dataset.
///
/// Contains file location information for both the episode data and individual video/image features.
#[derive(Debug, Clone)]
pub struct LeRobotEpisodeData {
    /// The index of this episode
    pub episode_index: EpisodeIndex,
    /// Chunk index for the episode's main data file
    pub data_chunk_index: usize,
    /// File index within the chunk for the episode's main data
    pub data_file_index: usize,
    /// File metadata for video/image features, keyed by feature name (e.g., "observation.images.cam_high")
    pub feature_files: HashMap<String, FeatureFileMetadata>,
}

impl LeRobotEpisodeData {
    fn load_from_directory(metadir: impl AsRef<Path>) -> Result<Vec<Self>, LeRobotError> {
        // Walk all subdirectories and load episode data files.
        let metadir = metadir.as_ref();
        let mut all_episodes = vec![];
        for entry in
            std::fs::read_dir(metadir).map_err(|err| LeRobotError::IO(err, metadir.to_owned()))?
        {
            let entry = entry.map_err(|err| LeRobotError::IO(err, metadir.to_owned()))?;
            let path = entry.path();
            let path = path.as_path();

            println!("Loading episode data from path: {path:?}");

            if path.is_dir() {
                for chunk_entry in
                    std::fs::read_dir(path).map_err(|err| LeRobotError::IO(err, path.to_owned()))?
                {
                    let chunk_entry =
                        chunk_entry.map_err(|err| LeRobotError::IO(err, path.to_owned()))?;
                    let chunk_path = chunk_entry.path();

                    if chunk_path.is_file() {
                        let chunk_parquet = ParquetRecordBatchReaderBuilder::try_new(
                            File::open(&chunk_path)
                                .map_err(|err| LeRobotError::IO(err, chunk_path.clone()))?,
                        )?
                        .build()?;

                        let episode_data: Vec<_> = chunk_parquet
                            .filter_map(|b| {
                                let b = b.ok()?;

                                let episode_index = b
                                    .column_by_name("episode_index")?
                                    .as_any()
                                    .downcast_ref::<arrow::array::Int64Array>()?;

                                let data_chunk_index = b
                                    .column_by_name("data/chunk_index")?
                                    .as_any()
                                    .downcast_ref::<arrow::array::Int64Array>()?;

                                let data_file_index = b
                                    .column_by_name("data/file_index")?
                                    .as_any()
                                    .downcast_ref::<arrow::array::Int64Array>()?;

                                // Parse feature-specific file metadata (videos, images)
                                // Pattern: "videos/{feature_name}/{field}" where field is chunk_index, file_index, from_timestamp, to_timestamp
                                let feature_metadata = Self::parse_feature_metadata(&b);

                                let mut episodes = vec![];
                                for i in 0..b.num_rows() {
                                    // Build feature_files map for this episode
                                    let feature_files = feature_metadata
                                        .iter()
                                        .filter_map(|(feature_name, metadata)| {
                                            // Only include if both chunk_index and file_index are present
                                            let chunk_index = metadata.chunk_index.as_ref()?;
                                            let file_index = metadata.file_index.as_ref()?;

                                            Some((
                                                feature_name.clone(),
                                                FeatureFileMetadata {
                                                    chunk_index: chunk_index.value(i) as usize,
                                                    file_index: file_index.value(i) as usize,
                                                    from_timestamp: metadata
                                                        .from_timestamp
                                                        .as_ref()
                                                        .and_then(|ts| {
                                                            ts.is_valid(i).then(|| ts.value(i))
                                                        }),
                                                    to_timestamp: metadata
                                                        .to_timestamp
                                                        .as_ref()
                                                        .and_then(|ts| {
                                                            ts.is_valid(i).then(|| ts.value(i))
                                                        }),
                                                },
                                            ))
                                        })
                                        .collect();

                                    episodes.push(LeRobotEpisodeData {
                                        episode_index: EpisodeIndex(
                                            episode_index.value(i) as usize
                                        ),
                                        data_chunk_index: data_chunk_index.value(i) as usize,
                                        data_file_index: data_file_index.value(i) as usize,
                                        feature_files,
                                    });
                                }
                                Some(episodes)
                            })
                            .flatten()
                            .collect();

                        all_episodes.extend(episode_data);
                    }
                }
            }
        }

        Ok(all_episodes)
    }

    /// Parse feature-specific file metadata from a [`RecordBatch`].
    ///
    /// Looks for columns matching pattern `videos/{feature_name}/{field}`
    /// and groups them by feature name.
    fn parse_feature_metadata(batch: &RecordBatch) -> HashMap<String, FeatureMetadataColumns> {
        use arrow::array::Float64Array;

        let mut features: HashMap<String, FeatureMetadataColumns> = HashMap::default();
        let schema = batch.schema();

        for field in schema.fields() {
            let column_name = field.name();

            // Look for columns like "videos/{feature_name}/chunk_index"
            if let Some(rest) = column_name.strip_prefix("videos/") {
                if let Some((feature_name, field_name)) = rest.rsplit_once('/') {
                    let entry = features.entry(feature_name.to_owned()).or_default();

                    match field_name {
                        "chunk_index" => {
                            if let Some(col) = batch
                                .column_by_name(column_name)
                                .and_then(|c| c.downcast_array_ref::<Int64Array>())
                            {
                                entry.chunk_index = Some(Arc::new(col.clone()));
                            }
                        }
                        "file_index" => {
                            if let Some(col) = batch
                                .column_by_name(column_name)
                                .and_then(|c| c.downcast_array_ref::<Int64Array>())
                            {
                                entry.file_index = Some(Arc::new(col.clone()));
                            }
                        }
                        "from_timestamp" => {
                            if let Some(col) = batch
                                .column_by_name(column_name)
                                .and_then(|c| c.downcast_array_ref::<Float64Array>())
                            {
                                entry.from_timestamp = Some(Arc::new(col.clone()));
                            }
                        }
                        "to_timestamp" => {
                            if let Some(col) = batch
                                .column_by_name(column_name)
                                .and_then(|c| c.downcast_array_ref::<Float64Array>())
                            {
                                entry.to_timestamp = Some(Arc::new(col.clone()));
                            }
                        }
                        _ => {} // Ignore unknown fields
                    }
                }
            }
        }

        features
    }
}

/// Temporary structure to hold Arrow arrays for feature metadata during parsing.
#[derive(Default)]
struct FeatureMetadataColumns {
    chunk_index: Option<Arc<Int64Array>>,
    file_index: Option<Arc<Int64Array>>,
    from_timestamp: Option<Arc<arrow::array::Float64Array>>,
    to_timestamp: Option<Arc<arrow::array::Float64Array>>,
}

/// `LeRobot` dataset metadata.
///
/// This struct contains the metadata for a `LeRobot` dataset, and is loaded from the `meta/info.json` file
/// of the dataset.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LeRobotDatasetInfoV3 {
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

impl LeRobotDatasetInfoV3 {
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

    /// Generates the file path for a given episode's Parquet data.
    pub fn episode_data_path(&self, episode_data: &LeRobotEpisodeData) -> PathBuf {
        // TODO(gijsd): Need a better way to handle this, as this only supports the default.
        self.data_path
            .replace(
                "{chunk_index:03d}",
                &format!("{:03}", episode_data.data_chunk_index),
            )
            .replace(
                "{file_index:03d}",
                &format!("{:03}", episode_data.data_file_index),
            )
            .into()
    }

    /// Generates the file path for a video observation of a given episode.
    ///
    /// In v3 datasets, video files are organized by feature-specific chunk and file indices,
    /// which are stored in the episode metadata and may differ from the episode data indices.
    pub fn video_path(
        &self,
        feature_key: &str,
        episode_data: &LeRobotEpisodeData,
    ) -> Result<PathBuf, LeRobotError> {
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

        let video_path_template = self
            .video_path
            .as_ref()
            .ok_or_else(|| LeRobotError::MissingDatasetInfo("video_path".to_owned()))?;

        // Try to get feature-specific file metadata from episode data
        if let Some(file_metadata) = episode_data.feature_files.get(feature_key) {
            // Use feature-specific chunk and file indices (v3 format)
            Ok(video_path_template
                .replace("{video_key}", feature_key)
                .replace(
                    "{chunk_index:03d}",
                    &format!("{:03}", file_metadata.chunk_index),
                )
                .replace(
                    "{file_index:03d}",
                    &format!("{:03}", file_metadata.file_index),
                )
                .into())
        } else {
            // Fallback: use old template format with episode-based indices
            // This handles backwards compatibility with older templates or missing metadata
            Ok(video_path_template
                .replace(
                    "{episode_chunk:03d}",
                    &format!("{:03}", episode_data.data_chunk_index),
                )
                .replace(
                    "{episode_index:06d}",
                    &format!("{:06}", episode_data.episode_index.0),
                )
                .replace("{video_key}", feature_key)
                .into())
        }
    }
}

// TODO(gijsd): Do we want to stream in episodes or tasks?
#[cfg(not(target_arch = "wasm32"))]
fn load_jsonl_file<D>(filepath: impl AsRef<Path>) -> Result<Vec<D>, LeRobotError>
where
    D: DeserializeOwned,
{
    use crate::lerobot::LeRobotError;

    let entries = std::fs::read_to_string(filepath.as_ref())
        .map_err(|err| LeRobotError::IO(err, filepath.as_ref().to_owned()))?
        .lines()
        .map(|line| serde_json::from_str(line))
        .collect::<Result<Vec<D>, _>>()?;

    Ok(entries)
}

pub struct LeRobotDatasetV3Tasks {
    pub tasks: HashMap<TaskIndex, LeRobotDatasetTask>,
}

impl LeRobotDatasetV3Tasks {
    pub fn load_from_parquet_file(filepath: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let filepath = filepath.as_ref().to_owned();
        let parquet_data =
            File::open(&filepath).map_err(|err| LeRobotError::IO(err, filepath.clone()))?;

        let reader = ParquetRecordBatchReaderBuilder::try_new(parquet_data)?.build()?;

        let tasks = reader
            .filter_map(|b| {
                let b = b.ok()?;
                let task_index = b.column_by_name("task_index")?;
                let task = b.column_by_name("__index_level_0__")?;
                let task_index = task_index
                    .as_any()
                    .downcast_ref::<arrow::array::Int64Array>()?;
                let task = task.as_any().downcast_ref::<StringArray>()?;
                Some(
                    (0..b.num_rows())
                        .map(|i| LeRobotDatasetTask {
                            index: TaskIndex(task_index.value(i) as usize),
                            task: task.value(i).to_owned(),
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
            .map(|t| (t.index, t))
            .collect::<HashMap<_, _>>();

        Ok(Self { tasks })
    }
}

// ============================================================================
// V3 Dataset Loading Functions
// ============================================================================

/// Columns in the `LeRobot` dataset schema that we do not visualize in the viewer, and thus ignore.
const LEROBOT_DATASET_IGNORED_COLUMNS: &[&str] =
    &["episode_index", "index", "frame_index", "timestamp"];

pub fn load_and_stream(
    dataset: &LeRobotDatasetV3,
    application_id: &ApplicationId,
    tx: &Sender<LoadedData>,
    loader_name: String,
) {
    // set up all recordings
    let episodes = prepare_episode_chunks(dataset, application_id, tx, loader_name.clone());

    for (episode, store_id) in &episodes {
        // log episode data to its respective recording
        match load_episode(dataset, *episode) {
            Ok(chunks) => {
                let recording_info = re_types::archetypes::RecordingInfo::new()
                    .with_name(format!("Episode {}", episode.0));

                let Ok(initial) = Chunk::builder(EntityPath::properties())
                    .with_archetype(RowId::new(), TimePoint::STATIC, &recording_info)
                    .build()
                else {
                    re_log::error!(
                        "Failed to build recording properties chunk for episode {}",
                        episode.0
                    );
                    return;
                };

                for chunk in std::iter::once(initial).chain(chunks.into_iter()) {
                    let data = LoadedData::Chunk(loader_name.clone(), store_id.clone(), chunk);

                    if tx.send(data).is_err() {
                        break; // The other end has decided to hang up, not our problem.
                    }
                }
            }
            Err(err) => {
                re_log::warn!(
                    "Failed to load episode {} from LeRobot dataset: {err}",
                    episode.0
                );
            }
        }
    }
}

/// Prepare the viewer for all episodes, by sending out a [`SetStoreInfo`](`re_log_types::SetStoreInfo`)
/// [`LogMsg`](`re_log_types::LogMsg`) for each episode.
fn prepare_episode_chunks(
    dataset: &LeRobotDatasetV3,
    application_id: &ApplicationId,
    tx: &Sender<LoadedData>,
    loader_name: String,
) -> Vec<(EpisodeIndex, StoreId)> {
    let mut store_ids = vec![];

    for episode_data in &dataset.metadata.episodes {
        let episode = episode_data.episode_index;

        let store_id = StoreId::recording(application_id.clone(), format!("episode_{}", episode.0));
        let set_store_info = LoadedData::LogMsg(
            loader_name.clone(),
            prepare_store_info(&store_id, re_log_types::FileSource::Sdk),
        );

        if tx.send(set_store_info).is_err() {
            break;
        }

        store_ids.push((episode, store_id));
    }

    store_ids
}

/// Loads a single episode from a `LeRobot` dataset and converts it into a collection of Rerun chunks.
///
/// This function processes an episode from the dataset by extracting the relevant data columns and
/// converting them into appropriate Rerun data structures. It handles different types of data
/// (videos, images, scalar values, etc.) based on their data type specifications in the dataset metadata.
pub fn load_episode(
    dataset: &LeRobotDatasetV3,
    episode: EpisodeIndex,
) -> Result<Vec<Chunk>, DataLoaderError> {
    let data = dataset
        .read_episode_data(episode)
        .map_err(|err| anyhow!("Reading data for episode {} failed: {err}", episode.0))?;

    let frame_indices = data
        .column_by_name("frame_index")
        .ok_or_else(|| anyhow!("Failed to get frame index column in LeRobot dataset"))?
        .clone();

    let timeline = re_log_types::Timeline::new_sequence("frame_index");
    let times: &arrow::buffer::ScalarBuffer<i64> = frame_indices
        .downcast_array_ref::<Int64Array>()
        .ok_or_else(|| anyhow!("LeRobot dataset frame indices are of an unexpected type"))?
        .values();

    let time_column = re_chunk::TimeColumn::new(None, timeline, times.clone());
    let timelines = std::iter::once((*timeline.name(), time_column.clone())).collect();

    let mut chunks = Vec::new();

    for (feature_key, feature) in dataset
        .metadata
        .info
        .features
        .iter()
        .filter(|(key, _)| !LEROBOT_DATASET_IGNORED_COLUMNS.contains(&key.as_str()))
    {
        match feature.dtype {
            DType::Video => {
                chunks.extend(load_episode_video(
                    dataset,
                    feature_key,
                    episode,
                    &timeline,
                    time_column.clone(),
                )?);
            }

            DType::Image => {
                let num_channels = feature.channel_dim();

                match num_channels {
                    1 => chunks.extend(load_episode_depth_images(feature_key, &timeline, &data)?),
                    3 => chunks.extend(load_episode_images(feature_key, &timeline, &data)?),
                    _ => re_log::warn_once!(
                        "Unsupported channel count {num_channels} (shape: {:?}) for LeRobot dataset; Only 1- and 3-channel images are supported",
                        feature.shape
                    ),
                }
            }
            DType::Int64 if feature_key == "task_index" => {
                // special case int64 task_index columns
                // this always refers to the task description in the dataset metadata.
                chunks.extend(log_episode_task(dataset, &timeline, &data)?);
            }
            DType::Int16 | DType::Int64 | DType::Bool | DType::String => {
                re_log::warn_once!(
                    "Loading LeRobot feature ({feature_key}) of dtype `{:?}` into Rerun is not yet implemented",
                    feature.dtype
                );
            }
            DType::Float32 | DType::Float64 => {
                chunks.extend(load_scalar(feature_key, feature, &timelines, &data)?);
            }
        }
    }

    Ok(chunks)
}

fn log_episode_task(
    dataset: &LeRobotDatasetV3,
    timeline: &Timeline,
    data: &RecordBatch,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
    let task_indices = data
        .column_by_name("task_index")
        .and_then(|c| c.downcast_array_ref::<Int64Array>())
        .with_context(|| "Failed to get task_index field from dataset!")?;

    let mut chunk = Chunk::builder("task");
    let mut row_id = RowId::new();
    let mut time_int = TimeInt::ZERO;

    for task_index in task_indices {
        let Some(task) = task_index
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| dataset.task_by_index(TaskIndex(i)))
        else {
            // if there is no valid task for the current frame index, we skip it.
            time_int = time_int.inc();
            continue;
        };

        let timepoint = TimePoint::default().with(*timeline, time_int);
        let text = TextDocument::new(task.task.clone());
        chunk = chunk.with_archetype(row_id, timepoint, &text);

        row_id = row_id.next();
        time_int = time_int.inc();
    }

    Ok(std::iter::once(chunk.build()?))
}

fn load_episode_images(
    observation: &str,
    timeline: &Timeline,
    data: &RecordBatch,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
    let image_bytes = data
        .column_by_name(observation)
        .and_then(|c| c.downcast_array_ref::<StructArray>())
        .and_then(|a| a.column_by_name("bytes"))
        .and_then(|a| a.downcast_array_ref::<BinaryArray>())
        .with_context(|| format!("Failed to get binary data from image feature: {observation}"))?;

    let mut chunk = Chunk::builder(observation);
    let mut row_id = RowId::new();

    for frame_idx in 0..image_bytes.len() {
        let img_buffer = image_bytes.value(frame_idx);
        let encoded_image = EncodedImage::from_file_contents(img_buffer.to_owned());
        let timepoint = TimePoint::default().with(*timeline, frame_idx as i64);
        chunk = chunk.with_archetype(row_id, timepoint, &encoded_image);

        row_id = row_id.next();
    }

    Ok(std::iter::once(chunk.build().with_context(|| {
        format!("Failed to build image chunk for image: {observation}")
    })?))
}

fn load_episode_depth_images(
    observation: &str,
    timeline: &Timeline,
    data: &RecordBatch,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
    let image_bytes = data
        .column_by_name(observation)
        .and_then(|c| c.downcast_array_ref::<StructArray>())
        .and_then(|a| a.column_by_name("bytes"))
        .and_then(|a| a.downcast_array_ref::<BinaryArray>())
        .with_context(|| format!("Failed to get binary data from image feature: {observation}"))?;

    let mut chunk = Chunk::builder(observation);
    let mut row_id = RowId::new();

    for frame_idx in 0..image_bytes.len() {
        let img_buffer = image_bytes.value(frame_idx);
        let depth_image = DepthImage::from_file_contents(img_buffer.to_owned())
            .map_err(|err| anyhow!("Failed to decode image: {err}"))?;

        let timepoint = TimePoint::default().with(*timeline, frame_idx as i64);
        chunk = chunk.with_archetype(row_id, timepoint, &depth_image);

        row_id = row_id.next();
    }

    Ok(std::iter::once(chunk.build().with_context(|| {
        format!("Failed to build image chunk for image: {observation}")
    })?))
}

fn load_episode_video(
    dataset: &LeRobotDatasetV3,
    observation: &str,
    episode: EpisodeIndex,
    timeline: &Timeline,
    time_column: TimeColumn,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
    let contents = dataset
        .read_episode_video_contents(observation, episode)
        .with_context(|| format!("Reading video contents for episode {episode:?} failed!"))?;

    let entity_path = observation;
    let video_bytes = contents.as_ref();

    // Parse the video to get its structure
    let video = VideoDataDescription::load_from_bytes(
        video_bytes,
        "video/mp4",
        "observation.images.cam_high",
    )
    .map_err(|err| {
        anyhow!("failed to read video data description for feature: {observation}: {err}")
    })?;

    let start_time = dataset
        .metadata
        .episodes
        .get(episode.0)
        .and_then(|ep_data| ep_data.feature_files.get(observation))
        .and_then(|file_meta| file_meta.from_timestamp)
        .unwrap_or(0.0);
    let end_time = dataset
        .metadata
        .episodes
        .get(episode.0)
        .and_then(|ep_data| ep_data.feature_files.get(observation))
        .and_then(|file_meta| file_meta.to_timestamp)
        .unwrap_or(0.0);

    if video.samples.is_empty() {
        return Err(DataLoaderError::Other(anyhow!(
            "Video feature {observation} for episode {episode:?} did not contain any samples"
        )));
    }

    // Convert timestamps to video time (assuming seconds)
    let timescale = video.timescale.unwrap();
    let start_video_time = re_video::Time::from_secs(start_time, timescale);
    let end_video_time = re_video::Time::from_secs(end_time, timescale);

    let start_gop = video
        .gop_index_containing_presentation_timestamp(start_video_time)
        .unwrap_or(0);

    let end_gop = video
        .gop_index_containing_presentation_timestamp(end_video_time)
        .unwrap_or(video.gops.num_elements() - 1);

    let start_sample_idx = video.gops[start_gop].sample_range.start;
    let end_sample_idx_exclusive = video.gops[end_gop].sample_range.end;

    let sample_range = start_sample_idx..end_sample_idx_exclusive;

    // Extract all samples in this range into a Vec
    let mut samples = Vec::new();
    let mut buffers = StableIndexDeque::new();
    buffers.push_back(video_bytes); // original asset slice

    for (sample_idx, sample_meta) in video.samples.iter_index_range_clamped(&sample_range) {
        let chunk = sample_meta.get(&buffers, sample_idx).ok_or_else(|| {
            anyhow!("Sample {sample_idx} out of bounds for feature {observation}")
        })?;

        samples.push((sample_meta.clone(), chunk.data)); // chunk.data is the per-sample bytes
    }

    println!(
        "Extracted {} samples from time range {:.3}s to {:.3}s",
        samples.len(),
        start_time,
        end_time
    );
    println!(
        "Sample range: {} to {}",
        sample_range.start, sample_range.end
    );

    let (samples_meta, samples): (Vec<_>, Vec<_>) = samples.into_iter().unzip();

    let samples_column = VideoStream::update_fields()
        .with_many_codec(vec![VideoCodec::AV1; samples.len()])
        .with_many_sample(samples)
        .columns_of_unit_batches()
        .with_context(|| format!("failed to create `VideoStream`"))?;

    // Build uniform time column that goes between start and end for the amount of samples
    let num_samples = samples_meta.len();
    let uniform_times: Vec<i64> = if num_samples > 1 {
        // Create evenly spaced timestamps based on presentation timestamps
        let first_sample = &samples_meta[0];
        let last_sample = &samples_meta[num_samples - 1];
        let first_pts = first_sample.presentation_timestamp.0 * 1_000_000; // convert to nanoseconds
        let last_pts = last_sample.presentation_timestamp.0 * 1_000_000; // convert to nanoseconds

        (0..num_samples)
            .map(|i| first_pts + ((last_pts - first_pts) * i as i64) / (num_samples - 1) as i64)
            .collect()
    } else if num_samples == 1 {
        vec![samples_meta[0].presentation_timestamp.0]
    } else {
        vec![]
    };

    let uniform_time_column = TimeColumn::new(
        Some(true), // is_sorted
        Timeline::new_duration("video"),
        ScalarBuffer::from(uniform_times),
    );

    let samples_chunk = Chunk::from_auto_row_ids(
        re_chunk::ChunkId::new(),
        entity_path.into(),
        std::iter::once(("video".into(), uniform_time_column)).collect(),
        samples_column.collect(),
    )?;

    Ok(std::iter::once(samples_chunk))
}

fn find_previous_or_current_gop_start(
    video: &VideoDataDescription,
    mut sample_idx: SampleIndex,
    video_bytes: &[u8],
    observation: &str,
) -> Result<SampleIndex, DataLoaderError> {
    let min_index = video.samples.min_index();
    sample_idx = sample_idx.max(min_index);

    loop {
        if sample_is_gop_start(video, sample_idx, video_bytes, observation)? {
            return Ok(sample_idx);
        }

        if sample_idx == min_index {
            return Ok(sample_idx);
        }

        sample_idx -= 1;
    }
}

fn find_next_gop_start_after(
    video: &VideoDataDescription,
    sample_idx: SampleIndex,
    video_bytes: &[u8],
    observation: &str,
) -> Result<SampleIndex, DataLoaderError> {
    let min_index = video.samples.min_index();
    let mut current_idx = sample_idx.saturating_add(1).max(min_index);
    let end_index = video.samples.next_index();

    while current_idx < end_index {
        if sample_is_gop_start(video, current_idx, video_bytes, observation)? {
            return Ok(current_idx);
        }

        current_idx += 1;
    }

    Ok(end_index)
}

fn sample_is_gop_start(
    video: &VideoDataDescription,
    sample_idx: SampleIndex,
    video_bytes: &[u8],
    observation: &str,
) -> Result<bool, DataLoaderError> {
    let sample_meta = video.samples.get(sample_idx).ok_or_else(|| {
        DataLoaderError::Other(anyhow!(
            "Sample index {sample_idx} is out of bounds for feature {observation}"
        ))
    })?;

    if sample_meta.is_sync {
        return Ok(true);
    }

    let byte_range = sample_meta.byte_span.range_usize();
    if byte_range.end > video_bytes.len() {
        return Err(DataLoaderError::Other(anyhow!(
            "Video sample range {:?} for feature {observation} is out of bounds (video size: {})",
            byte_range,
            video_bytes.len()
        )));
    }
    let sample_bytes = &video_bytes[byte_range];

    match re_video::detect_gop_start(sample_bytes, video.codec) {
        Ok(GopStartDetection::StartOfGop(_)) => Ok(true),
        Ok(GopStartDetection::NotStartOfGop) => Ok(false),
        Err(err) => {
            re_log::warn_once!(
                "Failed to detect GOP start for sample {sample_idx} in feature {observation}: {err}"
            );
            Ok(false)
        }
    }
}

/// Helper type similar to [`Either`], but with 3 variants.
enum ScalarChunkIterator {
    Empty(std::iter::Empty<Chunk>),
    Batch(Box<dyn ExactSizeIterator<Item = Chunk>>),

    // Boxed, because `Chunk` is huge, and by extension so is `std::iter::Once<Chunk>`.
    Single(Box<std::iter::Once<Chunk>>),
}

impl Iterator for ScalarChunkIterator {
    type Item = Chunk;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty(iter) => iter.next(),
            Self::Batch(iter) => iter.next(),
            Self::Single(iter) => iter.next(),
        }
    }
}

impl ExactSizeIterator for ScalarChunkIterator {}

fn load_scalar(
    feature_key: &str,
    feature: &Feature,
    timelines: &IntMap<TimelineName, TimeColumn>,
    data: &RecordBatch,
) -> Result<ScalarChunkIterator, DataLoaderError> {
    let field = data
        .schema_ref()
        .field_with_name(feature_key)
        .with_context(|| {
            format!("Failed to get field for feature {feature_key} from parquet file")
        })?;

    let entity_path = EntityPath::parse_forgiving(field.name());

    match field.data_type() {
        DataType::FixedSizeList(_, _) => {
            let fixed_size_array = data
                .column_by_name(feature_key)
                .and_then(|col| col.downcast_array_ref::<FixedSizeListArray>())
                .ok_or_else(|| {
                    DataLoaderError::Other(anyhow!(
                        "Failed to downcast feature to FixedSizeListArray"
                    ))
                })?;

            let batch_chunks =
                make_scalar_batch_entity_chunks(entity_path, feature, timelines, fixed_size_array)?;
            Ok(ScalarChunkIterator::Batch(Box::new(batch_chunks)))
        }
        DataType::List(_field) => {
            let list_array = data
                .column_by_name(feature_key)
                .and_then(|col| col.downcast_array_ref::<arrow::array::ListArray>())
                .ok_or_else(|| {
                    DataLoaderError::Other(anyhow!("Failed to downcast feature to ListArray"))
                })?;

            let sliced = extract_list_array_elements_as_f64(list_array).with_context(|| {
                format!("Failed to cast scalar feature {entity_path} to Float64")
            })?;

            Ok(ScalarChunkIterator::Single(Box::new(std::iter::once(
                make_scalar_entity_chunk(entity_path, timelines, &sliced)?,
            ))))
        }
        DataType::Float32 | DataType::Float64 => {
            let feature_data = data.column_by_name(feature_key).ok_or_else(|| {
                DataLoaderError::Other(anyhow!(
                    "Failed to get LeRobot dataset column data for: {:?}",
                    field.name()
                ))
            })?;

            let sliced = extract_scalar_slices_as_f64(feature_data).with_context(|| {
                format!("Failed to cast scalar feature {entity_path} to Float64")
            })?;

            Ok(ScalarChunkIterator::Single(Box::new(std::iter::once(
                make_scalar_entity_chunk(entity_path, timelines, &sliced)?,
            ))))
        }
        _ => {
            re_log::warn_once!(
                "Tried logging scalar {} with unsupported dtype: {}",
                field.name(),
                field.data_type()
            );
            Ok(ScalarChunkIterator::Empty(std::iter::empty()))
        }
    }
}

fn make_scalar_batch_entity_chunks(
    entity_path: EntityPath,
    feature: &Feature,
    timelines: &IntMap<TimelineName, TimeColumn>,
    data: &FixedSizeListArray,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
    let num_elements = data.value_length() as usize;

    let mut chunks = Vec::with_capacity(num_elements);

    let sliced = extract_fixed_size_list_array_elements_as_f64(data)
        .with_context(|| format!("Failed to cast scalar feature {entity_path} to Float64"))?;

    chunks.push(make_scalar_entity_chunk(
        entity_path.clone(),
        timelines,
        &sliced,
    )?);

    // If we have names for this feature, we insert a single static chunk containing the names.
    if let Some(names) = feature.names.clone() {
        let names: Vec<_> = (0..data.value_length() as usize)
            .map(|idx| names.name_for_index(idx))
            .collect();

        chunks.push(
            Chunk::builder(entity_path)
                .with_row(
                    RowId::new(),
                    TimePoint::default(),
                    std::iter::once((
                        archetypes::SeriesLines::descriptor_names(),
                        Arc::new(StringArray::from_iter(names)) as Arc<dyn ArrowArray>,
                    )),
                )
                .build()?,
        );
    }

    Ok(chunks.into_iter())
}

fn make_scalar_entity_chunk(
    entity_path: EntityPath,
    timelines: &IntMap<TimelineName, TimeColumn>,
    sliced_data: &[ArrayRef],
) -> Result<Chunk, DataLoaderError> {
    let data_arrays = sliced_data
        .iter()
        .map(|e| Some(e.as_ref()))
        .collect::<Vec<_>>();

    let data_field_inner = Field::new("item", DataType::Float64, true /* nullable */);
    #[expect(clippy::unwrap_used)] // we know we've given the right field type
    let data_field_array: arrow::array::ListArray =
        re_arrow_util::arrays_to_list_array(data_field_inner.data_type().clone(), &data_arrays)
            .unwrap();

    Ok(Chunk::from_auto_row_ids(
        ChunkId::new(),
        entity_path,
        timelines.clone(),
        std::iter::once((Scalars::descriptor_scalars().clone(), data_field_array)).collect(),
    )?)
}

fn extract_scalar_slices_as_f64(data: &ArrayRef) -> anyhow::Result<Vec<ArrayRef>> {
    // cast the slice to f64 first, as scalars need an f64
    let scalar_values = cast(&data, &DataType::Float64)
        .with_context(|| format!("Failed to cast {} to Float64", data.data_type()))?;

    Ok((0..data.len())
        .map(|idx| scalar_values.slice(idx, 1))
        .collect::<Vec<_>>())
}

fn extract_fixed_size_list_array_elements_as_f64(
    data: &FixedSizeListArray,
) -> anyhow::Result<Vec<ArrayRef>> {
    (0..data.len())
        .map(|idx| {
            cast(&data.value(idx), &DataType::Float64)
                .with_context(|| format!("Failed to cast {} to Float64", data.data_type()))
        })
        .collect::<Result<Vec<_>, _>>()
}

fn extract_list_array_elements_as_f64(
    data: &arrow::array::ListArray,
) -> anyhow::Result<Vec<ArrayRef>> {
    (0..data.len())
        .map(|idx| {
            cast(&data.value(idx), &DataType::Float64)
                .with_context(|| format!("Failed to cast {} to Float64", data.data_type()))
        })
        .collect::<Result<Vec<_>, _>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_dataset_tasks() {
        let tasks = LeRobotDatasetV3Tasks::load_from_parquet_file(
            "/Users/gijsd/rerun-io/lerobot_datasets/aloha_mobile_cabinet/meta/tasks.parquet",
        )
        .unwrap();

        println!("tasks: {:?}", tasks.tasks);
    }

    #[test]
    fn test_load_dataset_metadata() {
        let metadata = LeRobotDatasetMetadataV3::load_from_directory(
            "/Users/gijsd/rerun-io/lerobot_datasets/aloha_mobile_cabinet/meta",
        )
        .unwrap();

        // Verify episode metadata was loaded
        assert!(!metadata.episodes.is_empty(), "Should have loaded episodes");

        // Check that the first episode has feature file metadata
        let first_episode = &metadata.episodes[0];
        assert!(
            !first_episode.feature_files.is_empty(),
            "First episode should have feature file metadata"
        );

        // Verify video feature metadata exists
        let video_features = [
            "observation.images.cam_high",
            "observation.images.cam_left_wrist",
            "observation.images.cam_right_wrist",
        ];

        for feature in video_features {
            let file_metadata = first_episode.feature_files.get(feature);
            assert!(
                file_metadata.is_some(),
                "Feature {feature} should have file metadata"
            );

            if let Some(metadata) = file_metadata {
                println!(
                    "Feature '{}': chunk={}, file={}, from_ts={:?}, to_ts={:?}",
                    feature,
                    metadata.chunk_index,
                    metadata.file_index,
                    metadata.from_timestamp,
                    metadata.to_timestamp
                );
            }
        }
    }

    #[test]
    fn test_video_path_with_feature_metadata() {
        let dataset = LeRobotDatasetV3::load_from_directory(
            "/Users/gijsd/rerun-io/lerobot_datasets/aloha_mobile_cabinet",
        )
        .unwrap();

        let first_episode = &dataset.metadata.episodes[0];
        let feature_key = "observation.images.cam_high";

        // Generate video path
        let video_path = dataset
            .metadata
            .info
            .video_path(feature_key, first_episode)
            .unwrap();

        println!("Generated video path: {:?}", video_path);

        // The path should use the feature-specific chunk/file indices
        assert!(
            video_path.to_string_lossy().contains("chunk-000"),
            "Path should contain chunk directory"
        );
        assert!(
            video_path.to_string_lossy().contains("file-"),
            "Path should contain file reference"
        );
        assert!(
            video_path.to_string_lossy().contains(feature_key),
            "Path should contain feature name"
        );
    }
}
