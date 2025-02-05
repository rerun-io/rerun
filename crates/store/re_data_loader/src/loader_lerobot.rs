use anyhow::{anyhow, Context};
use arrow::array::{
    ArrayRef, BinaryArray, FixedSizeListArray, Int64Array, RecordBatch, StructArray,
};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field};
use itertools::Either;
use re_arrow_util::ArrowArrayDowncastRef;
use re_chunk::external::nohash_hasher::IntMap;
use re_chunk::{
    ArrowArray, Chunk, ChunkId, EntityPath, RowId, TimeColumn, TimeInt, TimePoint, Timeline,
};

use re_log_types::StoreId;
use re_types::archetypes::{AssetVideo, EncodedImage, VideoFrameReference};
use re_types::components::{Scalar, VideoTimestamp};
use re_types::{Archetype, Component, ComponentBatch};

use crate::lerobot::{is_le_robot_dataset, DType, EpisodeIndex, LeRobotDataset};
use crate::{DataLoader, DataLoaderError, LoadedData};

/// Columns in the `LeRobot` dataset schema that we do not visualize in the viewer, and thus ignore.
const LEROBOT_DATASET_IGNORED_COLUMNS: &[&str] = &[
    "episode_index",
    "task_index",
    "index",
    "frame_index",
    "timestamp",
];

/// A [`DataLoader`] for `LeRobot` datasets.
///
/// An example dataset which can be loaded can be found on Hugging Face: [lerobot/pusht_image](https://huggingface.co/datasets/lerobot/pusht_image)
///
/// See [`crate::lerobot`] for more on the `LeRobot` dataset format.
pub struct LeRobotDatasetLoader;

impl DataLoader for LeRobotDatasetLoader {
    fn name(&self) -> String {
        "LeRobotDatasetLoader".into()
    }

    fn load_from_path(
        &self,
        _settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        if !is_le_robot_dataset(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
        }

        let dataset = LeRobotDataset::load_from_directory(&filepath)
            .map_err(|e| anyhow!("Loading LeRobot dataset failed: {e:?}"))?;

        re_log::info!(
            "Loading LeRobot dataset from {:?}, with {} episode(s)",
            dataset.path,
            dataset.metadata.episodes.len()
        );

        for episode in &dataset.metadata.episodes {
            let episode_idx = episode.index;
            let store_id = StoreId::from_string(
                re_log_types::StoreKind::Recording,
                format!("episode_{}", episode_idx.0),
            );

            let data = dataset
                .read_episode_data(episode_idx)
                .map_err(|e| anyhow!("Reading data for episode {episode_idx:?} failed: {e:?}"))?;

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
                        chunks.extend(log_episode_video(
                            &dataset,
                            feature_key,
                            episode_idx,
                            &timeline,
                            time_column.clone(),
                        )?);
                    }
                    DType::Image => {
                        chunks.extend(log_episode_images(feature_key, &timeline, &data)?);
                    }
                    DType::Int64 | DType::Bool => {
                        re_log::warn_once!(
                            "Loading LeRobot feature ({}) of dtype `{:?}` into Rerun is not yet implemented",
                            feature_key,
                            feature.dtype
                        );
                    }
                    DType::Float32 | DType::Float64 => {
                        chunks.extend(load_scalar(feature_key, &timelines, &data)?);
                    }
                }
            }

            for chunk in chunks {
                let data = LoadedData::Chunk(self.name(), store_id.clone(), chunk);
                if tx.send(data).is_err() {
                    break; // The other end has decided to hang up, not our problem.
                }
            }
        }

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        _settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        Err(DataLoaderError::Incompatible(filepath))
    }
}

fn log_episode_images(
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

fn log_episode_video(
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
    feature: &str,
    timelines: &IntMap<Timeline, TimeColumn>,
    data: &RecordBatch,
) -> Result<ScalarChunkIterator, DataLoaderError> {
    let field = data
        .schema_ref()
        .field_with_name(feature)
        .with_context(|| format!("Failed to get field for feature {feature} from parquet file"))?;

    match field.data_type() {
        DataType::FixedSizeList(_, _) => {
            let fixed_size_array = data
                .column_by_name(feature)
                .and_then(|col| col.downcast_array_ref::<FixedSizeListArray>())
                .ok_or_else(|| {
                    DataLoaderError::Other(anyhow!(
                        "Failed to downcast feature to FixedSizeListArray"
                    ))
                })?;

            let batch_chunks = make_scalar_batch_entity_chunks(field, timelines, fixed_size_array)?;
            Ok(ScalarChunkIterator::Batch(Box::new(batch_chunks)))
        }
        DataType::Float32 => {
            let feature_data = data.column_by_name(feature).ok_or_else(|| {
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
    timelines: &IntMap<Timeline, TimeColumn>,
    data: &FixedSizeListArray,
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    let num_elements = data.value_length() as usize;
    let num_values = data.len();

    let mut chunks = Vec::with_capacity(num_elements);

    for idx in 0..num_elements {
        let entity_path = format!("{}/{idx}", field.name());
        chunks.push(make_scalar_entity_chunk(
            entity_path.into(),
            timelines,
            &data.values().slice(idx, num_values),
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
        re_arrow_util::arrow_util::arrays_to_list_array(
            data_field_inner.data_type().clone(),
            &data_arrays,
        )
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
