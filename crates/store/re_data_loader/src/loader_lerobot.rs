use std::sync::mpsc::Sender;
use std::thread;

use anyhow::{anyhow, Context};
use arrow::array::{
    ArrayRef, BinaryArray, FixedSizeListArray, Int64Array, RecordBatch, StructArray,
};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field};
use itertools::Either;
use re_arrow_util::{extract_fixed_size_array_element, ArrowArrayDowncastRef};
use re_chunk::external::nohash_hasher::IntMap;
use re_chunk::{
    ArrowArray, Chunk, ChunkId, EntityPath, RowId, TimeColumn, TimeInt, TimePoint, Timeline,
};

use re_log_types::{ApplicationId, StoreId};
use re_types::archetypes::{AssetVideo, EncodedImage, TextDocument, VideoFrameReference};
use re_types::components::{Scalar, VideoTimestamp};
use re_types::{Archetype, Component, ComponentBatch};

use crate::lerobot::{is_lerobot_dataset, DType, EpisodeIndex, Feature, LeRobotDataset, TaskIndex};
use crate::load_file::prepare_store_info;
use crate::{DataLoader, DataLoaderError, LoadedData};

/// Columns in the `LeRobot` dataset schema that we do not visualize in the viewer, and thus ignore.
const LEROBOT_DATASET_IGNORED_COLUMNS: &[&str] =
    &["episode_index", "index", "frame_index", "timestamp"];

/// A [`DataLoader`] for `LeRobot` datasets.
///
/// An example dataset which can be loaded can be found on Hugging Face: [lerobot/pusht_image](https://huggingface.co/datasets/lerobot/pusht_image)
pub struct LeRobotDatasetLoader;

impl DataLoader for LeRobotDatasetLoader {
    fn name(&self) -> String {
        "LeRobotDatasetLoader".into()
    }

    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        tx: Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        if !is_lerobot_dataset(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
        }

        let dataset = LeRobotDataset::load_from_directory(&filepath)
            .map_err(|err| anyhow!("Loading LeRobot dataset failed: {err}"))?;
        let application_id = settings
            .application_id
            .clone()
            .unwrap_or(ApplicationId(format!("{filepath:?}")));

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        thread::Builder::new()
            .name(format!("load_and_stream({filepath:?}"))
            .spawn({
                move || {
                    re_log::info!(
                        "Loading LeRobot dataset from {:?}, with {} episode(s)",
                        dataset.path,
                        dataset.metadata.episodes.len(),
                    );
                    load_and_stream(&dataset, &application_id, &tx);
                }
            })
            .with_context(|| {
                format!("Failed to spawn IO thread to load LeRobot dataset {filepath:?} ")
            })?;

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        _settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        Err(DataLoaderError::Incompatible(filepath))
    }
}

fn load_and_stream(
    dataset: &LeRobotDataset,
    application_id: &ApplicationId,
    tx: &Sender<crate::LoadedData>,
) {
    // set up all recordings
    let episodes = prepare_episode_chunks(dataset, application_id, tx);

    for (episode, store_id) in &episodes {
        // log episode data to its respective recording
        match load_episode(dataset, *episode) {
            Ok(chunks) => {
                for chunk in chunks {
                    let data = LoadedData::Chunk(
                        LeRobotDatasetLoader::name(&LeRobotDatasetLoader),
                        store_id.clone(),
                        chunk,
                    );

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
    tx: &Sender<crate::LoadedData>,
) -> Vec<(EpisodeIndex, StoreId)> {
    let mut store_ids = vec![];

    for episode in &dataset.metadata.episodes {
        let episode = episode.index;

        let store_id = StoreId::from_string(
            re_log_types::StoreKind::Recording,
            format!("episode_{}", episode.0),
        );
        let set_store_info = LoadedData::LogMsg(
            LeRobotDatasetLoader::name(&LeRobotDatasetLoader),
            prepare_store_info(
                application_id.clone(),
                &store_id,
                re_log_types::FileSource::Sdk,
            ),
        );

        if tx.send(set_store_info).is_err() {
            break;
        }

        store_ids.push((episode, store_id.clone()));
    }

    store_ids
}

fn load_episode(
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
    let timelines = std::iter::once((timeline, time_column.clone())).collect();

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

            DType::Image => chunks.extend(load_episode_images(feature_key, &timeline, &data)?),
            DType::Int64 if feature_key == "task_index" => {
                // special case int64 task_index columns
                // this always refers to the task description in the dataset metadata.
                chunks.extend(log_episode_task(dataset, &timeline, &data)?);
            }
            DType::Int64 | DType::Bool | DType::String => {
                re_log::warn_once!(
                    "Loading LeRobot feature ({}) of dtype `{:?}` into Rerun is not yet implemented",
                    feature_key,
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
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    let task_indices = data
        .column_by_name("task_index")
        .and_then(|c| c.downcast_array_ref::<Int64Array>())
        .with_context(|| "Failed to get task_index field from dataset!")?;

    let mut chunk = Chunk::builder("task".into());
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

        let mut timepoint = TimePoint::default();
        timepoint.insert(*timeline, time_int);
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
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    let image_bytes = data
        .column_by_name(observation)
        .and_then(|c| c.downcast_array_ref::<StructArray>())
        .and_then(|a| a.column_by_name("bytes"))
        .and_then(|a| a.downcast_array_ref::<BinaryArray>())
        .with_context(|| format!("Failed to get binary data from image feature: {observation}"))?;

    let mut chunk = Chunk::builder(observation.into());
    let mut row_id = RowId::new();
    let mut time_int = TimeInt::ZERO;

    for idx in 0..image_bytes.len() {
        let img_buffer = image_bytes.value(idx);
        let encoded_image = EncodedImage::from_file_contents(img_buffer.to_owned());
        let mut timepoint = TimePoint::default();
        timepoint.insert(*timeline, time_int);
        chunk = chunk.with_archetype(row_id, timepoint, &encoded_image);

        row_id = row_id.next();
        time_int = time_int.inc();
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
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    let contents = dataset
        .read_episode_video_contents(observation, episode)
        .with_context(|| format!("Reading video contents for episode {episode:?} failed!"))?;

    let video_asset = AssetVideo::new(contents.into_owned());
    let entity_path = observation;

    let video_frame_reference_chunk = match video_asset.read_frame_timestamps_ns() {
        Ok(frame_timestamps_ns) => {
            let frame_timestamps_ns: arrow::buffer::ScalarBuffer<i64> = frame_timestamps_ns.into();

            let video_timestamps = frame_timestamps_ns
                .iter()
                .copied()
                .map(VideoTimestamp::from_nanoseconds)
                .collect::<Vec<_>>();
            let video_timestamp_batch = &video_timestamps as &dyn ComponentBatch;
            let video_timestamp_list_array = video_timestamp_batch
                .to_arrow_list_array()
                .map_err(re_chunk::ChunkError::from)?;

            // Indicator column.
            let video_frame_reference_indicators =
                <VideoFrameReference as Archetype>::Indicator::new_array(video_timestamps.len());
            let video_frame_reference_indicators_list_array = video_frame_reference_indicators
                .to_arrow_list_array()
                .map_err(re_chunk::ChunkError::from)?;

            Some(Chunk::from_auto_row_ids(
                re_chunk::ChunkId::new(),
                entity_path.into(),
                std::iter::once((*timeline, time_column)).collect(),
                [
                    (
                        VideoFrameReference::indicator().descriptor.clone(),
                        video_frame_reference_indicators_list_array,
                    ),
                    (
                        video_timestamp_batch.descriptor().into_owned(),
                        video_timestamp_list_array,
                    ),
                ]
                .into_iter()
                .collect(),
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
    let video_asset_chunk = Chunk::builder(entity_path.into())
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
    Single(std::iter::Once<Chunk>),
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
    timelines: &IntMap<Timeline, TimeColumn>,
    data: &RecordBatch,
) -> Result<ScalarChunkIterator, DataLoaderError> {
    let field = data
        .schema_ref()
        .field_with_name(feature_key)
        .with_context(|| {
            format!("Failed to get field for feature {feature_key} from parquet file")
        })?;

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
                make_scalar_batch_entity_chunks(field, feature, timelines, fixed_size_array)?;
            Ok(ScalarChunkIterator::Batch(Box::new(batch_chunks)))
        }
        DataType::Float32 => {
            let feature_data = data.column_by_name(feature_key).ok_or_else(|| {
                DataLoaderError::Other(anyhow!(
                    "Failed to get LeRobot dataset column data for: {:?}",
                    field.name()
                ))
            })?;

            Ok(ScalarChunkIterator::Single(std::iter::once(
                make_scalar_entity_chunk(
                    field.name().clone().into(),
                    timelines,
                    &feature_data.clone(),
                )?,
            )))
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
    field: &Field,
    feature: &Feature,
    timelines: &IntMap<Timeline, TimeColumn>,
    data: &FixedSizeListArray,
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    let num_elements = data.value_length() as usize;

    let mut chunks = Vec::with_capacity(num_elements);

    for idx in 0..num_elements {
        let name = feature
            .names
            .as_ref()
            .and_then(|names| names.name_for_index(idx).cloned())
            .unwrap_or(format!("{idx}"));

        // The data that comes out of lerobot is structured as a fixed size array, but Rerun
        // needs us to submit these as individual chunks of scalar values, so for each element
        // in the source array we create a new chunk.
        // TODO(#9005): Once we have Rerun support for native fixed size list arrays we can stop
        // doing this.
        let scalar_values = extract_fixed_size_array_element(data, idx as u32).map_err(|err| {
            anyhow!(
                "Failed to extract values for scalar feature {:?}: {err}",
                field.name()
            )
        })?;

        let entity_path = format!("{}/{name}", field.name());
        chunks.push(make_scalar_entity_chunk(
            entity_path.into(),
            timelines,
            &scalar_values,
        )?);
    }

    Ok(chunks.into_iter())
}

fn make_scalar_entity_chunk(
    entity_path: EntityPath,
    timelines: &IntMap<Timeline, TimeColumn>,
    data: &ArrayRef,
) -> Result<Chunk, DataLoaderError> {
    // cast the slice to f64 first, as scalars need an f64
    let scalar_values = cast(&data, &DataType::Float64).with_context(|| {
        format!(
            "Failed to cast scalar feature {:?} to Float64",
            entity_path.clone()
        )
    })?;

    let sliced = (0..data.len())
        .map(|idx| scalar_values.slice(idx, 1))
        .collect::<Vec<_>>();

    let data_arrays = sliced.iter().map(|e| Some(e.as_ref())).collect::<Vec<_>>();

    let data_field_inner = Field::new("item", DataType::Float64, true /* nullable */);
    #[allow(clippy::unwrap_used)] // we know we've given the right field type
    let data_field_array: arrow::array::ListArray =
        re_arrow_util::arrays_to_list_array(data_field_inner.data_type().clone(), &data_arrays)
            .unwrap();

    Ok(Chunk::from_auto_row_ids(
        ChunkId::new(),
        entity_path,
        timelines.clone(),
        std::iter::once((
            <Scalar as Component>::descriptor().clone(),
            data_field_array,
        ))
        .collect(),
    )?)
}
