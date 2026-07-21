use crate::lerobot::common::{
    LEROBOT_DATASET_IGNORED_COLUMNS, LeRobotDataset, load_and_stream_versioned,
    load_episode_depth_images, load_episode_images, load_scalar,
};
use crate::lerobot::{
    DType, EpisodeIndex, Feature, LeRobotDatasetSubtask, LeRobotDatasetTask, LeRobotError,
    SubtaskIndex, TaskIndex,
};

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ahash::HashMap;
use anyhow::{Context as _, anyhow};
use arrow::array::{
    ArrayRef, Float64Array, Int64Array, ListArray, RecordBatch, StringArray, StructArray,
};
use arrow::buffer::ScalarBuffer;
use arrow::compute::{cast, concat_batches};
use arrow::datatypes::DataType;
use crossbeam::channel::Sender;
use itertools::Itertools as _;
use parking_lot::RwLock;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use re_chunk::{ArrowArray as _, ChunkId};
use re_video::VideoDataDescription;
use re_video::player::VideoSliceSource;
use serde::{Deserialize, Serialize};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{Chunk, EntityPath, RowId, TimeColumn, TimePoint, Timeline};
use re_log_types::ApplicationId;
use re_sdk_types::archetypes::{TextDocument, VideoStream};

use crate::{ImportedData, ImporterError};

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
    video_cache: RwLock<VideoBlobCache>,
    episode_data_cache: RwLock<HashMap<EpisodeIndex, Arc<RecordBatch>>>,
}

/// Video blob cache with reference counting for automatic eviction.
///
/// Video blobs are lazily loaded from disk when first requested and automatically
/// evicted when all episodes that reference them have been processed.
#[derive(Default)]
struct VideoBlobCache {
    /// Cached video file contents, keyed by full file path.
    blobs: HashMap<PathBuf, Arc<[u8]>>,

    /// Number of episodes that still need each video file.
    /// When a count reaches 0, the corresponding blob is evicted.
    remaining_refs: HashMap<PathBuf, usize>,
}

/// Episode location within a Parquet file
#[derive(Debug, Clone)]
struct EpisodeRowRange {
    start_row: usize,
    end_row: usize,
}

/// Read the string at index `i`, returning `None` if the array is absent or the value is null.
fn value_at(array: Option<&StringArray>, i: usize) -> Option<&str> {
    let a = array?;
    a.is_valid(i).then(|| a.value(i))
}

/// Render the elements of a single `tool_calls` list to text, one element per line.
fn tool_calls_to_text(elements: &dyn arrow::array::Array) -> Option<String> {
    use arrow::util::display::{ArrayFormatter, FormatOptions};
    let formatter = ArrayFormatter::try_new(elements, &FormatOptions::default()).ok()?;
    let joined = (0..elements.len())
        .map(|i| formatter.value(i).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    (!joined.is_empty()).then_some(joined)
}

impl LeRobotDatasetV3 {
    /// Loads a `LeRobotDataset` from a directory.
    ///
    /// This method initializes a dataset by reading its metadata from the `meta/` directory.
    pub fn load_from_directory(path: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let path = path.as_ref();
        let metadatapath = path.join("meta");
        let metadata = LeRobotDatasetMetadataV3::load_from_directory(&metadatapath)?;

        let dataset = Self {
            path: path.to_path_buf(),
            metadata,
            video_cache: RwLock::new(VideoBlobCache::default()),
            episode_data_cache: RwLock::new(HashMap::default()),
        };

        dataset.load_all_episode_data_files()?;
        dataset.init_video_ref_counts();

        Ok(dataset)
    }

    fn load_all_episode_data_files(&self) -> Result<(), LeRobotError> {
        re_tracing::profile_scope!("load_all_episode_data_files");

        // Group episodes by their data file
        let mut files_to_episodes: HashMap<(usize, usize), Vec<EpisodeIndex>> = HashMap::default();
        for episode in self.metadata.episodes.values() {
            files_to_episodes
                .entry((episode.data_chunk_index, episode.data_file_index))
                .or_default()
                .push(episode.episode_index);
        }

        for episodes_in_file in files_to_episodes.into_values() {
            if let Some(first_episode) = episodes_in_file.first() {
                let episode_data = self
                    .metadata
                    .get_episode_data(*first_episode)
                    .ok_or(LeRobotError::InvalidEpisodeIndex(*first_episode))?;
                self.cache_episode_file(episode_data, &episodes_in_file)?;
            }
        }

        Ok(())
    }

    /// Precompute reference counts for all video files across episodes.
    fn init_video_ref_counts(&self) {
        let video_features: Vec<&str> = self
            .metadata
            .info
            .features
            .iter()
            .filter(|(_, feature)| feature.dtype == DType::Video)
            .map(|(key, _)| key.as_str())
            .collect();

        if video_features.is_empty() {
            return;
        }

        let mut cache = self.video_cache.write();
        for episode_data in self.metadata.episodes.values() {
            for feature_key in &video_features {
                if let Ok(video_file) = self.metadata.info.video_path(feature_key, episode_data) {
                    let video_path = self.path.join(video_file);
                    *cache.remaining_refs.entry(video_path).or_insert(0) += 1;
                }
            }
        }

        re_log::debug!(
            "Initialized video cache with {} unique video files across {} episodes",
            cache.remaining_refs.len(),
            self.metadata.episodes.len()
        );
    }

    /// Release video blob references for a completed episode.
    fn release_episode_videos(&self, episode: EpisodeIndex) {
        let Some(episode_data) = self.metadata.get_episode_data(episode) else {
            return;
        };

        let mut cache = self.video_cache.write();
        for (feature_key, feature) in &self.metadata.info.features {
            if feature.dtype != DType::Video {
                continue;
            }

            if let Ok(video_file) = self.metadata.info.video_path(feature_key, episode_data) {
                let video_path = self.path.join(video_file);
                if let Some(count) = cache.remaining_refs.get_mut(&video_path) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        cache.blobs.remove(&video_path);
                        cache.remaining_refs.remove(&video_path);
                    }
                }
            }
        }
    }

    fn cache_episode_file(
        &self,
        file_metadata: &LeRobotEpisodeData,
        episodes_in_file: &[EpisodeIndex],
    ) -> Result<(), LeRobotError> {
        if episodes_in_file.is_empty() {
            return Ok(());
        }

        // Check if already cached
        {
            let cache = self.episode_data_cache.read();
            if episodes_in_file.iter().all(|ep| cache.contains_key(ep)) {
                return Ok(());
            }
        }

        let episode_data_path = self.metadata.info.episode_data_path(file_metadata);
        let episode_parquet_file = self.path.join(&episode_data_path);

        let file = File::open(&episode_parquet_file)
            .map_err(|err| LeRobotError::io(err, episode_parquet_file.clone()))?;

        // Read all data at once
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;
        let batches: Vec<RecordBatch> = reader.try_collect().map_err(LeRobotError::Arrow)?;

        if batches.is_empty() {
            return Ok(());
        }

        let schema = batches[0].schema();
        let full_data = concat_batches(&schema, &batches).map_err(LeRobotError::Arrow)?;

        // Build episode row index in a single pass
        let episode_indices = full_data
            .column_by_name("episode_index")
            .and_then(|c| c.downcast_array_ref::<Int64Array>())
            .ok_or_else(|| {
                LeRobotError::MissingDatasetInfo(
                    "`episode_index` column missing or wrong type".into(),
                )
            })?;

        let row_ranges = Self::build_episode_row_index(episode_indices);

        // Slice out each episode
        let mut cache = self.episode_data_cache.write();
        for &ep_idx in episodes_in_file {
            if cache.contains_key(&ep_idx) {
                continue;
            }

            if let Some(range) = row_ranges.get(&ep_idx) {
                let sliced = full_data.slice(range.start_row, range.end_row - range.start_row);
                cache.insert(ep_idx, Arc::new(sliced));
            }
        }

        Ok(())
    }

    /// Build an index mapping `episode_index` -> row range in a single pass
    fn build_episode_row_index(
        episode_indices: &Int64Array,
    ) -> HashMap<EpisodeIndex, EpisodeRowRange> {
        let mut ranges: HashMap<EpisodeIndex, EpisodeRowRange> = HashMap::default();
        let mut current_episode: Option<i64> = None;
        let mut current_start = 0;

        for (i, ep_idx) in episode_indices.iter().enumerate() {
            let ep_idx = ep_idx.unwrap_or(-1);

            if Some(ep_idx) != current_episode {
                // Finalize previous episode
                if let Some(prev_ep) = current_episode
                    && prev_ep >= 0
                {
                    ranges.insert(
                        EpisodeIndex(prev_ep as usize),
                        EpisodeRowRange {
                            start_row: current_start,
                            end_row: i,
                        },
                    );
                }
                current_episode = Some(ep_idx);
                current_start = i;
            }
        }

        // Don't forget the last episode
        if let Some(ep_idx) = current_episode
            && ep_idx >= 0
        {
            ranges.insert(
                EpisodeIndex(ep_idx as usize),
                EpisodeRowRange {
                    start_row: current_start,
                    end_row: episode_indices.len(),
                },
            );
        }

        ranges
    }

    /// Read the Parquet data file for the provided episode.
    ///
    /// Episode data gets cached eagerly when the dataset loads, so this method mostly returns
    /// clones of cached [`RecordBatch`]es.
    pub fn read_episode_data(&self, episode: EpisodeIndex) -> Result<RecordBatch, LeRobotError> {
        let cache = self.episode_data_cache.read();
        if let Some(cached_data) = cache.get(&episode) {
            return Ok((**cached_data).clone());
        }

        Err(LeRobotError::EmptyEpisode(episode))
    }

    /// Read video feature for the provided episode.
    pub fn read_episode_video_contents(
        &self,
        observation_key: &str,
        episode: EpisodeIndex,
    ) -> Result<Arc<[u8]>, LeRobotError> {
        let episode_data = self
            .metadata
            .get_episode_data(episode)
            .ok_or(LeRobotError::InvalidEpisodeIndex(episode))?;
        let video_file = self
            .metadata
            .info
            .video_path(observation_key, episode_data)?;
        let videopath = self.path.join(video_file);

        // fast path, check whether we already have this video cached
        {
            let cache = self.video_cache.read();
            if let Some(cached_contents) = cache.blobs.get(&videopath) {
                return Ok(Arc::clone(cached_contents));
            }
        }

        let contents = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(&videopath).map_err(|err| LeRobotError::io(err, videopath.clone()))?
        };

        // cache contents of big video blobs, it will be evicted when all episodes that reference it have been processed
        let mut cache = self.video_cache.write();
        if let Some(cached_contents) = cache.blobs.get(&videopath) {
            return Ok(Arc::clone(cached_contents));
        }

        let contents: Arc<[u8]> = Arc::from(contents.into_boxed_slice());
        cache.blobs.insert(videopath, contents.clone());

        Ok(contents)
    }

    /// Retrieve the task using the provided task index.
    pub fn task_by_index(&self, task: TaskIndex) -> Option<&LeRobotDatasetTask> {
        self.metadata.tasks.tasks.get(&task)
    }

    /// Retrieve the subtask using the provided subtask index.
    pub fn subtask_by_index(&self, subtask: SubtaskIndex) -> Option<&LeRobotDatasetSubtask> {
        self.metadata.subtasks.as_ref()?.subtasks.get(&subtask)
    }

    /// Loads a single episode from a `LeRobot` dataset and converts it into a collection of Rerun chunks.
    ///
    /// This function processes an episode from the dataset by extracting the relevant data columns and
    /// converting them into appropriate Rerun data structures. It handles different types of data
    /// (videos, images, scalar values, etc.) based on their data type specifications in the dataset metadata.
    fn load_episode(&self, episode: EpisodeIndex) -> Result<Vec<Chunk>, ImporterError> {
        let data = self
            .read_episode_data(episode)
            .map_err(|err| anyhow!("Reading data for episode {} failed: {err}", episode.0))?;

        let (timeline, time_column) = if let Some(frame_indices) =
            data.column_by_name("frame_index")
        {
            let timeline = re_log_types::Timeline::new_sequence("frame_index");
            let times: &arrow::buffer::ScalarBuffer<i64> = frame_indices
                .downcast_array_ref::<Int64Array>()
                .ok_or_else(|| anyhow!("LeRobot dataset frame indices are of an unexpected type"))?
                .values();
            (
                timeline,
                re_chunk::TimeColumn::new(None, timeline, times.clone()),
            )
        } else if let Some(timestamps) = data.column_by_name("timestamp") {
            let timeline = re_log_types::Timeline::new_duration("timestamp");
            let times: arrow::buffer::ScalarBuffer<i64> = timestamps
                .downcast_array_ref::<Float64Array>()
                .ok_or_else(|| anyhow!("LeRobot dataset timestamps are of an unexpected type"))?
                .values()
                .iter()
                .map(|t| re_log_types::Duration::from_secs(*t).as_nanos())
                .collect();
            (timeline, re_chunk::TimeColumn::new(None, timeline, times))
        } else {
            return Err(
                anyhow!("LeRobot dataset has neither frame_index nor timestamp column").into(),
            );
        };
        let timelines = std::iter::once((*timeline.name(), time_column.clone())).collect();

        let mut chunks = Vec::new();

        for (feature_key, feature) in self
            .metadata
            .info
            .features
            .iter()
            .filter(|(key, _)| !LEROBOT_DATASET_IGNORED_COLUMNS.contains(&key.as_str()))
        {
            match feature.dtype {
                DType::Video => {
                    chunks.extend(self.load_episode_video(
                        feature_key,
                        episode,
                        &timeline,
                        &time_column,
                    )?);
                }

                DType::Image => {
                    let num_channels = feature.channel_dim();

                    match num_channels {
                        1 => {
                            chunks.extend(load_episode_depth_images(
                                feature_key,
                                &timeline,
                                &data,
                            )?);
                        }
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
                    chunks.extend(self.log_episode_task(&timeline, &data)?);
                }
                DType::Int64 if feature_key == "subtask_index" => {
                    // special case int64 subtask_index columns
                    // this always refers to the subtask description in the dataset metadata.
                    chunks.extend(self.log_episode_subtask(&timeline, &data)?);
                }
                DType::Language => {
                    chunks.extend(Self::log_episode_language(feature_key, &timeline, &data)?);
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
        &self,
        timeline: &Timeline,
        data: &RecordBatch,
    ) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, ImporterError> {
        let task_indices = data
            .column_by_name("task_index")
            .and_then(|c| c.downcast_array_ref::<Int64Array>())
            .with_context(|| "Failed to get task_index field from dataset!")?;

        let mut chunk = Chunk::builder("task");
        let mut row_id = RowId::new();

        for (frame_idx, task_index_opt) in task_indices.iter().enumerate() {
            let Some(task_idx) = task_index_opt
                .and_then(|i| usize::try_from(i).ok())
                .map(TaskIndex)
            else {
                continue;
            };

            if let Some(task) = self.task_by_index(task_idx) {
                let frame_idx = i64::try_from(frame_idx)
                    .map_err(|err| anyhow!("Frame index exceeds max value: {err}"))?;

                let timepoint = TimePoint::default().with(*timeline, frame_idx);
                let text = TextDocument::new(task.task.clone());
                chunk = chunk.with_archetype(row_id, timepoint, &text);
                row_id = row_id.next();
            }
        }

        Ok(std::iter::once(chunk.build()?))
    }

    fn log_episode_subtask(
        &self,
        timeline: &Timeline,
        data: &RecordBatch,
    ) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, ImporterError> {
        let subtask_indices = data
            .column_by_name("subtask_index")
            .and_then(|c| c.downcast_array_ref::<Int64Array>())
            .with_context(|| "Failed to get subtask_index field from dataset!")?;

        let mut chunk = Chunk::builder("subtask");
        let mut row_id = RowId::new();

        for (frame_idx, subtask_index_opt) in subtask_indices.iter().enumerate() {
            let Some(subtask_idx) = subtask_index_opt
                .and_then(|i| usize::try_from(i).ok())
                .map(SubtaskIndex)
            else {
                continue;
            };

            if let Some(subtask) = self.subtask_by_index(subtask_idx) {
                let frame_idx = i64::try_from(frame_idx)
                    .map_err(|err| anyhow!("Frame index exceeds max value: {err}"))?;

                let timepoint = TimePoint::default().with(*timeline, frame_idx);
                let text = TextDocument::new(subtask.subtask.clone());
                chunk = chunk.with_archetype(row_id, timepoint, &text);
                row_id = row_id.next();
            }
        }

        Ok(std::iter::once(chunk.build()?))
    }

    /// Log a `LeRobot` v0.6.0+ language feature (`language_persistent` or `language_events`).
    ///
    /// The two columns follow different temporal conventions
    /// ([spec](https://huggingface.co/docs/lerobot/en/language_and_recipes)):
    /// - **Persistent** (`subtask`, `plan`, …): broadcast to every frame. Placed on the frame matching
    ///   the row's `timestamp`; latest-at keeps them active until replaced.
    /// - **Event** (`interjection`, `vqa`): placed on the exact frame they occur.
    ///
    /// Each row becomes a [`TextDocument`] at `{feature}/{style}[/{role}][/{camera}]`; `tool_calls`
    /// go on a `…/tool_calls` sub-entity. Empty columns (e.g. an unused `language_events`) are skipped.
    fn log_episode_language(
        feature_key: &str,
        timeline: &Timeline,
        data: &RecordBatch,
    ) -> Result<Vec<Chunk>, ImporterError> {
        let Some(list) = data
            .column_by_name(feature_key)
            .and_then(|c| c.downcast_array_ref::<ListArray>())
        else {
            return Ok(vec![]);
        };

        // First frame carrying rows: an all-empty column stops here, and for a persistent column
        // this frame holds the full broadcast row set.
        let Some(representative_idx) = (0..list.len()).find(|&i| list.value_length(i) > 0) else {
            return Ok(vec![]);
        };

        let representative_rows = list.value(representative_idx);
        let Some(representative_rows) = representative_rows.downcast_array_ref::<StructArray>()
        else {
            re_log::warn_once!(
                "LeRobot language feature `{feature_key}` is not a list of annotation rows; skipping"
            );
            return Ok(vec![]);
        };

        // Per the spec there are exactly two language columns, so the column name fixes the
        // convention: `language_persistent` broadcasts across every frame, `language_events` sits on
        // the emitting frame. See https://huggingface.co/docs/lerobot/en/language_and_recipes
        let is_persistent = feature_key == "language_persistent";

        // Entity path -> (frame, content) pairs, collected across the relevant frames.
        let mut by_entity: HashMap<String, Vec<(i64, String)>> = HashMap::default();

        if is_persistent {
            // Persistent rows are broadcast identically onto every frame, so the representative frame
            // already holds the full set. Place each on the frame matching its emission `timestamp` —
            // the first frame at or after it — so the text lands on the shared episode timeline.
            let row_timestamps =
                Self::timestamps_as_f64(representative_rows.column_by_name("timestamp"));
            let frame_timestamps = Self::timestamps_as_f64(data.column_by_name("timestamp"));
            // A timestamp past the episode's end (or a missing frame-`timestamp` column) falls back to
            // the last frame rather than frame 0: latest-at would otherwise broadcast an end-of-episode
            // annotation across the whole episode.
            let last_frame = i64::try_from(list.len().saturating_sub(1)).unwrap_or(0);
            let frame_for_timestamp = |ts: f64| -> i64 {
                frame_timestamps
                    .as_ref()
                    .and_then(|frames| frames.values().iter().position(|&t| t >= ts))
                    .and_then(|frame| i64::try_from(frame).ok())
                    .unwrap_or_else(|| {
                        re_log::warn_once!(
                            "No frame at or after language timestamp {ts}s in `{feature_key}`; placing on last frame"
                        );
                        last_frame
                    })
            };
            Self::collect_language_rows(feature_key, representative_rows, &mut by_entity, |i| {
                // A persistent row without its own timestamp is anchored to frame 0.
                row_timestamps
                    .as_ref()
                    .filter(|ts| ts.is_valid(i))
                    .map_or(0, |ts| frame_for_timestamp(ts.value(i)))
            });
        } else {
            // Events live on their exact frame, so place each row on the frame it occupies.
            for frame in 0..list.len() {
                if list.value_length(frame) == 0 {
                    continue;
                }
                let rows = list.value(frame);
                let Some(rows) = rows.downcast_array_ref::<StructArray>() else {
                    continue;
                };
                let frame = i64::try_from(frame).unwrap_or(0);
                Self::collect_language_rows(feature_key, rows, &mut by_entity, |_| frame);
            }
        }

        let mut chunks = Vec::with_capacity(by_entity.len());
        for (entity_path, mut annotations) in by_entity {
            annotations.sort_by_key(|(frame, _)| *frame);

            let mut chunk = Chunk::builder(EntityPath::parse_forgiving(&entity_path));
            let mut row_id = RowId::new();
            for (frame, content) in annotations {
                let timepoint = TimePoint::default().with(*timeline, frame);
                chunk = chunk.with_archetype(row_id, timepoint, &TextDocument::new(content));
                row_id = row_id.next();
            }
            chunks.push(chunk.build()?);
        }

        Ok(chunks)
    }

    /// Read a `timestamp` column as `f64`.
    ///
    /// `LeRobot` types both frame and language-row `timestamp`s as `float32`, but some datasets store
    /// `float64`; casting to a common `f64` lets either load. Returns `None` if the column is absent
    /// or not castable.
    fn timestamps_as_f64(column: Option<&ArrayRef>) -> Option<Float64Array> {
        let casted = cast(column?, &DataType::Float64).ok()?;
        casted.downcast_array_ref::<Float64Array>().cloned()
    }

    /// Collect language annotation rows into per-entity `(frame, content)` pairs.
    ///
    /// Each row is routed to an entity path of `{feature}/{style}[/{role}][/{camera}]` and placed on
    /// the frame returned by `frame_of_row` (its emission frame for persistent rows, or the frame it
    /// occupies for events). Rows with neither `content` nor `tool_calls` produce nothing.
    fn collect_language_rows(
        feature_key: &str,
        rows: &StructArray,
        by_entity: &mut HashMap<String, Vec<(i64, String)>>,
        frame_of_row: impl Fn(usize) -> i64,
    ) {
        let styles = rows
            .column_by_name("style")
            .and_then(|c| c.downcast_array_ref::<StringArray>());
        let contents = rows
            .column_by_name("content")
            .and_then(|c| c.downcast_array_ref::<StringArray>());
        let roles = rows
            .column_by_name("role")
            .and_then(|c| c.downcast_array_ref::<StringArray>());
        let cameras = rows
            .column_by_name("camera")
            .and_then(|c| c.downcast_array_ref::<StringArray>());
        // `tool_calls` is a `list<struct>` of OpenAI-style function calls (real on-disk shape:
        // `list<struct<type, function<name, arguments<…>>>>`); we render each element generically.
        let tool_calls = rows
            .column_by_name("tool_calls")
            .and_then(|c| c.downcast_array_ref::<ListArray>());

        for i in 0..rows.len() {
            // Key by the present distinguishing fields, in resolver order (style, role, camera).
            // `style` and `camera` are nullable (e.g. `say` speech rows have no style), so we simply
            // omit any absent segment rather than inventing a placeholder.
            let mut entity_path = feature_key.to_owned();
            for segment in [
                value_at(styles, i),
                value_at(roles, i),
                value_at(cameras, i),
            ]
            .into_iter()
            .flatten()
            {
                entity_path.push('/');
                entity_path.push_str(segment);
            }

            let frame = frame_of_row(i);

            // Tool calls (if any) go on a `…/tool_calls` sub-entity.
            if let Some(tool_calls) = tool_calls
                && tool_calls.is_valid(i)
                && tool_calls.value_length(i) > 0
                && let Some(text) = tool_calls_to_text(tool_calls.value(i).as_ref())
            {
                by_entity
                    .entry(format!("{entity_path}/tool_calls"))
                    .or_default()
                    .push((frame, text));
            }

            if let Some(content) = value_at(contents, i) {
                by_entity
                    .entry(entity_path)
                    .or_default()
                    .push((frame, content.to_owned()));
            }
        }
    }

    /// Extract feature-specific timestamp metadata for a given episode and observation.
    ///
    /// Returns (`start_time`, `end_time`) in seconds, defaulting to (0.0, 0.0) if not found.
    fn get_feature_timestamps(&self, episode: EpisodeIndex, observation: &str) -> (f64, f64) {
        self.metadata
            .get_episode_data(episode)
            .and_then(|ep_data| ep_data.feature_files.get(observation))
            .map(|file_meta| {
                (
                    file_meta.from_timestamp.unwrap_or(0.0),
                    file_meta.to_timestamp.unwrap_or(0.0),
                )
            })
            .unwrap_or((0.0, 0.0))
    }

    fn load_episode_video(
        &self,
        observation: &str,
        episode: EpisodeIndex,
        timeline: &Timeline,
        time_column: &TimeColumn,
    ) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, ImporterError> {
        let contents = self
            .read_episode_video_contents(observation, episode)
            .with_context(|| format!("Reading video contents for episode {episode:?} failed!"))?;

        let entity_path = observation;
        let video_bytes: &[u8] = &contents;

        // Parse the video to get its structure
        let video = VideoDataDescription::load_from_bytes(video_bytes, "video/mp4", observation)
            .map_err(|err| {
                anyhow!("Failed to read video data description for feature '{observation}': {err}")
            })?;

        let (start_time, end_time) = self.get_feature_timestamps(episode, observation);

        if video.samples.is_empty() {
            return Err(ImporterError::Other(anyhow!(
                "Video feature '{observation}' for episode {episode:?} did not contain any samples"
            )));
        }

        // Convert timestamps to video time
        let timescale = video.timescale.ok_or_else(|| {
            anyhow!("Video feature '{observation}' is missing timescale information")
        })?;

        let start_video_time = re_video::Time::from_secs(start_time, timescale);
        let end_video_time = re_video::Time::from_secs(end_time, timescale);

        // Find the GOPs that contain our time range
        let start_keyframe = video
            .presentation_time_keyframe_index(start_video_time)
            .unwrap_or(0);

        let end_keyframe = video
            .presentation_time_keyframe_index(end_video_time)
            .or_else(|| video.keyframe_indices.len().checked_sub(1))
            .ok_or(ImporterError::Other(anyhow!("No keyframes in the video")))?;

        // Determine the sample range to extract from the video
        let start_sample = video
            .gop_sample_range_for_keyframe(start_keyframe)
            .ok_or(ImporterError::Other(anyhow!("Bad video data")))?
            .start;

        let end_sample = video
            .gop_sample_range_for_keyframe(end_keyframe)
            .ok_or(ImporterError::Other(anyhow!("Bad video data")))?
            .end;

        let sample_range = start_sample..end_sample;

        // Extract all video samples in this range
        let mut samples = Vec::with_capacity(sample_range.len());

        for (sample_idx, sample_meta) in video.samples.iter_index_range_clamped(&sample_range) {
            let Some(sample_meta) = sample_meta.sample() else {
                continue;
            };

            // make sure we absolutely do not leak any samples from outside the requested time range
            if sample_meta.presentation_timestamp < start_video_time
                || sample_meta.presentation_timestamp >= end_video_time
            {
                continue;
            }

            let chunk = sample_meta
                .get(&VideoSliceSource(video_bytes), sample_idx)
                .ok_or_else(|| {
                    anyhow!("Sample {sample_idx} out of bounds for feature '{observation}'")
                })?;

            let sample_bytes = video
            .sample_data_in_stream_format(&chunk)
            .with_context(|| {
                format!(
                    "Failed to convert sample {sample_idx} for feature '{observation}' to the expected codec stream format"
                )
            })?;

            samples.push((sample_meta.clone(), sample_bytes));
        }

        let (samples_meta, samples): (Vec<_>, Vec<_>) = samples.into_iter().unzip();

        let samples_column = VideoStream::update_fields()
            .with_many_sample(samples)
            .columns_of_unit_batches()
            .with_context(|| "Failed to create VideoStream")?;

        // Map video samples to episode frame indices
        //
        // Video samples may not align 1:1 with episode frames. We distribute samples uniformly
        // across the frame timeline. When there are more samples than frames, multiple samples
        // map to the same frame index; when there are fewer samples, some frames have no samples.
        let num_samples = samples_meta.len();
        let frame_count = time_column.num_rows();

        let uniform_times: Vec<i64> = (0..num_samples)
            .map(|i| i64::try_from((i * frame_count) / num_samples).unwrap_or_default())
            .collect();

        let uniform_time_column = TimeColumn::new(
            Some(true), // is_sorted
            *timeline,
            ScalarBuffer::from(uniform_times),
        );

        let codec =
            re_sdk_types::components::VideoCodec::try_from(video.codec.clone()).map_err(|err| {
                anyhow!(
                    "Unsupported video codec {:?} for feature: '{observation}': {err}",
                    video.codec
                )
            })?;

        let codec_chunk = Chunk::builder(entity_path)
            .with_archetype(
                RowId::new(),
                TimePoint::default(),
                &VideoStream::update_fields().with_codec(codec),
            )
            .build()?;

        let samples_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.into(),
            std::iter::once((timeline.name().to_owned(), uniform_time_column)).collect(),
            samples_column.collect(),
        )?;

        Ok([samples_chunk, codec_chunk].into_iter())
    }
}

impl LeRobotDataset for LeRobotDatasetV3 {
    fn iter_episode_indices(&self) -> impl std::iter::Iterator<Item = EpisodeIndex> {
        self.metadata.iter_episode_indices()
    }

    fn load_episode_chunks(&self, episode: EpisodeIndex) -> Result<Vec<Chunk>, ImporterError> {
        let result = self.load_episode(episode);

        // Release video blob references for this episode regardless of success or failure to avoid leaking memory if we fail to load an episode after caching its video blobs.
        self.release_episode_videos(episode);

        result
    }
}

/// Metadata for a `LeRobot` dataset.
///
/// This is a wrapper struct for the metadata files in the `meta` directory of a
/// `LeRobot` dataset. For more see [`LeRobotDatasetV3`].
pub struct LeRobotDatasetMetadataV3 {
    pub info: LeRobotDatasetInfoV3,
    pub tasks: LeRobotDatasetV3Tasks,
    pub subtasks: Option<LeRobotDatasetV3Subtasks>,
    pub episodes: HashMap<EpisodeIndex, LeRobotEpisodeData>,
}

impl LeRobotDatasetMetadataV3 {
    /// Get the number of episodes in the dataset.
    pub fn episode_count(&self) -> usize {
        self.episodes.len()
    }

    /// Get episode data by index.
    pub fn get_episode_data(&self, episode: EpisodeIndex) -> Option<&LeRobotEpisodeData> {
        self.episodes.get(&episode)
    }

    /// Iterate over the indices of all episodes in the dataset.
    pub fn iter_episode_indices(&self) -> impl Iterator<Item = EpisodeIndex> + '_ {
        self.episodes.values().map(|episode| episode.episode_index)
    }

    /// Loads all metadata files from the provided directory.
    ///
    /// This method reads dataset metadata from JSON and Parquet files stored in the `meta/` directory.
    /// It retrieves general dataset information, a list of recorded episodes, and defined tasks.
    pub fn load_from_directory(metadir: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let metadir = metadir.as_ref();

        let episode_data = LeRobotEpisodeData::load_from_directory(metadir.join("episodes"))?;
        let info = LeRobotDatasetInfoV3::load_from_json_file(metadir.join("info.json"))?;
        let tasks = LeRobotDatasetV3Tasks::load_from_parquet_file(metadir.join("tasks.parquet"))?;

        let subtasks_path = metadir.join("subtasks.parquet");
        let subtasks = if subtasks_path.is_file() {
            Some(LeRobotDatasetV3Subtasks::load_from_parquet_file(
                subtasks_path,
            )?)
        } else {
            None
        };

        // Convert episode data Vec to HashMap for O(1) lookups
        let episodes = episode_data
            .into_iter()
            .map(|ep| (ep.episode_index, ep))
            .collect();

        Ok(Self {
            info,
            tasks,
            subtasks,
            episodes,
        })
    }
}

/// File metadata for a specific feature (video or image) in a `LeRobot` dataset.
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
    pub from_timestamp: Option<f64>,

    /// End timestamp for the feature data in this file
    pub to_timestamp: Option<f64>,
}

/// Episode metadata for a `LeRobot` v3 dataset.
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

    /// File metadata for video/image features, keyed by feature name (e.g., `observation.images.cam_high`)
    pub feature_files: HashMap<String, FeatureFileMetadata>,
}

impl LeRobotEpisodeData {
    fn load_from_directory(metadir: impl AsRef<Path>) -> Result<Vec<Self>, LeRobotError> {
        // Walk all subdirectories and load episode data files.
        let metadir = metadir.as_ref();
        let mut all_episodes = vec![];
        for entry in std::fs::read_dir(metadir).map_err(|err| LeRobotError::io(err, metadir))? {
            let entry = entry.map_err(|err| LeRobotError::io(err, metadir))?;
            let path = entry.path();
            let path = path.as_path();

            re_log::trace!("Loading episode metadata from: {path:?}");

            if path.is_dir() {
                for chunk_entry in
                    std::fs::read_dir(path).map_err(|err| LeRobotError::io(err, path))?
                {
                    let chunk_entry = chunk_entry.map_err(|err| LeRobotError::io(err, path))?;
                    let chunk_path = chunk_entry.path();

                    if chunk_path.is_file() {
                        let chunk_parquet = ParquetRecordBatchReaderBuilder::try_new(
                            File::open(&chunk_path)
                                .map_err(|err| LeRobotError::io(err, chunk_path.clone()))?,
                        )?
                        .build()?;

                        let episode_data: Vec<_> = chunk_parquet
                            .filter_map(|batch| {
                                let batch = batch.ok()?;

                                let episode_index = batch
                                    .column_by_name("episode_index")?
                                    .as_any()
                                    .downcast_ref::<Int64Array>()?;

                                let data_chunk_index = batch
                                    .column_by_name("data/chunk_index")?
                                    .as_any()
                                    .downcast_ref::<Int64Array>()?;

                                let data_file_index = batch
                                    .column_by_name("data/file_index")?
                                    .as_any()
                                    .downcast_ref::<Int64Array>()?;

                                Some(Self::collect_episode_data(
                                    &batch,
                                    episode_index,
                                    data_chunk_index,
                                    data_file_index,
                                ))
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

    fn collect_episode_data(
        batch: &RecordBatch,
        episode_index: &Int64Array,
        data_chunk_index: &Int64Array,
        data_file_index: &Int64Array,
    ) -> Vec<Self> {
        // Parse feature-specific file metadata (videos, images)
        // Pattern: "videos/{feature_name}/{field}" where field is chunk_index, file_index, from_timestamp, to_timestamp
        let feature_metadata = Self::parse_feature_metadata(batch);

        let mut episodes = Vec::with_capacity(batch.num_rows());
        for i in 0..batch.num_rows() {
            // Build feature_files map for this episode
            let feature_files = feature_metadata
                .iter()
                .filter_map(|(feature_name, metadata)| {
                    // Only include if both chunk_index and file_index are present
                    let chunk_index = metadata.chunk_index.as_ref()?;
                    let file_index = metadata.file_index.as_ref()?;

                    Some((
                        feature_name.to_string(),
                        FeatureFileMetadata {
                            chunk_index: chunk_index.value(i) as usize,
                            file_index: file_index.value(i) as usize,
                            from_timestamp: metadata.from_timestamp.as_ref().and_then(
                                |timestamps| timestamps.is_valid(i).then(|| timestamps.value(i)),
                            ),
                            to_timestamp: metadata.to_timestamp.as_ref().and_then(|timestamps| {
                                timestamps.is_valid(i).then(|| timestamps.value(i))
                            }),
                        },
                    ))
                })
                .collect();

            episodes.push(Self {
                episode_index: EpisodeIndex(episode_index.value(i) as usize),
                data_chunk_index: data_chunk_index.value(i) as usize,
                data_file_index: data_file_index.value(i) as usize,
                feature_files,
            });
        }
        episodes
    }

    /// Parse feature-specific file metadata from a [`RecordBatch`].
    ///
    /// Looks for columns matching pattern `videos/{feature_name}/{field}`
    /// and groups them by feature name.
    fn parse_feature_metadata(batch: &RecordBatch) -> HashMap<Arc<str>, FeatureMetadataColumns> {
        let mut features: HashMap<Arc<str>, FeatureMetadataColumns> = HashMap::default();
        let schema = batch.schema();

        for field in schema.fields() {
            let column_name = field.name();

            // Look for columns like "videos/{feature_name}/chunk_index"
            if let Some(rest) = column_name.strip_prefix("videos/")
                && let Some((feature_name, field_name)) = rest.rsplit_once('/')
            {
                let entry = features.entry(Arc::from(feature_name)).or_default();

                match field_name {
                    "chunk_index" => {
                        if let Some(col) = batch
                            .column_by_name(column_name)
                            .and_then(|c| c.downcast_array_ref::<Int64Array>())
                        {
                            entry.chunk_index = Some(col.clone());
                        }
                    }
                    "file_index" => {
                        if let Some(col) = batch
                            .column_by_name(column_name)
                            .and_then(|c| c.downcast_array_ref::<Int64Array>())
                        {
                            entry.file_index = Some(col.clone());
                        }
                    }
                    "from_timestamp" => {
                        if let Some(col) = batch
                            .column_by_name(column_name)
                            .and_then(|c| c.downcast_array_ref::<Float64Array>())
                        {
                            entry.from_timestamp = Some(col.clone());
                        }
                    }
                    "to_timestamp" => {
                        if let Some(col) = batch
                            .column_by_name(column_name)
                            .and_then(|c| c.downcast_array_ref::<Float64Array>())
                        {
                            entry.to_timestamp = Some(col.clone());
                        }
                    }
                    _ => {} // Ignore unknown fields
                }
            }
        }

        features
    }
}

/// Structure to hold Arrow arrays for feature metadata during parsing.
#[derive(Default)]
struct FeatureMetadataColumns {
    chunk_index: Option<Int64Array>,
    file_index: Option<Int64Array>,
    from_timestamp: Option<Float64Array>,
    to_timestamp: Option<Float64Array>,
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
    pub fps: f32,

    /// A mapping of feature names to their respective [`Feature`] definitions.
    pub features: HashMap<String, Feature>,
}

impl LeRobotDatasetInfoV3 {
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

pub struct LeRobotDatasetV3Tasks {
    pub tasks: HashMap<TaskIndex, LeRobotDatasetTask>,
}

impl LeRobotDatasetV3Tasks {
    pub fn load_from_parquet_file(filepath: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let filepath = filepath.as_ref().to_owned();
        let parquet_data =
            File::open(&filepath).map_err(|err| LeRobotError::io(err, filepath.clone()))?;

        let reader = ParquetRecordBatchReaderBuilder::try_new(parquet_data)?.build()?;

        let tasks = reader
            .filter_map(|record_batch| {
                let b = record_batch.ok()?;
                let task_index_col = b.column_by_name("task_index")?;
                let task_col = b.column_by_name("__index_level_0__")?;
                let task_index = task_index_col.as_any().downcast_ref::<Int64Array>()?;
                let task = task_col.as_any().downcast_ref::<StringArray>()?;

                let num_rows = b.num_rows();
                Some(
                    (0..num_rows)
                        .map(move |i| {
                            (
                                TaskIndex(task_index.value(i) as usize),
                                LeRobotDatasetTask {
                                    index: TaskIndex(task_index.value(i) as usize),
                                    task: task.value(i).to_owned(),
                                },
                            )
                        })
                        .collect(),
                )
            })
            .flat_map(|e: Vec<(TaskIndex, LeRobotDatasetTask)>| e)
            .collect::<HashMap<_, _>>();

        Ok(Self { tasks })
    }
}

pub struct LeRobotDatasetV3Subtasks {
    pub subtasks: HashMap<SubtaskIndex, LeRobotDatasetSubtask>,
}

impl LeRobotDatasetV3Subtasks {
    pub fn load_from_parquet_file(filepath: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let filepath = filepath.as_ref().to_owned();
        let parquet_data =
            File::open(&filepath).map_err(|err| LeRobotError::io(err, filepath.clone()))?;

        let reader = ParquetRecordBatchReaderBuilder::try_new(parquet_data)?.build()?;

        let subtasks = reader
            .filter_map(|record_batch| {
                let b = record_batch.ok()?;
                let subtask_index_col = b.column_by_name("subtask_index")?;
                let subtask_col = b.column_by_name("subtask")?;
                let subtask_index = subtask_index_col.as_any().downcast_ref::<Int64Array>()?;
                let subtask = subtask_col.as_any().downcast_ref::<StringArray>()?;

                let num_rows = b.num_rows();
                Some(
                    (0..num_rows)
                        .map(move |i| {
                            (
                                SubtaskIndex(subtask_index.value(i) as usize),
                                LeRobotDatasetSubtask {
                                    index: SubtaskIndex(subtask_index.value(i) as usize),
                                    subtask: subtask.value(i).to_owned(),
                                },
                            )
                        })
                        .collect(),
                )
            })
            .flat_map(|e: Vec<(SubtaskIndex, LeRobotDatasetSubtask)>| e)
            .collect::<HashMap<_, _>>();

        Ok(Self { subtasks })
    }
}

pub fn load_and_stream(
    dataset: &LeRobotDatasetV3,
    application_id: &ApplicationId,
    tx: &Sender<ImportedData>,
    loader_name: &str,
) {
    load_and_stream_versioned(dataset, application_id, tx, loader_name);
}

#[cfg(test)]
mod tests {
    use super::*;

    use arrow::array::{Float32Array, ListArray, RecordBatchOptions};
    use arrow::buffer::OffsetBuffer;
    use arrow::datatypes::{DataType, Field, Fields, Schema};
    use std::sync::Arc;

    /// Build a single `RecordBatch` from fields and columns, using the metadata/options-aware
    /// constructors our clippy config mandates.
    fn test_batch(fields: Vec<Field>, columns: Vec<ArrayRef>) -> RecordBatch {
        let schema = Schema::new_with_metadata(fields, Default::default());
        RecordBatch::try_new_with_options(Arc::new(schema), columns, &RecordBatchOptions::default())
            .unwrap()
    }

    /// A single `LeRobot` language annotation row, used to build synthetic test data.
    struct Row {
        style: Option<&'static str>,
        content: Option<&'static str>,
        role: Option<&'static str>,
        camera: Option<&'static str>,
        timestamp: Option<f64>,
        tool_calls: Option<Vec<&'static str>>,
    }

    /// Build a `list<struct>` language column from per-frame rows.
    fn language_column(frames: &[Vec<Row>]) -> ListArray {
        let all: Vec<&Row> = frames.iter().flatten().collect();

        // Build `tool_calls` in its real on-disk shape — an OpenAI-style function-call struct,
        // `list<struct<function<name, arguments<text>>, type>>` — rather than flattening it to a
        // list of strings, so the test drives the importer's generic struct rendering. Each call is
        // a `say` function (the catalog's canonical/default tool) whose `arguments.text` is the
        // spoken utterance. See https://huggingface.co/docs/lerobot/en/tools
        let say_texts: Vec<&str> = all
            .iter()
            .flat_map(|r| r.tool_calls.clone().unwrap_or_default())
            .collect();
        let arguments_fields = Fields::from(vec![Field::new("text", DataType::Utf8, true)]);
        let function_fields = Fields::from(vec![
            Field::new(
                "arguments",
                DataType::Struct(arguments_fields.clone()),
                true,
            ),
            Field::new("name", DataType::Utf8, true),
        ]);
        let tool_call_fields = Fields::from(vec![
            Field::new("function", DataType::Struct(function_fields.clone()), true),
            Field::new("type", DataType::Utf8, true),
        ]);
        let arguments = StructArray::new(
            arguments_fields,
            vec![Arc::new(
                say_texts.iter().map(|t| Some(*t)).collect::<StringArray>(),
            )],
            None,
        );
        let function = StructArray::new(
            function_fields,
            vec![
                Arc::new(arguments),
                Arc::new(
                    say_texts
                        .iter()
                        .map(|_| Some("say"))
                        .collect::<StringArray>(),
                ),
            ],
            None,
        );
        let tool_call_values = StructArray::new(
            tool_call_fields.clone(),
            vec![
                Arc::new(function),
                Arc::new(
                    say_texts
                        .iter()
                        .map(|_| Some("function"))
                        .collect::<StringArray>(),
                ),
            ],
            None,
        );
        let tool_calls_item = Field::new("item", DataType::Struct(tool_call_fields), true);
        let tool_calls = ListArray::new(
            Arc::new(tool_calls_item.clone()),
            OffsetBuffer::from_lengths(
                all.iter()
                    .map(|r| r.tool_calls.as_ref().map_or(0, Vec::len)),
            ),
            Arc::new(tool_call_values),
            None,
        );

        // `timestamp` is `float32` in the source schema (some datasets store `float64`) — use
        // `float32` here so the importer's float32→f64 handling is exercised.
        let struct_fields = Fields::from(vec![
            Field::new("style", DataType::Utf8, true),
            Field::new("content", DataType::Utf8, true),
            Field::new("role", DataType::Utf8, true),
            Field::new("camera", DataType::Utf8, true),
            Field::new("timestamp", DataType::Float32, true),
            Field::new(
                "tool_calls",
                DataType::List(Arc::new(tool_calls_item)),
                true,
            ),
        ]);
        let values = StructArray::new(
            struct_fields.clone(),
            vec![
                Arc::new(all.iter().map(|r| r.style).collect::<StringArray>()),
                Arc::new(all.iter().map(|r| r.content).collect::<StringArray>()),
                Arc::new(all.iter().map(|r| r.role).collect::<StringArray>()),
                Arc::new(all.iter().map(|r| r.camera).collect::<StringArray>()),
                Arc::new(
                    all.iter()
                        .map(|r| r.timestamp.map(|t| t as f32))
                        .collect::<Float32Array>(),
                ),
                Arc::new(tool_calls),
            ],
            None,
        );
        let offsets = OffsetBuffer::from_lengths(frames.iter().map(Vec::len));
        let item = Field::new("item", DataType::Struct(struct_fields), true);
        ListArray::new(Arc::new(item), offsets, Arc::new(values), None)
    }

    fn entity_paths(chunks: &[Chunk]) -> Vec<String> {
        let mut paths: Vec<_> = chunks.iter().map(|c| c.entity_path().to_string()).collect();
        paths.sort();
        paths
    }

    fn chunk<'a>(chunks: &'a [Chunk], entity_path: &str) -> &'a Chunk {
        chunks
            .iter()
            .find(|c| c.entity_path().to_string() == entity_path)
            .unwrap_or_else(|| panic!("missing chunk for `{entity_path}`"))
    }

    /// Render every logged component value in a chunk to text (for asserting on document contents).
    fn rendered_text(chunk: &Chunk) -> String {
        use arrow::util::display::{ArrayFormatter, FormatOptions};
        chunk
            .components()
            .values()
            .flat_map(|col| {
                let formatter =
                    ArrayFormatter::try_new(&col.list_array, &FormatOptions::default()).unwrap();
                (0..col.list_array.len())
                    .map(|i| formatter.value(i).to_string())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Persistent rows carry a timestamp, are broadcast to every frame, and are placed on the frame
    /// matching their emission timestamp. View-dependent (`camera`) and role-paired rows must not
    /// collapse onto the same entity.
    #[test]
    fn language_persistent_broadcast_and_keying() {
        // Three frames, each broadcasting the same three persistent rows.
        let rows = || {
            vec![
                Row {
                    style: Some("subtask"),
                    content: Some("pick"),
                    role: Some("assistant"),
                    camera: None,
                    timestamp: Some(0.0),
                    tool_calls: None,
                },
                Row {
                    style: Some("subtask"),
                    content: Some("place"),
                    role: Some("assistant"),
                    camera: None,
                    timestamp: Some(2.0),
                    tool_calls: None,
                },
                Row {
                    style: Some("vqa"),
                    content: Some("what is it?"),
                    role: Some("user"),
                    camera: Some("observation.images.top"),
                    timestamp: Some(1.0),
                    tool_calls: None,
                },
            ]
        };
        let frames = vec![rows(), rows(), rows()];

        let batch = test_batch(
            vec![
                Field::new("timestamp", DataType::Float32, false),
                Field::new(
                    "language_persistent",
                    language_column(&frames).data_type().clone(),
                    true,
                ),
            ],
            vec![
                Arc::new(Float32Array::from(vec![0.0, 1.0, 2.0])),
                Arc::new(language_column(&frames)),
            ],
        );

        let timeline = Timeline::new_sequence("frame_index");
        let chunks =
            LeRobotDatasetV3::log_episode_language("language_persistent", &timeline, &batch)
                .unwrap();

        assert_eq!(
            entity_paths(&chunks),
            vec![
                "/language_persistent/subtask/assistant".to_owned(),
                "/language_persistent/vqa/user/observation.images.top".to_owned(),
            ]
        );
        // Two subtasks placed at frames 0 (ts 0.0) and 2 (ts 2.0).
        assert_eq!(
            chunk(&chunks, "/language_persistent/subtask/assistant").num_rows(),
            2
        );
        // One vqa query at frame 1 (ts 1.0), kept separate by role + camera.
        assert_eq!(
            chunk(
                &chunks,
                "/language_persistent/vqa/user/observation.images.top"
            )
            .num_rows(),
            1
        );
    }

    /// Event rows omit the timestamp and exist only on the exact frame they were emitted on, so each
    /// must be placed on the frame it occupies rather than broadcast.
    #[test]
    fn language_events_placed_per_frame() {
        let frames = vec![
            vec![],
            vec![Row {
                style: Some("interjection"),
                content: Some("stop!"),
                role: Some("assistant"),
                camera: None,
                timestamp: None,
                tool_calls: None,
            }],
            vec![],
        ];

        let batch = test_batch(
            vec![
                Field::new("timestamp", DataType::Float32, false),
                Field::new(
                    "language_events",
                    language_column(&frames).data_type().clone(),
                    true,
                ),
            ],
            vec![
                Arc::new(Float32Array::from(vec![0.0, 1.0, 2.0])),
                Arc::new(language_column(&frames)),
            ],
        );

        let timeline = Timeline::new_sequence("frame_index");
        let chunks =
            LeRobotDatasetV3::log_episode_language("language_events", &timeline, &batch).unwrap();

        assert_eq!(
            entity_paths(&chunks),
            vec!["/language_events/interjection/assistant".to_owned()]
        );
        assert_eq!(
            chunk(&chunks, "/language_events/interjection/assistant").num_rows(),
            1
        );
    }

    /// A pure tool-call event row (null `content`, non-empty `tool_calls`) is not dropped: its calls
    /// are captured as text on a `…/tool_calls` sub-entity.
    #[test]
    fn language_tool_calls_captured_on_sub_entity() {
        let frames = vec![
            vec![],
            vec![Row {
                style: None, // speech rows carry no style
                content: None,
                role: Some("assistant"),
                camera: None,
                timestamp: None,
                tool_calls: Some(vec!["hello there"]), // the `say` text
            }],
        ];

        let batch = test_batch(
            vec![
                Field::new("timestamp", DataType::Float32, false),
                Field::new(
                    "language_events",
                    language_column(&frames).data_type().clone(),
                    true,
                ),
            ],
            vec![
                Arc::new(Float32Array::from(vec![0.0, 1.0])),
                Arc::new(language_column(&frames)),
            ],
        );

        let timeline = Timeline::new_sequence("frame_index");
        let chunks =
            LeRobotDatasetV3::log_episode_language("language_events", &timeline, &batch).unwrap();

        // No content entity (content was null), only the tool-call sub-entity. Style is null (speech
        // rows carry none), so the path omits the style segment rather than inventing a placeholder.
        assert_eq!(
            entity_paths(&chunks),
            vec!["/language_events/assistant/tool_calls".to_owned()]
        );
        let tool_call_chunk = chunk(&chunks, "/language_events/assistant/tool_calls");
        assert_eq!(tool_call_chunk.num_rows(), 1);
        // The generic struct render keeps the `say` text, even if it's buried in the struct dump.
        assert!(
            rendered_text(tool_call_chunk).contains("hello there"),
            "rendered tool call should retain the say text, got: {}",
            rendered_text(tool_call_chunk)
        );
    }

    /// An all-null / empty language column (e.g. an unused `language_events`) yields no chunks.
    #[test]
    fn language_empty_column_is_skipped() {
        let frames: Vec<Vec<Row>> = vec![vec![], vec![], vec![]];
        let batch = test_batch(
            vec![Field::new(
                "language_events",
                language_column(&frames).data_type().clone(),
                true,
            )],
            vec![Arc::new(language_column(&frames))],
        );

        let timeline = Timeline::new_sequence("frame_index");
        let chunks =
            LeRobotDatasetV3::log_episode_language("language_events", &timeline, &batch).unwrap();
        assert!(chunks.is_empty());
    }

    /// The `style`/`role`/`camera` path segments are dataset-authored strings we don't control, so
    /// they can be empty or contain characters (`/`, spaces, `!`) that are meaningful in an entity
    /// path. `EntityPath::parse_forgiving` must absorb these without panicking, and degrade sanely:
    /// empty segments collapse (dropped duplicate slash), an embedded `/` just nests deeper, and
    /// other characters get escaped.
    #[test]
    fn language_edge_case_path_segments_are_forgiving() {
        let frames = vec![
            vec![],
            vec![
                // Empty style: the `//` it would produce collapses to a single separator.
                Row {
                    style: Some(""),
                    content: Some("empty style"),
                    role: Some("assistant"),
                    camera: None,
                    timestamp: None,
                    tool_calls: None,
                },
                // A `/` inside a segment splits into extra path parts rather than erroring.
                Row {
                    style: Some("vqa"),
                    content: Some("slashy camera"),
                    role: Some("user"),
                    camera: Some("observation/images/top"),
                    timestamp: None,
                    tool_calls: None,
                },
                // Spaces and `!` are escaped, not rejected.
                Row {
                    style: Some("pick up!"),
                    content: Some("special chars"),
                    role: None,
                    camera: None,
                    timestamp: None,
                    tool_calls: None,
                },
            ],
        ];

        let batch = test_batch(
            vec![
                Field::new("timestamp", DataType::Float32, false),
                Field::new(
                    "language_events",
                    language_column(&frames).data_type().clone(),
                    true,
                ),
            ],
            vec![
                Arc::new(Float32Array::from(vec![0.0, 1.0])),
                Arc::new(language_column(&frames)),
            ],
        );

        let timeline = Timeline::new_sequence("frame_index");
        // Must not panic on any of the awkward segments.
        let chunks =
            LeRobotDatasetV3::log_episode_language("language_events", &timeline, &batch).unwrap();

        let paths = entity_paths(&chunks);
        // Empty style drops the duplicate slash; the `/` in the camera nests deeper; the special
        // characters survive (escaped) rather than breaking the path. Three distinct entities.
        assert_eq!(
            paths,
            vec![
                "/language_events/assistant".to_owned(),
                "/language_events/pick\\ up\\!".to_owned(),
                "/language_events/vqa/user/observation/images/top".to_owned(),
            ]
        );
    }

    /// Manual verification against a real `LeRobot` v0.6.0 dataset parquet (e.g.
    /// `pepijn223/human_new_35_annotated`). Ignored by default; run with:
    ///   `LEROBOT_LANG_PARQUET=/path/to/file-000.parquet cargo test -p re_importer --all-features
    ///   verify_real_language_parquet -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a local LeRobot v0.6.0 parquet via LEROBOT_LANG_PARQUET"]
    fn verify_real_language_parquet() {
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

        let path = std::env::var("LEROBOT_LANG_PARQUET").expect("set LEROBOT_LANG_PARQUET");
        let file = std::fs::File::open(&path).unwrap();
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)
            .unwrap()
            .build()
            .unwrap();
        let batches: Vec<RecordBatch> = reader.map(Result::unwrap).collect();
        let batch = concat_batches(&batches[0].schema(), &batches).unwrap();

        let timeline = Timeline::new_sequence("frame_index");
        for feature in ["language_persistent", "language_events"] {
            if batch.schema().index_of(feature).is_err() {
                println!("\n===== {feature}: not present =====");
                continue;
            }
            let chunks =
                LeRobotDatasetV3::log_episode_language(feature, &timeline, &batch).unwrap();
            println!("\n===== {feature}: {} entities =====", chunks.len());
            for c in &chunks {
                let preview: String = rendered_text(c)
                    .replace('\n', " ")
                    .chars()
                    .take(160)
                    .collect();
                println!(
                    "  {} ({} rows)\n      {preview}",
                    c.entity_path(),
                    c.num_rows()
                );
            }
        }
    }
}
