use std::sync::Arc;

use anyhow::{Context as _, anyhow};
use arrow::{
    array::{
        ArrayRef, BinaryArray, FixedSizeListArray, Float64Array, Int64Array, RecordBatch,
        StringArray, StructArray,
    },
    buffer::ScalarBuffer,
    compute::cast,
    datatypes::{DataType, Field},
};
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{
    ArrowArray as _, Chunk, ChunkId, EntityPath, RowId, TimeColumn, TimeInt, TimePoint, Timeline,
    TimelineName, external::nohash_hasher::IntMap,
};
use re_sdk_types::archetypes::{self, AssetVideo, TextDocument, VideoFrameReference, VideoStream};
use re_sdk_types::{archetypes::EncodedImage, datatypes::VideoTimestamp};
use re_video::VideoDataDescription;
use re_video::player::VideoSliceSource;

use crate::{EpisodeIndex, Feature, LeRobotError};

/// Behavioral interface shared by all `LeRobot` dataset versions.
///
/// This trait is deliberately frozen at exactly two methods: iterating episode indices and
/// loading a single episode's chunks. Construction and version dispatch live on the
/// [`crate::LeRobotDataset`] enum instead — the trait is the mockable behavioral surface (see
/// the importer's `TestDataset`), not a place to grow new operations.
pub trait LeRobotDatasetOps {
    /// Returns an iterator over all episode indices within the dataset.
    fn iter_episode_indices(&self) -> impl Iterator<Item = EpisodeIndex>;

    /// Loads a specific episode and returns its chunks.
    fn load_episode_chunks(&self, episode: EpisodeIndex) -> Result<Vec<Chunk>, LeRobotError>;
}

/// Columns in the `LeRobot` dataset schema that we do not visualize in the viewer, and thus ignore.
pub const LEROBOT_DATASET_IGNORED_COLUMNS: &[&str] =
    &["episode_index", "index", "frame_index", "timestamp"];

/// Derive the episode's timeline from its record batch: a `frame_index` sequence timeline
/// when present, otherwise a `timestamp` duration timeline.
pub(crate) fn derive_timeline(data: &RecordBatch) -> Result<(Timeline, TimeColumn), LeRobotError> {
    let (timeline, time_column) = if let Some(frame_indices) = data.column_by_name("frame_index") {
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
        return Err(anyhow!("LeRobot dataset has neither frame_index nor timestamp column").into());
    };
    Ok((timeline, time_column))
}

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

/// One `TextDocument` chunk from resolved (time, text) rows.
pub(crate) fn build_text_chunk(
    entity: &str,
    rows: &[(TimeInt, String)],
    timeline: &Timeline,
) -> Result<Chunk, LeRobotError> {
    let mut chunk = Chunk::builder(entity);
    let mut row_id = RowId::new();
    for (time, text) in rows {
        let timepoint = TimePoint::default().with(*timeline, *time);
        chunk = chunk.with_archetype(row_id, timepoint, &TextDocument::new(text.clone()));
        row_id = row_id.next();
    }
    Ok(chunk.build()?)
}

/// v2 video: a static [`AssetVideo`] chunk plus, when frame timestamps can be read from the
/// container, a [`VideoFrameReference`] chunk aligning video frames with the episode timeline.
pub(crate) fn build_video_asset_chunks(
    entity: &str,
    contents: Vec<u8>,
    timeline: &Timeline,
    time_column: TimeColumn,
) -> Result<Vec<Chunk>, LeRobotError> {
    let video_asset = AssetVideo::new(contents);
    // Static asset chunk kept separate — it can be large.
    let mut chunks = vec![
        Chunk::builder(entity)
            .with_archetype(RowId::new(), TimePoint::default(), &video_asset)
            .build()?,
    ];

    match video_asset.read_frame_timestamps_nanos() {
        Ok(ts) => {
            let ts: ScalarBuffer<i64> = ts.into();
            let video_timestamps = ts
                .iter()
                .take(time_column.num_rows())
                .copied()
                .map(VideoTimestamp::from_nanos)
                .collect::<Vec<_>>();
            let column = VideoFrameReference::update_fields()
                .with_many_timestamp(video_timestamps)
                .columns_of_unit_batches()
                .with_context(|| {
                    format!("Failed to build VideoFrameReference column for {entity}")
                })?;
            chunks.push(Chunk::from_auto_row_ids(
                ChunkId::new(),
                entity.into(),
                std::iter::once((*timeline.name(), time_column)).collect(),
                column.collect(),
            )?);
        }
        Err(err) => {
            re_log::warn_once!("Failed to read frame timestamps from {entity} video: {err}");
        }
    }
    Ok(chunks)
}

/// v3 video: `VideoStream` codec + sample chunks for the episode's timestamp range.
pub(crate) fn build_video_stream_chunks(
    entity: &str,
    contents: &[u8],
    from_ts: f64,
    to_ts: f64,
    timeline: &Timeline,
    time_column: &TimeColumn,
) -> Result<Vec<Chunk>, LeRobotError> {
    // Parse the video to get its structure
    let video =
        VideoDataDescription::load_from_bytes(contents, "video/mp4", entity).map_err(|err| {
            anyhow!("Failed to read video data description for feature '{entity}': {err}")
        })?;

    if video.samples.is_empty() {
        return Err(LeRobotError::Other(anyhow!(
            "Video feature '{entity}' did not contain any samples"
        )));
    }

    // Convert timestamps to video time
    let timescale = video
        .timescale
        .ok_or_else(|| anyhow!("Video feature '{entity}' is missing timescale information"))?;

    let start_video_time = re_video::Time::from_secs(from_ts, timescale);
    let end_video_time = re_video::Time::from_secs(to_ts, timescale);

    // Find the GOPs that contain our time range
    let start_keyframe = video
        .presentation_time_keyframe_index(start_video_time)
        .unwrap_or(0);

    let end_keyframe = video
        .presentation_time_keyframe_index(end_video_time)
        .or_else(|| video.keyframe_indices.len().checked_sub(1))
        .ok_or(LeRobotError::Other(anyhow!("No keyframes in the video")))?;

    // Determine the sample range to extract from the video
    let start_sample = video
        .gop_sample_range_for_keyframe(start_keyframe)
        .ok_or(LeRobotError::Other(anyhow!("Bad video data")))?
        .start;

    let end_sample = video
        .gop_sample_range_for_keyframe(end_keyframe)
        .ok_or(LeRobotError::Other(anyhow!("Bad video data")))?
        .end();

    let sample_range =
        re_span::Span::try_from_start_end(start_sample, end_sample).ok_or_else(|| {
            anyhow!(
                "Inverted sample range {start_sample}..{end_sample} for feature '{entity}' \
                 (episode timestamp range {from_ts}..{to_ts})"
            )
        })?;

    // Extract all video samples in this range
    let mut samples = Vec::with_capacity(sample_range.len);

    for (sample_idx, sample_meta) in video.samples.iter_index_range_clamped(sample_range) {
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
            .get(&VideoSliceSource(contents), sample_idx)
            .ok_or_else(|| anyhow!("Sample {sample_idx} out of bounds for feature '{entity}'"))?;

        let sample_bytes = video.sample_data_in_stream_format(&chunk).with_context(|| {
            format!(
                "Failed to convert sample {sample_idx} for feature '{entity}' to the expected codec stream format"
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
                "Unsupported video codec {:?} for feature: '{entity}': {err}",
                video.codec
            )
        })?;

    let codec_chunk = Chunk::builder(entity)
        .with_archetype(
            RowId::new(),
            TimePoint::default(),
            &VideoStream::update_fields().with_codec(codec),
        )
        .build()?;

    let samples_chunk = Chunk::from_auto_row_ids(
        ChunkId::new(),
        entity.into(),
        std::iter::once((timeline.name().to_owned(), uniform_time_column)).collect(),
        samples_column.collect(),
    )?;

    Ok(vec![samples_chunk, codec_chunk])
}
