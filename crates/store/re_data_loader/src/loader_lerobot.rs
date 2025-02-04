use anyhow::{anyhow, Context};
use arrow::array::{FixedSizeListArray, Int64Array, RecordBatch};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field};
use itertools::Either;
use re_arrow_util::ArrowArrayDowncastRef;
use re_chunk::external::nohash_hasher::IntMap;
use re_chunk::{ArrowArray, Chunk, ChunkId, RowId, TimeColumn, TimePoint, Timeline};

use re_log_types::StoreId;
use re_types::archetypes::{AssetVideo, VideoFrameReference};
use re_types::components::{Scalar, VideoTimestamp};
use re_types::{Archetype, Component, ComponentBatch};

use crate::le_robot::{DType, LeRobotDataset};
use crate::{DataLoader, DataLoaderError, LoadedData};

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
        if !crate::le_robot::is_le_robot_dataset(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
        }

        let dataset = LeRobotDataset::load_from_directory(&filepath)
            .with_context(|| "Loading LeRobot dataset failed")?;

        re_log::info!(
            "Loading LeRobot dataset from `{:?}`, with {} episode(s)",
            dataset.path,
            dataset.metadata.episodes.len()
        );

        for episode in &dataset.metadata.episodes {
            let episode_idx = episode.episode_index;
            let store_id = StoreId::from_string(
                re_log_types::StoreKind::Recording,
                format!("episode_{episode_idx}"),
            );

            let data = dataset
                .read_episode_data(episode_idx)
                .with_context(|| format!("Reading data for episode {episode_idx} failed!"))?;

            let frame_indices = data
                .column_by_name("frame_index")
                .ok_or_else(|| anyhow!("failed to get frame index"))?
                .clone();

            let timeline = re_log_types::Timeline::new_sequence("frame_index");
            let times: &arrow::buffer::ScalarBuffer<i64> = frame_indices
                .downcast_array_ref::<Int64Array>()
                .ok_or_else(|| anyhow!("frame indices are of an unexpected type"))?
                .values();

            let time_column = re_chunk::TimeColumn::new(Some(true), timeline, times.clone());
            let timelines = std::iter::once((timeline, time_column.clone())).collect();

            let mut chunks = Vec::new();

            for (feature_key, feature) in &dataset.metadata.info.features {
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
                    DType::Image | DType::Int64 | DType::Bool => {
                        // TODO(gijsd): log images
                        re_log::warn_once!("Logging for dtype {:?} not implemented", feature.dtype);
                    }
                    DType::Float32 | DType::Float64 => {
                        chunks.extend(log_scalar(feature_key, &timelines, &data)?);
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

fn log_episode_video(
    dataset: &LeRobotDataset,
    observation: &str,
    episode: usize,
    timeline: &Timeline,
    time_column: TimeColumn,
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    let contents = dataset
        .read_episode_video_contents(observation, episode)
        .with_context(|| format!("Reading video contents for episode {episode} failed!"))?;

    let video_asset = AssetVideo::new(contents.into_owned());
    let entity_path = "video";

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
                "Failed to read frame timestamps from episode {episode} video: {err}"
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

fn log_scalar(
    feature: &str,
    timelines: &IntMap<Timeline, TimeColumn>,
    data: &RecordBatch,
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    let field = data
        .schema_ref()
        .field_with_name(feature)
        .with_context(|| format!("Failed to get field for feature {feature} from parquet file"))?;

    if !matches!(field.data_type(), DataType::FixedSizeList(_, _)) {
        re_log::warn_once!(
            "Tried logging scalar with unsupported dtype: {}",
            field.data_type()
        );
        return Ok(Either::Left(std::iter::empty()));
    }

    let fixed_size_array = data
        .column_by_name(feature)
        .and_then(|col| col.downcast_array_ref::<FixedSizeListArray>())
        .ok_or_else(|| {
            DataLoaderError::Other(anyhow!("Failed to downcast feature to FixedSizeListArray"))
        })?;

    Ok(Either::Right(make_scalar_entity_chunks(
        field,
        timelines,
        fixed_size_array,
    )?))
}

fn make_scalar_entity_chunks(
    field: &Field,
    timelines: &IntMap<Timeline, TimeColumn>,
    data: &FixedSizeListArray,
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    let num_elements = data.value_length() as usize;
    let num_values = data.len();

    let mut chunks = Vec::with_capacity(num_elements);
    let data_field_inner = Field::new("item", DataType::Float64, true /* nullable */);

    for idx in 0..num_elements {
        // cast the slice to f64 first, as scalars need an f64
        let scalar_values = cast(&data.values().slice(idx, num_values), &DataType::Float64)
            .with_context(|| {
                format!("Failed to cast scalar feature {} to Float64", field.name())
            })?;

        let sliced = (0..num_values)
            .map(|idx| scalar_values.slice(idx, 1))
            .collect::<Vec<_>>();

        let data_arrays = sliced.iter().map(|e| Some(e.as_ref())).collect::<Vec<_>>();

        #[allow(clippy::unwrap_used)] // we know we've given the right field type
        let data_field_array: arrow::array::ListArray =
            re_arrow_util::arrow_util::arrays_to_list_array(
                data_field_inner.data_type().clone(),
                &data_arrays,
            )
            .unwrap();

        let entity_path = format!("{}/{idx}", field.name());
        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.into(),
            timelines.clone(),
            std::iter::once((
                <Scalar as Component>::descriptor().clone(),
                data_field_array,
            ))
            .collect(),
        )?;

        chunks.push(chunk);
    }

    Ok(chunks.into_iter())
}
