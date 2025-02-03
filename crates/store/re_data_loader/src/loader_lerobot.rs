use std::sync::Arc;

use arrow::array::{ArrayRef, FixedSizeListArray, Float32Array, Int64Array};
use arrow::datatypes::{DataType, Field, FieldRef};
use itertools::Either;
use re_arrow_util::ArrowArrayDowncastRef;
use re_chunk::external::nohash_hasher::IntMap;
use re_chunk::{
    ArrowArray, Chunk, ChunkId, RowId, TimeColumn, TimePoint, Timeline, TransportChunk,
};

use re_log_types::external::re_tuid::{self};
use re_log_types::StoreId;
use re_types::archetypes::{AssetVideo, VideoFrameReference};
use re_types::components::{Scalar, VideoTimestamp};
use re_types::{Archetype, Component, ComponentBatch, Loggable};

use crate::le_robot::{LeRobotDataset, LeRobotError};
use crate::{DataLoader, DataLoaderError, LoadedData};

pub struct LeRobotDatasetLoader;

impl DataLoader for LeRobotDatasetLoader {
    fn name(&self) -> String {
        "LeRobotDatasetLoader".into()
    }

    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        if !crate::le_robot::is_le_robot_dataset(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
        }

        let dataset = LeRobotDataset::load_from_directory(&filepath)
            .map_err(|err| DataLoaderError::Other(anyhow::Error::new(err)))?;

        re_log::info!(
            "Loading LeRobot dataset from `{:?}`, with {} episode(s)",
            dataset.path,
            dataset.metadata.episodes.len()
        );

        for episode in &dataset.metadata.episodes {
            let episode_idx = episode.episode_index;
            // re_log::info!("loading episode {episode_idx}");
            let store_id = StoreId::from_string(
                re_log_types::StoreKind::Recording,
                format!("episode_{episode_idx}"),
            );

            let data = dataset
                .read_episode_data(episode_idx)
                .map_err(|err| DataLoaderError::Other(anyhow::Error::new(err)))?;

            // re_log::info!("episode has {} columns", data.num_columns());

            let frame_indices = Arc::new(
                data.column_by_name("frame_index")
                    .expect("failed to get frame index")
                    .clone(),
            );

            let mut timepoint = TimePoint::default();
            let timeline = re_log_types::Timeline::new_sequence("frame_index");

            let times: &arrow::buffer::ScalarBuffer<i64> = frame_indices
                .downcast_array_ref::<Int64Array>()
                .unwrap()
                .values();

            let time_column = re_chunk::TimeColumn::new(Some(true), timeline, times.clone());

            let timelines = std::iter::once((timeline, time_column.clone())).collect();

            let video_chunks = log_episode_video(
                &dataset,
                episode_idx,
                timepoint,
                &timeline,
                time_column.clone(),
            )
            .map_err(|err| DataLoaderError::Other(anyhow::Error::new(err)))?;

            for chunk in video_chunks {
                let data = LoadedData::Chunk(Self::name(&Self), store_id.clone(), chunk);
                tx.send(data).expect("failed to send video chunk!");
            }

            for idx in 0..data.num_columns() {
                let field = data.schema_ref().field(idx);
                // println!("column {}: {}", idx, data.schema().field(idx).name());
                // println!("   - {:?}", data.schema().field(idx).data_type());

                match field.data_type() {
                    // TODO: match on type of element
                    DataType::FixedSizeList(_element, _) => {
                        // Unwrap: we know the type of the column
                        let fixed_size_array = data
                            .column(idx)
                            .downcast_array_ref::<FixedSizeListArray>()
                            .unwrap();

                        for chunk in make_entity_chunks(field, &timelines, &fixed_size_array) {
                            let chunk_msg = LoadedData::Chunk(
                                LeRobotDatasetLoader::name(&LeRobotDatasetLoader),
                                store_id.clone(),
                                chunk,
                            );

                            tx.send(chunk_msg).expect("failed to send batch");
                        }
                    }
                    _ => {
                        // eprintln!(
                        //     "field with unknown data type {}: {:?}",
                        //     field.name(),
                        //     field.data_type()
                        // );
                    }
                }
            }
        }

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        // re_log::info!("loading path: {filepath:?}");
        return Err(DataLoaderError::Incompatible(filepath));
    }
}

fn log_episode_video(
    dataset: &LeRobotDataset,
    episode_idx: usize,
    timepoint: TimePoint,
    timeline: &Timeline,
    time_column: TimeColumn,
) -> Result<impl ExactSizeIterator<Item = Chunk>, LeRobotError> {
    let contents = dataset.read_episode_video_contents(episode_idx)?;
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
                .map_err(re_chunk::ChunkError::from)
                .expect("failed to create timestamp list array");

            // re_log::info!("num timestamps: {:?}", video_timestamps.len());

            // Indicator column.
            let video_frame_reference_indicators =
                <VideoFrameReference as Archetype>::Indicator::new_array(video_timestamps.len());
            let video_frame_reference_indicators_list_array = video_frame_reference_indicators
                .to_arrow_list_array()
                .map_err(re_chunk::ChunkError::from)
                .expect("failed to create frame reference indicators");

            Some(
                Chunk::from_auto_row_ids(
                    re_chunk::ChunkId::new(),
                    entity_path.into(),
                    std::iter::once((timeline.clone(), time_column)).collect(),
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
                )
                .expect("failed to create reference chunk!"),
            )
        }
        Err(err) => {
            re_log::warn_once!(
                "Failed to read frame timestamps from episode {episode_idx} video: {err}"
            );
            None
        }
    };

    // Put video asset into its own chunk since it can be fairly large.
    let video_asset_chunk = Chunk::builder(entity_path.into())
        .with_archetype(RowId::new(), timepoint.clone(), &video_asset)
        .build()
        .expect("failed to build video chunk");

    if let Some(video_frame_reference_chunk) = video_frame_reference_chunk {
        Ok(Either::Left(
            [video_asset_chunk, video_frame_reference_chunk].into_iter(),
        ))
    } else {
        // Still log the video asset, but don't include video frames.
        Ok(Either::Right(std::iter::once(video_asset_chunk)))
    }
}

fn make_entity_chunks(
    field: &Field,
    timelines: &IntMap<Timeline, TimeColumn>,
    data: &FixedSizeListArray,
) -> Vec<Chunk> {
    let num_elements = data.value_length() as usize;
    let num_values = data.len();
    // re_log::info!(
    //     "creating {} ({} rows) entities for {}",
    //     num_elements,
    //     data.len(),
    //     field.name()
    // );

    let inner_values = data
        .values()
        .as_any()
        .downcast_ref::<Float32Array>()
        .unwrap(); // TODO: what to do with this

    (0..num_elements)
        .map(|idx| {
            let data_field_inner = Field::new("item", DataType::Float64, true /* nullable */);
            let scalar_values = Arc::new(
                inner_values
                    .slice(idx, num_values)
                    .iter()
                    .map(|v| v.map(f64::from))
                    .collect::<arrow::array::Float64Array>(),
            ) as ArrayRef;

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

            Chunk::from_auto_row_ids(
                ChunkId::new(),
                entity_path.into(),
                timelines.clone(),
                [(
                    <re_types::components::Scalar as re_types::Component>::descriptor().clone(),
                    data_field_array,
                )]
                .into_iter()
                .collect(),
            )
            .expect("failed to create chunk!")
        })
        .collect::<Vec<_>>()
}
