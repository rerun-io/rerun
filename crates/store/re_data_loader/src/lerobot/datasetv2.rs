use crate::lerobot::{DType, EpisodeIndex, Feature, LeRobotDatasetTask, LeRobotError, TaskIndex};

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use ahash::HashMap;
use anyhow::{Context as _, anyhow};
use arrow::{
    array::{
        ArrayRef, BinaryArray, FixedSizeListArray, Int64Array, RecordBatch, StringArray,
        StructArray,
    },
    compute::cast,
    datatypes::{DataType, Field},
};
use itertools::Either;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
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
        if !self.metadata.episodes.contains_key(&episode) {
            return Err(LeRobotError::InvalidEpisodeIndex(episode));
        }

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

// ============================================================================
// V2 Dataset Loading Functions
// ============================================================================

/// Columns in the `LeRobot` dataset schema that we do not visualize in the viewer, and thus ignore.
const LEROBOT_DATASET_IGNORED_COLUMNS: &[&str] =
    &["episode_index", "index", "frame_index", "timestamp"];

pub fn load_and_stream(
    dataset: &LeRobotDataset,
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
    dataset: &LeRobotDataset,
    application_id: &ApplicationId,
    tx: &Sender<LoadedData>,
    loader_name: String,
) -> Vec<(EpisodeIndex, StoreId)> {
    let mut store_ids = vec![];

    for episode_index in dataset.metadata.episodes.keys() {
        let episode = *episode_index;

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
    dataset: &LeRobotDataset,
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
    dataset: &LeRobotDataset,
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
    dataset: &LeRobotDataset,
    observation: &str,
    episode: EpisodeIndex,
    timeline: &Timeline,
    time_column: TimeColumn,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
    let contents = dataset
        .read_episode_video_contents(observation, episode)
        .with_context(|| format!("Reading video contents for episode {episode:?} failed!"))?;

    let video_asset = AssetVideo::new(contents.into_owned());
    let entity_path = observation;

    let video_frame_reference_chunk = match video_asset.read_frame_timestamps_nanos() {
        Ok(frame_timestamps_nanos) => {
            let frame_timestamps_nanos: arrow::buffer::ScalarBuffer<i64> =
                frame_timestamps_nanos.into();

            let video_timestamps = frame_timestamps_nanos
                .iter()
                .take(time_column.num_rows())
                .copied()
                .map(VideoTimestamp::from_nanos)
                .collect::<Vec<_>>();

            let video_frame_reference_column = VideoFrameReference::update_fields()
                .with_many_timestamp(video_timestamps)
                .columns_of_unit_batches()
                .with_context(|| {
                    format!(
                        "Failed to create `VideoFrameReference` column for episode {episode:?}."
                    )
                })?;

            Some(Chunk::from_auto_row_ids(
                re_chunk::ChunkId::new(),
                entity_path.into(),
                std::iter::once((*timeline.name(), time_column)).collect(),
                video_frame_reference_column.collect(),
            )?)
        }
        Err(err) => {
            re_log::warn_once!(
                "Failed to read frame timestamps from episode {episode:?} video: {err}"
            );
            None
        }
    };

    // Put video asset into its own (static) chunk since it can be fairly large.
    let video_asset_chunk = Chunk::builder(entity_path)
        .with_archetype(RowId::new(), TimePoint::default(), &video_asset)
        .build()?;

    if let Some(video_frame_reference_chunk) = video_frame_reference_chunk {
        Ok(Either::Left(
            [video_asset_chunk, video_frame_reference_chunk].into_iter(),
        ))
    } else {
        // Still log the video asset, but don't include video frames.
        Ok(Either::Right(std::iter::once(video_asset_chunk)))
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
