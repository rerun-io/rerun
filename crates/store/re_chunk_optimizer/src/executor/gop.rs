//! Execution of one [`PlanUnit::GopRun`](crate::plan::PlanUnit).

use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::sync::Arc;

use arrow::array::{Array as _, BooleanBuilder, ListArray, ListBuilder};
use arrow::buffer::{Buffer, ScalarBuffer};
use arrow::datatypes::{DataType, Field};
use itertools::Either;

use re_chunk::external::nohash_hasher::IntMap;
use re_chunk::{Chunk, ChunkComponents, ChunkId, LatestAtQuery, RowId, TimeColumn};
use re_chunk_index::ChunkProvider;
use re_log_types::{EntityPath, TimeInt, TimePoint, TimelineName};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::{IsKeyframe, VideoCodec};
use re_types_core::{ArrowDataType as _, SerializedComponentColumn};
use re_video::DetectGopStartError;

use super::accumulator::Accumulator;
use super::load_in_order;
use crate::error::{Error, VideoStreamErrorSource};
use crate::plan::{
    CodecSource, GopChunkingConfig, KeyframePlacementConfig, codec_column, sample_column,
};
use crate::row_merge::{MergeInput, RowMerge};
use crate::view::ChunkIndexView;

/// The stream's codec, decided from `source`. Under [`CodecSource::Temporal`], the codec is also
/// pushed to `ready` as one static chunk.
///
/// Reminder: this ONLY checks codec values that are NOT collocated with samples, because loading
/// sample-containing chunks for a check would be too expensive. Codec values collocated with
/// samples (if any) are instead checked during the actual GoP batching run.
// TODO(RR-5857): this amounts to performing a query ("unique codec values") off the input store.
// We should be able to do that with existing tools, such as a table provider and datafusion.
async fn decide_codec(
    provider: &dyn ChunkProvider,
    view: &ChunkIndexView,
    entity_path: &EntityPath,
    source: &CodecSource,
    ready: &mut VecDeque<Arc<Chunk>>,
) -> Result<VideoCodec, Error> {
    let (CodecSource::Static(codec_slices) | CodecSource::Temporal(codec_slices)) = source;
    let chunks = load_in_order(provider, view, codec_slices).await?;
    let chunk_ids: Vec<ChunkId> = codec_slices
        .iter()
        .map(|slice| view.chunk(slice.chunk).chunk_id)
        .collect();
    let no_codec = || {
        let chunk_id = view.chunk(codec_slices.first().chunk).chunk_id;
        Error::video_stream(entity_path, chunk_id, VideoStreamErrorSource::NoCodec)
    };

    match source {
        CodecSource::Static(_) => {
            // The latest static row wins, as in the store. The timeline is ignored on static
            // chunks.
            let query = LatestAtQuery::new(TimelineName::log_time(), TimeInt::MAX);
            let latest = std::iter::zip(&chunks, &chunk_ids)
                .filter_map(|(chunk, &chunk_id)| {
                    Some((chunk.latest_at(&query, codec_column())?, chunk_id))
                })
                .max_by_key(|(latest, _)| latest.row_id());
            let Some((latest, chunk_id)) = latest else {
                return Err(no_codec());
            };
            match latest.component_instance::<VideoCodec>(codec_column(), 0) {
                Some(Ok(codec)) => Ok(codec),
                Some(Err(source)) => Err(Error::video_stream(
                    entity_path,
                    chunk_id,
                    VideoStreamErrorSource::UnreadableCodec { source },
                )),
                None => Err(no_codec()),
            }
        }

        CodecSource::Temporal(_) => {
            let mut codec = None;
            for (chunk, &chunk_id) in std::iter::zip(&chunks, &chunk_ids) {
                for batch in chunk.iter_component::<VideoCodec>(codec_column()) {
                    for &found in batch.as_slice() {
                        match codec {
                            None => codec = Some(found),
                            Some(expected) if expected != found => {
                                return Err(Error::video_stream(
                                    entity_path,
                                    chunk_id,
                                    VideoStreamErrorSource::SeveralCodecs {
                                        first: expected,
                                        second: found,
                                    },
                                ));
                            }
                            Some(_) => {}
                        }
                    }
                }
            }
            let codec = codec.ok_or_else(no_codec)?;

            let static_chunk = Chunk::builder(entity_path.clone())
                .with_component_batch(
                    RowId::new(),
                    TimePoint::default(),
                    (VideoStream::descriptor_codec(), &[codec]),
                )
                .build()
                .map_err(|err| Error::malformed_generated_chunk(entity_path, err))?;
            ready.push_back(Arc::new(static_chunk));
            Ok(codec)
        }
    }
}

/// Check one loaded input's codec cells and drop them, then detect its keyframes: the chunk to
/// merge, and whether each of its rows holds a keyframe.
fn prepare_input(
    mut chunk: Arc<Chunk>,
    chunk_id: ChunkId,
    codec: VideoCodec,
    entity_path: &EntityPath,
    rejected_samples: &mut RejectedSamples,
) -> Result<(Arc<Chunk>, Vec<bool>), Error> {
    re_tracing::profile_function!();

    if chunk.components().contains_component(codec_column()) {
        check_codec(&chunk, codec, entity_path, chunk_id)?;
        chunk = Arc::new(chunk.component_dropped(codec_column()));
    }

    let mut keyframes = vec![false; chunk.num_rows()];
    for (row, sample) in sample_bytes(&chunk, entity_path, chunk_id)? {
        if let Some(bytes) = sample {
            match re_video::detect_gop_start(bytes.as_slice(), codec.into()) {
                Ok(detection) => keyframes[row] = detection.is_start_of_gop(),
                Err(err) => {
                    rejected_samples.count += 1;
                    rejected_samples.first.get_or_insert((chunk_id, err));
                }
            }
        }
    }
    Ok((chunk, keyframes))
}

/// Fail unless every codec value of `chunk` is `expected`.
fn check_codec(
    chunk: &Chunk,
    expected: VideoCodec,
    entity_path: &EntityPath,
    chunk_id: ChunkId,
) -> Result<(), Error> {
    for batch in chunk.iter_component::<VideoCodec>(codec_column()) {
        if let Some(&found) = batch.as_slice().iter().find(|&&codec| codec != expected) {
            return Err(Error::video_stream(
                entity_path,
                chunk_id,
                VideoStreamErrorSource::CodecMismatch { found, expected },
            ));
        }
    }
    Ok(())
}

/// The sample of every row with a non-null `VideoStream:sample` cell, as `(row, sample)`; the
/// sample is `None` for an empty batch.
fn sample_bytes<'a>(
    chunk: &'a Chunk,
    entity_path: &EntityPath,
    chunk_id: ChunkId,
) -> Result<impl Iterator<Item = (usize, Option<Buffer>)> + 'a, Error> {
    let Some(list_array) = chunk.components().get_array(sample_column()) else {
        return Ok(Either::Left(std::iter::empty()));
    };

    // `iter_slices` silently yields nothing on a datatype it does not handle.
    let datatype = list_array.values().data_type();
    let handled = match datatype {
        DataType::Binary | DataType::LargeBinary => true,
        DataType::List(field) => *field.data_type() == DataType::UInt8,
        _ => false,
    };
    if !handled {
        return Err(Error::video_stream(
            entity_path,
            chunk_id,
            VideoStreamErrorSource::UnsupportedSampleDatatype(datatype.clone()),
        ));
    }

    // `iter_slices` yields one batch per valid row, skipping the null ones like `valid_rows`.
    let valid_rows = (0..list_array.len()).filter(|&row| list_array.is_valid(row));
    let batches = chunk.iter_slices::<&[u8]>(sample_column());
    Ok(Either::Right(
        std::iter::zip(valid_rows, batches).map(|(row, batch)| (row, batch.into_iter().next())),
    ))
}

/// Executor state of a single [`PlanUnit::GopRun`](crate::plan::PlanUnit).
pub struct GopRunState {
    entity_path: EntityPath,

    codec: StreamCodec,

    /// The run's inputs, each row annotated with whether its sample is a keyframe.
    merge: RowMerge<bool>,

    /// `None` under [`GopChunkingConfig::OnePerGop`]: every GoP chunk goes to the output as-is.
    samples: Option<Accumulator>,
    keyframes: KeyframeSink,

    warned_orphan_head: bool,

    /// Samples the detector rejected, reported in one warning when the run finishes. They count as
    /// non-keyframes: a missing `is_keyframe` cell only costs a reader an earlier sync point.
    rejected_samples: RejectedSamples,
}

/// The stream's codec: where to decide it from until the first step, then the decision.
enum StreamCodec {
    Pending(CodecSource),
    Decided(VideoCodec),
}

#[derive(Default)]
struct RejectedSamples {
    count: u64,
    first: Option<(ChunkId, DetectGopStartError)>,
}

/// Where the run's `is_keyframe` cells go, per [`KeyframePlacementConfig`].
enum KeyframeSink {
    Inline,
    OwnChunk(Accumulator),
}

impl GopRunState {
    pub fn new(
        entity_path: EntityPath,
        inputs: Vec<MergeInput>,
        codec: CodecSource,
        timeline: TimelineName,
        gop_chunking: GopChunkingConfig,
        keyframes: KeyframePlacementConfig,
    ) -> Self {
        Self {
            entity_path,
            codec: StreamCodec::Pending(codec),
            merge: RowMerge::new(timeline, inputs),
            samples: match gop_chunking {
                GopChunkingConfig::OnePerGop => None,
                GopChunkingConfig::Packed(target) => Some(Accumulator::new(target)),
            },
            keyframes: match keyframes {
                KeyframePlacementConfig::Inline => KeyframeSink::Inline,
                KeyframePlacementConfig::OwnChunk(target) => {
                    KeyframeSink::OwnChunk(Accumulator::new(target))
                }
            },
            warned_orphan_head: false,
            rejected_samples: RejectedSamples::default(),
        }
    }

    /// Drive the run until it emits at least one output into `ready` or finishes.
    pub async fn step(
        &mut self,
        provider: &dyn ChunkProvider,
        view: &ChunkIndexView,
        ready: &mut VecDeque<Arc<Chunk>>,
    ) -> Result<ControlFlow<()>, Error> {
        let codec = match &self.codec {
            StreamCodec::Decided(codec) => *codec,
            StreamCodec::Pending(source) => {
                let codec = decide_codec(provider, view, &self.entity_path, source, ready).await?;
                self.codec = StreamCodec::Decided(codec);
                codec
            }
        };

        loop {
            if let Some(len) = self.next_complete_gop() {
                self.emit_gop(len, ready)?;
                if !ready.is_empty() {
                    return Ok(ControlFlow::Continue(()));
                }
            } else if !self.merge.is_exhausted() {
                self.load_batch(provider, view, codec).await?;
            } else {
                if let Some(samples) = &mut self.samples {
                    samples.flush(ready)?;
                }
                if let KeyframeSink::OwnChunk(keyframes) = &mut self.keyframes {
                    keyframes.flush(ready)?;
                }
                let RejectedSamples { count, first } = &self.rejected_samples;
                if let Some((chunk_id, err)) = first {
                    re_log::warn!(
                        "The keyframe detector rejected {} video sample(s), which are not marked as \
                         keyframes. First rejection: {err}\nChunk: {chunk_id}\nEntity: {}",
                        re_format::format_uint(*count),
                        self.entity_path
                    );
                }
                re_log::debug_assert!(
                    self.merge.final_rows().is_empty(),
                    "every loaded row was emitted"
                );
                return Ok(ControlFlow::Break(()));
            }
        }
    }

    /// The length of the complete GoP at the head of the merge's final rows, if any.
    ///
    /// A GoP is complete once the next keyframe is final. Rows before the stream's first keyframe
    /// are not a GoP, and are emitted as soon as they are final.
    fn next_complete_gop(&self) -> Option<usize> {
        let final_rows = self.merge.final_rows();
        if final_rows.is_empty() {
            return None;
        }

        match final_rows.iter().position(|row| row.annotation) {
            // The head is a keyframe once one has been seen, so these precede the first keyframe.
            None => Some(final_rows.len()),
            Some(0) => match final_rows[1..].iter().position(|row| row.annotation) {
                Some(next) => Some(next + 1),
                None => self.merge.is_exhausted().then_some(final_rows.len()),
            },
            Some(head) => Some(head),
        }
    }

    /// Load the next IO batch of inputs into the merge.
    async fn load_batch(
        &mut self,
        provider: &dyn ChunkProvider,
        view: &ChunkIndexView,
        codec: VideoCodec,
    ) -> Result<(), Error> {
        let entity_path = &self.entity_path;
        let rejected_samples = &mut self.rejected_samples;
        self.merge
            .load_batch(provider, view, |chunk, chunk_id| {
                prepare_input(chunk, chunk_id, codec, entity_path, rejected_samples)
            })
            .await
    }

    /// Emit the GoP made of the merge's first `len` final rows.
    fn emit_gop(&mut self, len: usize, ready: &mut VecDeque<Arc<Chunk>>) -> Result<(), Error> {
        re_tracing::profile_function!();

        let Some(head) = self.merge.final_rows().first() else {
            return Ok(());
        };
        let head_is_keyframe = head.annotation;
        if !head_is_keyframe && !self.warned_orphan_head {
            re_log::warn!(
                "Video stream does not start at a keyframe\nEntity: {}",
                self.entity_path
            );
            self.warned_orphan_head = true;
        }

        let (chunk, head_position) = self
            .merge
            .take(len)
            .map_err(|err| Error::merge_chunks(&self.entity_path, err))?;

        let keyframe_position = head_is_keyframe.then_some(head_position);
        let chunk = self.attach_keyframe(chunk, keyframe_position, ready)?;

        match &mut self.samples {
            Some(samples) => {
                samples.push(chunk, ready)?;
            }
            None => ready.push_back(chunk),
        }
        Ok(())
    }

    /// Add the `is_keyframe` column to the GoP chunk, or push a one-row `is_keyframe` chunk to the
    /// `is_keyframe` output. `position` is the keyframe row's, `None` when the GoP chunk holds
    /// rows before the stream's first keyframe.
    fn attach_keyframe(
        &mut self,
        mut chunk: Chunk,
        position: Option<usize>,
        ready: &mut VecDeque<Arc<Chunk>>,
    ) -> Result<Arc<Chunk>, Error> {
        match &mut self.keyframes {
            KeyframeSink::Inline => {
                let column = is_keyframe_array(chunk.num_rows(), position);
                chunk
                    .add_component(SerializedComponentColumn::new(
                        column,
                        VideoStream::descriptor_is_keyframe(),
                    ))
                    .map_err(|err| Error::malformed_generated_chunk(&self.entity_path, err))?;
                Ok(Arc::new(chunk))
            }

            KeyframeSink::OwnChunk(keyframes) => {
                if let Some(position) = position {
                    let is_keyframe = is_keyframe_chunk(&self.entity_path, &chunk, position)?;
                    keyframes.push(is_keyframe, ready)?;
                }
                Ok(Arc::new(chunk))
            }
        }
    }
}

/// An `is_keyframe` column of `num_rows` rows: `[true]` at `position`, null elsewhere.
fn is_keyframe_array(num_rows: usize, position: Option<usize>) -> ListArray {
    let field = Field::new("item", IsKeyframe::arrow_data_type(), true);
    let mut builder =
        ListBuilder::with_capacity(BooleanBuilder::new(), num_rows).with_field(Arc::new(field));
    for row in 0..num_rows {
        if Some(row) == position {
            builder.values().append_value(true);
            builder.append(true);
        } else {
            builder.append_null();
        }
    }
    builder.finish()
}

/// A one-row `is_keyframe` chunk at the keyframe row `position` of `gop_chunk`: its row id and
/// times.
fn is_keyframe_chunk(
    entity_path: &EntityPath,
    gop_chunk: &Chunk,
    position: usize,
) -> Result<Arc<Chunk>, Error> {
    let row_id = gop_chunk.row_ids_slice()[position];
    let timelines: IntMap<TimelineName, TimeColumn> = gop_chunk
        .timelines()
        .iter()
        .map(|(name, time_column)| {
            (
                *name,
                TimeColumn::new(
                    Some(true),
                    *time_column.timeline(),
                    ScalarBuffer::from(vec![time_column.times_raw()[position]]),
                ),
            )
        })
        .collect();

    let mut components = ChunkComponents::default();
    components.insert(SerializedComponentColumn::new(
        is_keyframe_array(1, Some(0)),
        VideoStream::descriptor_is_keyframe(),
    ));

    Chunk::from_native_row_ids(
        ChunkId::new(),
        entity_path.clone(),
        Some(true),
        &[row_id],
        timelines,
        components,
    )
    .map(Arc::new)
    .map_err(|err| Error::malformed_generated_chunk(entity_path, err))
}
