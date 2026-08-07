use std::sync::Arc;

use anyhow::{Context as _, anyhow};
use arrow::{
    array::{ArrayRef, BinaryArray, FixedSizeListArray, RecordBatch, StringArray, StructArray},
    compute::cast,
    datatypes::{DataType, Field},
};
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{
    ArrowArray as _, Chunk, ChunkId, EntityPath, RowId, TimeColumn, TimePoint, TimelineName,
    external::nohash_hasher::IntMap,
};
use re_sdk_types::archetypes;
use re_sdk_types::archetypes::EncodedImage;

use crate::{EpisodeIndex, Feature, LeRobotError};

/// Shared interface for all `LeRobot` dataset versions.
pub trait LeRobotDataset {
    /// Returns an iterator over all episode indices within the dataset.
    fn iter_episode_indices(&self) -> impl Iterator<Item = EpisodeIndex>;

    /// Loads a specific episode and returns its chunks.
    fn load_episode_chunks(&self, episode: EpisodeIndex) -> Result<Vec<Chunk>, LeRobotError>;
}

/// Columns in the `LeRobot` dataset schema that we do not visualize in the viewer, and thus ignore.
pub const LEROBOT_DATASET_IGNORED_COLUMNS: &[&str] =
    &["episode_index", "index", "frame_index", "timestamp"];

pub fn load_episode_images(
    observation: &str,
    timeline: &re_chunk::Timeline,
    data: &RecordBatch,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, LeRobotError> {
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

        let frame_idx = i64::try_from(frame_idx)
            .map_err(|err| anyhow!("Frame index exceeds max value: {err}"))?;
        let timepoint = TimePoint::default().with(*timeline, frame_idx);
        chunk = chunk.with_archetype(row_id, timepoint, &encoded_image);

        row_id = row_id.next();
    }

    Ok(std::iter::once(chunk.build().with_context(|| {
        format!("Failed to build image chunk for image: {observation}")
    })?))
}

pub fn load_episode_depth_images(
    observation: &str,
    timeline: &re_chunk::Timeline,
    data: &RecordBatch,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, LeRobotError> {
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
        let depth_image =
            re_sdk_types::archetypes::DepthImage::from_file_contents(img_buffer.to_owned())
                .map_err(|err| anyhow!("Failed to decode image: {err}"))?;

        let frame_idx = i64::try_from(frame_idx)
            .map_err(|err| anyhow!("Frame index exceeds max value: {err}"))?;
        let timepoint = TimePoint::default().with(*timeline, frame_idx);
        chunk = chunk.with_archetype(row_id, timepoint, &depth_image);

        row_id = row_id.next();
    }

    Ok(std::iter::once(chunk.build().with_context(|| {
        format!("Failed to build image chunk for image: {observation}")
    })?))
}

/// Helper type similar to [`itertools::Either`], but with 3 variants.
pub enum ScalarChunkIterator {
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

pub fn load_scalar(
    feature_key: &str,
    feature: &Feature,
    timelines: &IntMap<TimelineName, TimeColumn>,
    data: &RecordBatch,
) -> Result<ScalarChunkIterator, LeRobotError> {
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
                    LeRobotError::Other(anyhow!("Failed to downcast feature to FixedSizeListArray"))
                })?;

            let batch_chunks = make_scalar_batch_entity_chunks(
                &entity_path,
                feature,
                timelines,
                fixed_size_array,
            )?;
            Ok(ScalarChunkIterator::Batch(Box::new(batch_chunks)))
        }
        DataType::List(_field) => {
            let list_array = data
                .column_by_name(feature_key)
                .and_then(|col| col.downcast_array_ref::<arrow::array::ListArray>())
                .ok_or_else(|| {
                    LeRobotError::Other(anyhow!("Failed to downcast feature to ListArray"))
                })?;

            let sliced = extract_list_array_elements_as_f64(list_array).with_context(|| {
                format!("Failed to cast scalar feature {entity_path} to Float64")
            })?;

            let mut chunks = vec![make_scalar_entity_chunk(
                entity_path.clone(),
                timelines,
                &sliced,
            )?];

            if let Some(names_chunk) = make_names_chunk(&entity_path, feature, sliced.len())? {
                chunks.push(names_chunk);
            }

            Ok(ScalarChunkIterator::Batch(Box::new(chunks.into_iter())))
        }
        DataType::Float32 | DataType::Float64 => {
            let feature_data = data.column_by_name(feature_key).ok_or_else(|| {
                LeRobotError::Other(anyhow!(
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
    entity_path: &EntityPath,
    feature: &Feature,
    timelines: &IntMap<TimelineName, TimeColumn>,
    data: &FixedSizeListArray,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, LeRobotError> {
    let num_elements = data.value_length() as usize;

    let mut chunks = Vec::with_capacity(num_elements);

    let sliced = extract_fixed_size_list_array_elements_as_f64(data)
        .with_context(|| format!("Failed to cast scalar feature {entity_path} to Float64"))?;

    chunks.push(make_scalar_entity_chunk(
        entity_path.clone(),
        timelines,
        &sliced,
    )?);

    if let Some(names_chunk) = make_names_chunk(entity_path, feature, data.value_length() as usize)?
    {
        chunks.push(names_chunk);
    }

    Ok(chunks.into_iter())
}

/// If the feature has names, create a static chunk containing them.
fn make_names_chunk(
    entity_path: &EntityPath,
    feature: &Feature,
    num_elements: usize,
) -> Result<Option<Chunk>, LeRobotError> {
    let Some(names) = feature.names.clone() else {
        return Ok(None);
    };

    let names: Vec<_> = (0..num_elements)
        .map(|idx| names.name_for_index(idx))
        .collect();

    Ok(Some(
        Chunk::builder(entity_path.clone())
            .with_row(
                RowId::new(),
                TimePoint::default(),
                std::iter::once((
                    archetypes::SeriesLines::descriptor_names(),
                    Arc::new(StringArray::from_iter(names)) as Arc<dyn re_chunk::ArrowArray>,
                )),
            )
            .build()?,
    ))
}

fn make_scalar_entity_chunk(
    entity_path: EntityPath,
    timelines: &IntMap<TimelineName, TimeColumn>,
    sliced_data: &[ArrayRef],
) -> Result<Chunk, LeRobotError> {
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
        std::iter::once((
            archetypes::Scalars::descriptor_scalars().clone(),
            data_field_array,
        ))
        .collect(),
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
        .try_collect()
}

fn extract_list_array_elements_as_f64(
    data: &arrow::array::ListArray,
) -> anyhow::Result<Vec<ArrayRef>> {
    (0..data.len())
        .map(|idx| {
            cast(&data.value(idx), &DataType::Float64)
                .with_context(|| format!("Failed to cast {} to Float64", data.data_type()))
        })
        .try_collect()
}
