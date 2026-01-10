use crate::lerobot::common::{
    LEROBOT_DATASET_IGNORED_COLUMNS, LeRobotDataset, load_and_stream_versioned,
    load_episode_depth_images, load_episode_images, load_scalar,
};
use crate::lerobot::{DType, EpisodeIndex, Feature, LeRobotDatasetTask, LeRobotError, TaskIndex};

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc::Sender};

use ahash::HashMap;
use anyhow::{Context as _, anyhow};
use arrow::array::{Float64Array, Int64Array, RecordBatch, StringArray};
use arrow::buffer::ScalarBuffer;
use arrow::compute::concat_batches;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use re_chunk::{ArrowArray as _, ChunkId};
use re_video::VideoDataDescription;
use serde::{Deserialize, Serialize};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{Chunk, RowId, TimeColumn, TimePoint, Timeline};
use re_log_types::ApplicationId;
use re_sdk_types::archetypes::{TextDocument, VideoStream};

use crate::{DataLoaderError, LoadedData};

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
    video_cache: parking_lot::RwLock<HashMap<PathBuf, Arc<[u8]>>>,
    episode_data_cache: parking_lot::RwLock<HashMap<EpisodeIndex, Arc<RecordBatch>>>,
}

/// Episode location within a Parquet file
#[derive(Debug, Clone)]
struct EpisodeRowRange {
    start_row: usize,
    end_row: usize,
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
            video_cache: parking_lot::RwLock::new(HashMap::default()),
            episode_data_cache: parking_lot::RwLock::new(HashMap::default()),
        };

        dataset.load_all_episode_data_files()?;

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
            .map_err(|err| LeRobotError::IO(err, episode_parquet_file.clone()))?;

        // Read all data at once
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;
        let batches: Vec<RecordBatch> = reader
            .collect::<Result<_, _>>()
            .map_err(LeRobotError::Arrow)?;

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
            if let Some(cached_contents) = cache.get(&videopath) {
                return Ok(Arc::clone(cached_contents));
            }
        }

        let contents = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(&videopath).map_err(|err| LeRobotError::IO(err, videopath.clone()))?
        };

        // cache contents of big video blobs
        let mut cache = self.video_cache.write();
        if let Some(cached_contents) = cache.get(&videopath) {
            return Ok(Arc::clone(cached_contents));
        }

        let contents: Arc<[u8]> = Arc::from(contents.into_boxed_slice());
        cache.insert(videopath, contents.clone());

        Ok(contents)
    }

    /// Retrieve the task using the provided task index.
    pub fn task_by_index(&self, task: TaskIndex) -> Option<&LeRobotDatasetTask> {
        self.metadata.tasks.tasks.get(&task)
    }

    /// Loads a single episode from a `LeRobot` dataset and converts it into a collection of Rerun chunks.
    ///
    /// This function processes an episode from the dataset by extracting the relevant data columns and
    /// converting them into appropriate Rerun data structures. It handles different types of data
    /// (videos, images, scalar values, etc.) based on their data type specifications in the dataset metadata.
    fn load_episode(&self, episode: EpisodeIndex) -> Result<Vec<Chunk>, DataLoaderError> {
        let data = self
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
    ) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
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
    ) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
        let contents = self
            .read_episode_video_contents(observation, episode)
            .with_context(|| format!("Reading video contents for episode {episode:?} failed!"))?;

        let entity_path = observation;
        let video_bytes: &[u8] = &contents;

        // Parse the video to get its structure
        let video = VideoDataDescription::load_from_bytes(
            video_bytes,
            "video/mp4",
            observation,
            re_log_types::external::re_tuid::Tuid::new(),
        )
        .map_err(|err| {
            anyhow!("Failed to read video data description for feature '{observation}': {err}")
        })?;

        let (start_time, end_time) = self.get_feature_timestamps(episode, observation);

        if video.samples.is_empty() {
            return Err(DataLoaderError::Other(anyhow!(
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
            .map(|idx| idx + 1)
            .unwrap_or_else(|| video.keyframe_indices.len());

        // Determine the sample range to extract from the video
        let start_sample = video
            .get_keyframe_sample_range(start_keyframe)
            .ok_or(DataLoaderError::Other(anyhow!("Bad video data")))?
            .start;
        let end_sample = video
            .get_keyframe_sample_range(end_keyframe)
            .ok_or(DataLoaderError::Other(anyhow!("Bad video data")))?
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
                .get(&|_| video_bytes, sample_idx)
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

        let codec = re_sdk_types::components::VideoCodec::try_from(video.codec).map_err(|err| {
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

    fn load_episode_chunks(&self, episode: EpisodeIndex) -> Result<Vec<Chunk>, DataLoaderError> {
        self.load_episode(episode)
    }
}

/// Metadata for a `LeRobot` dataset.
///
/// This is a wrapper struct for the metadata files in the `meta` directory of a
/// `LeRobot` dataset. For more see [`LeRobotDatasetV3`].
pub struct LeRobotDatasetMetadataV3 {
    pub info: LeRobotDatasetInfoV3,
    pub tasks: LeRobotDatasetV3Tasks,
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

        // Convert episode data Vec to HashMap for O(1) lookups
        let episodes = episode_data
            .into_iter()
            .map(|ep| (ep.episode_index, ep))
            .collect();

        Ok(Self {
            info,
            tasks,
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
        for entry in
            std::fs::read_dir(metadir).map_err(|err| LeRobotError::IO(err, metadir.to_owned()))?
        {
            let entry = entry.map_err(|err| LeRobotError::IO(err, metadir.to_owned()))?;
            let path = entry.path();
            let path = path.as_path();

            re_log::trace!("Loading episode metadata from: {path:?}");

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

pub fn load_and_stream(
    dataset: &LeRobotDatasetV3,
    application_id: &ApplicationId,
    tx: &Sender<LoadedData>,
    loader_name: &str,
) {
    load_and_stream_versioned(dataset, application_id, tx, loader_name);
}
