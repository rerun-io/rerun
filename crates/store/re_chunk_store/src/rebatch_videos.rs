use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ahash::{HashMap, HashSet};
use arrow::array::{ArrayRef, BooleanArray, ListArray as ArrowListArray};
use arrow::buffer::{OffsetBuffer, ScalarBuffer};
use arrow::datatypes::Field;
use itertools::izip;
use re_byte_size::SizeBytes as _;
use re_chunk::{Chunk, ChunkId, ChunkShared, EntityPath, TimeColumn, Timeline, TimelineName};
use re_format::format_bytes;
use re_log_types::TimeInt;
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::{IsKeyframe, VideoCodec, VideoSample};

use crate::{ChunkStore, ChunkStoreConfig, ChunkTrackingMode};

/// Info about a single video sample (frame) in the index.
#[derive(Clone, Copy)]
struct SampleInfo {
    chunk_id: ChunkId,

    /// Row index within the source chunk.
    row_index: usize,

    /// Timestamp on an automatically chosen timeline
    time: TimeInt,

    /// Is sync/keyframe?
    is_start_of_gop: bool,
}

/// Rebatch video stream chunks along GoP boundaries.
///
/// Each output chunk contains one or more complete GoPs. Multiple GoPs are packed
/// into the same chunk as long as the total size stays within `chunk_max_bytes`.
/// If `chunk_max_bytes` is 0, each GoP gets its own chunk.
///
/// Non-video chunks are passed through unchanged.
///
/// This allows for much faster random-reads of video frames,
/// as you always need to load at most one chunk.
///
/// `is_start_of_gop` can use `re_video::is_start_of_gop`.
/// We do dependency injection here in order to avoid a direct dependency on `re_video`.
pub fn rebatch_video_chunks_to_gops(
    store: &ChunkStore,
    config: &ChunkStoreConfig,
    is_start_of_gop: &dyn Fn(&[u8], VideoCodec) -> anyhow::Result<bool>,
    fix_keyframe: bool,
) -> anyhow::Result<ChunkStore> {
    re_tracing::profile_function!();

    let sample_component = VideoStream::descriptor_sample().component;
    let keyframe_component = VideoStream::descriptor_is_keyframe().component;

    // Collect all temporal chunks that contain video samples, grouped by entity.
    // Also collect dedicated `is_keyframe` chunks (no sample column) separately —
    // we need to read user labels from them during validation.
    let mut sample_chunks_per_entity: HashMap<EntityPath, HashMap<ChunkId, ChunkShared>> =
        Default::default();
    let mut dedicated_keyframe_chunks_per_entity: HashMap<
        EntityPath,
        HashMap<ChunkId, ChunkShared>,
    > = Default::default();
    for chunk in store.iter_physical_chunks() {
        if chunk.is_static() {
            continue;
        }
        if chunk.components().contains_component(sample_component) {
            sample_chunks_per_entity
                .entry(chunk.entity_path().clone())
                .or_default()
                .insert(chunk.id(), chunk.clone());
        } else if chunk.components().contains_component(keyframe_component) {
            dedicated_keyframe_chunks_per_entity
                .entry(chunk.entity_path().clone())
                .or_default()
                .insert(chunk.id(), chunk.clone());
        }
    }

    if sample_chunks_per_entity.is_empty() {
        return Ok(store.clone()); // no video streams in the store
    }

    let mut replaced_chunk_ids: HashSet<ChunkId> = HashSet::default();
    let mut new_chunks: Vec<Chunk> = Vec::new();
    let mut keyframe_chunks: Vec<Chunk> = Vec::new();

    re_log::info!(
        num_video_entities = sample_chunks_per_entity.len(),
        "found video entities for GoP realignment"
    );

    for (entity_path, sample_chunks) in &sample_chunks_per_entity {
        let dedicated_keyframe_chunks = dedicated_keyframe_chunks_per_entity.get(entity_path);
        match rebatch_video_entity(
            store,
            config,
            is_start_of_gop,
            entity_path,
            sample_chunks,
            dedicated_keyframe_chunks,
            fix_keyframe,
        ) {
            Ok(EntityRebatch::Rebuild {
                rebatched,
                new_keyframe_chunk,
            }) => {
                replaced_chunk_ids.extend(sample_chunks.keys().copied());
                if let Some(stale) = dedicated_keyframe_chunks {
                    replaced_chunk_ids.extend(stale.keys().copied());
                }
                new_chunks.extend(rebatched);
                if let Some(kf) = new_keyframe_chunk {
                    keyframe_chunks.push(*kf);
                }
            }
            Ok(EntityRebatch::KeepDedicatedKeyframeChunks { rebatched }) => {
                // Existing dedicated keyframe chunks are canonical — leave
                // them alone. Sample chunks still get GoP-rebatched.
                replaced_chunk_ids.extend(sample_chunks.keys().copied());
                new_chunks.extend(rebatched);
            }
            Ok(EntityRebatch::Skip) => {
                // Entity can't be safely rebatched (e.g. unsorted timelines).
                // Leave its chunks alone; a warning was already logged.
            }
            Err(err) => {
                return Err(err.context(format!("VideoStream '{entity_path}'")));
            }
        }
    }

    if replaced_chunk_ids.is_empty() {
        return Ok(store.clone());
    }

    let new_config = ChunkStoreConfig::ALL_DISABLED; // So that we don't undo what we just did!
    let mut new_store = ChunkStore::new(store.id(), new_config);

    for chunk in store.iter_physical_chunks() {
        if !replaced_chunk_ids.contains(&chunk.id()) {
            new_store.insert_chunk(chunk)?;
        }
    }

    let mut overall_max_chunk_bytes: u64 = 0;
    for chunk in new_chunks {
        overall_max_chunk_bytes = overall_max_chunk_bytes.max(chunk.heap_size_bytes());
        new_store.insert_chunk(&Arc::new(chunk))?;
    }

    for chunk in keyframe_chunks {
        new_store.insert_chunk(&Arc::new(chunk))?;
    }

    /// Warn once per compaction if any rebatched chunk exceeds this size.
    ///
    /// GoP rebatching never splits a GoP across chunks, so streams with long
    /// keyframe intervals can produce chunks much larger than `chunk_max_bytes`.
    const LARGE_CHUNK_WARN_THRESHOLD: u64 = 10 * 1024 * 1024;

    if LARGE_CHUNK_WARN_THRESHOLD < overall_max_chunk_bytes {
        re_log::warn_once!(
            "GoP rebatching produced a video chunk of size {}. \
            Consider re-encoding the source with shorter keyframe intervals, \
            or turn off GoP-batching, or fix this code to allow splitting large GoPs-batches",
            format_bytes(overall_max_chunk_bytes as _)
        );
    }

    Ok(new_store)
}

/// Per-entity rebatch result.
enum EntityRebatch {
    /// Rebuild both sample chunks and the keyframe marker chunk. Existing
    /// dedicated `is_keyframe` chunks for this entity are dropped.
    Rebuild {
        /// Chunks that replace the original `VideoSample` chunks for this entity.
        rebatched: Vec<Chunk>,

        /// Marker chunk holding sparse `is_keyframe` rows for this entity, if any.
        /// Boxed to keep the `Rebuild` variant small enough to avoid a
        /// `clippy::large_enum_variant` warning vs. the unit `Skip` variant.
        new_keyframe_chunk: Option<Box<Chunk>>,
    },

    /// Replace sample chunks with `rebatched`, but leave the existing
    /// dedicated `is_keyframe` chunks alone — they're already canonical.
    KeepDedicatedKeyframeChunks { rebatched: Vec<Chunk> },

    /// Don't touch this entity's chunks. Used when rebatching can't be done
    /// safely (e.g. the sample chunks have unsorted timelines).
    Skip,
}

/// Rebatch a single video entity's sample chunks along GoP boundaries, and
/// emit a sparse dedicated `is_keyframe` marker chunk derived by parsing the
/// encoded samples.
///
/// When the user has supplied their own `is_keyframe` data (and `fix_keyframe`
/// is not set), validate it against the encoded samples:
/// - Canonical (correct labels in a pure dedicated chunk, no `false` rows):
///   the user's chunk is preserved verbatim.
/// - Correct but co-located with other components: rebuild a clean dedicated
///   chunk with the same content.
/// - Mismatched against the encoded samples, or any `false` row present: error.
fn rebatch_video_entity(
    store: &ChunkStore,
    config: &ChunkStoreConfig,
    is_start_of_gop: &dyn Fn(&[u8], VideoCodec) -> anyhow::Result<bool>,
    entity_path: &EntityPath,
    sample_chunks: &HashMap<ChunkId, ChunkShared>,
    dedicated_keyframe_chunks: Option<&HashMap<ChunkId, ChunkShared>>,
    fix_keyframe: bool,
) -> anyhow::Result<EntityRebatch> {
    re_tracing::profile_function!();

    for chunk in sample_chunks.values() {
        let unsorted_timelines: Vec<_> = chunk
            .timelines()
            .iter()
            .filter(|(_, tc)| !tc.is_sorted())
            .map(|(name, _)| name)
            .collect();
        if !unsorted_timelines.is_empty() {
            // We could try pick one of the timelines _are_ sorted (w/ relation to RowId),
            // but let's be better safe than sorry for now. Video playback on these
            // timelines may already be broken, and rebatching could make things worse.
            re_log::warn!(
                entity = %entity_path,
                chunk = %chunk.id(),
                ?unsorted_timelines,
                "skipping GoP rebatching: chunk has unsorted timelines (compared to RowId)"
            );
            return Ok(EntityRebatch::Skip);
        }
    }

    let timeline_name = *choose_timeline(sample_chunks)
        .ok_or_else(|| anyhow::anyhow!("no timeline found"))?
        .name();

    let codec = extract_codec(store, entity_path, timeline_name)
        .ok_or_else(|| anyhow::anyhow!("couldn't resolve video codec"))?;

    let sample_index = build_sample_index(is_start_of_gop, sample_chunks, timeline_name, codec)?;

    anyhow::ensure!(!sample_index.is_empty(), "no video samples found");

    // GoP-rebatch the sample chunks. This happens regardless of the keyframe
    // column's state — even when the keyframe column is already canonical, the
    // sample chunks might still need to be aligned to GoP boundaries.
    let gop_groups = split_into_gop_groups(entity_path, &sample_index);
    let gop_chunks: Vec<Chunk> = gop_groups
        .iter()
        .map(|group| chunk_from_gop(group, sample_chunks))
        .collect::<anyhow::Result<_, _>>()?;
    log_gop_stats(entity_path, &gop_chunks);
    let merged = merge_chunks(config, gop_chunks)?;
    log_entity_chunk_stats(entity_path, timeline_name, codec, &merged);

    // Decide what to do with the `is_keyframe` column.
    if !fix_keyframe {
        let user =
            collect_user_keyframe_labels(sample_chunks, dedicated_keyframe_chunks, timeline_name);

        if user.has_any_label {
            let codec_true: BTreeSet<TimeInt> = sample_index
                .iter()
                .filter(|s| s.is_start_of_gop)
                .map(|s| s.time)
                .collect();

            let mismatched = user.true_times != codec_true;
            let has_false = !user.false_times.is_empty();
            if mismatched || has_false {
                return Err(build_keyframe_validation_error(
                    &user.true_times,
                    &codec_true,
                    &user.false_times,
                ));
            }

            // Labels match the codec and no `false` rows are present. If every
            // is_keyframe row lives in a pure dedicated chunk, those chunks are
            // already what we'd emit — keep them.
            if !user.shares_chunk {
                return Ok(EntityRebatch::KeepDedicatedKeyframeChunks { rebatched: merged });
            }
            // Else fall through: move the `is_keyframe` column into a
            // dedicated chunk.
        }
    }

    let new_keyframe_chunk =
        build_keyframe_chunk(entity_path, sample_chunks, timeline_name, &sample_index)?;

    Ok(EntityRebatch::Rebuild {
        rebatched: merged,
        new_keyframe_chunk: new_keyframe_chunk.map(Box::new),
    })
}

/// Summary of user-supplied `VideoStream:is_keyframe` data for one entity.
#[derive(Default)]
struct UserKeyframeLabels {
    /// Times at which the user logged `is_keyframe=true`, on the chosen timeline.
    true_times: BTreeSet<TimeInt>,

    /// Times at which the user logged `is_keyframe=false`. Optimize refuses to
    /// run unless this is empty — no `false` should remain in the output.
    false_times: BTreeSet<TimeInt>,

    /// True if at least one `is_keyframe` row sits in a chunk that also carries
    /// any other component column (sample data, scalars, anything). The
    /// canonical layout is a dedicated chunk holding only `is_keyframe`.
    shares_chunk: bool,

    /// True if any `is_keyframe` value was logged at all.
    has_any_label: bool,
}

/// Walk every chunk for this entity and aggregate its user-supplied
/// `VideoStream:is_keyframe` labels into a [`UserKeyframeLabels`].
fn collect_user_keyframe_labels(
    sample_chunks: &HashMap<ChunkId, ChunkShared>,
    dedicated_keyframe_chunks: Option<&HashMap<ChunkId, ChunkShared>>,
    timeline_name: TimelineName,
) -> UserKeyframeLabels {
    let keyframe_component = VideoStream::descriptor_is_keyframe().component;

    let mut labels = UserKeyframeLabels::default();

    let mut process_chunk = |chunk: &ChunkShared| {
        if !chunk.components().contains_component(keyframe_component) {
            return;
        }
        if !chunk.timelines().contains_key(&timeline_name) {
            return;
        }
        let is_pure_keyframe_chunk = chunk
            .components()
            .values()
            .all(|c| c.descriptor.component == keyframe_component);
        for ((time, _row_id), value) in izip!(
            chunk.iter_component_indices(timeline_name, keyframe_component),
            chunk.iter_component::<IsKeyframe>(keyframe_component),
        ) {
            let Some(kf) = value.as_slice().first() else {
                continue;
            };
            labels.has_any_label = true;
            if !is_pure_keyframe_chunk {
                labels.shares_chunk = true;
            }
            if bool::from(kf.0) {
                labels.true_times.insert(time);
            } else {
                labels.false_times.insert(time);
            }
        }
    };

    for chunk in sample_chunks.values() {
        process_chunk(chunk);
    }
    if let Some(kf_chunks) = dedicated_keyframe_chunks {
        for chunk in kf_chunks.values() {
            process_chunk(chunk);
        }
    }

    labels
}

/// Build an actionable error describing how the user's `is_keyframe` labels
/// disagree with the codec, or that `false` rows are present.
fn build_keyframe_validation_error(
    user_true: &BTreeSet<TimeInt>,
    codec_true: &BTreeSet<TimeInt>,
    false_times: &BTreeSet<TimeInt>,
) -> anyhow::Error {
    const MAX_EXAMPLES: usize = 3;

    let missing: Vec<_> = codec_true.difference(user_true).copied().collect();
    let extra: Vec<_> = user_true.difference(codec_true).copied().collect();
    let false_examples: Vec<_> = false_times.iter().copied().collect();

    let fmt_examples = |times: &[TimeInt]| -> String {
        let head: Vec<_> = times
            .iter()
            .take(MAX_EXAMPLES)
            .map(|t| t.as_i64().to_string())
            .collect();
        if times.len() > MAX_EXAMPLES {
            format!("{} (+{} more)", head.join(", "), times.len() - MAX_EXAMPLES)
        } else {
            head.join(", ")
        }
    };

    let mut parts = Vec::new();
    if !missing.is_empty() {
        parts.push(format!(
            "{} codec keyframe(s) missing an `is_keyframe=true` label (e.g. at times {})",
            missing.len(),
            fmt_examples(&missing),
        ));
    }
    if !extra.is_empty() {
        parts.push(format!(
            "{} sample(s) labeled `is_keyframe=true` that are not codec keyframes (e.g. at times {})",
            extra.len(),
            fmt_examples(&extra),
        ));
    }
    if !false_examples.is_empty() {
        parts.push(format!(
            "{} `is_keyframe=false` row(s) (e.g. at times {}); `is_keyframe` is a sparse marker, only `true` should be logged",
            false_examples.len(),
            fmt_examples(&false_examples),
        ));
    }

    anyhow::anyhow!(
        "user-supplied `is_keyframe` data is incorrect: {}; \
         pass `--fix-keyframe` (Python: `fix_keyframe=True`) to drop the existing labels and re-derive them from the encoded samples",
        parts.join("; "),
    )
}

/// Build a sparse `is_keyframe` marker chunk for this entity.
///
/// Emits one row per keyframe sample, all with value `true`, on the
/// [`VideoStream::descriptor_is_keyframe`] descriptor. The result carries every
/// timeline present in the source sample chunks so that keyframe queries work
/// on any timeline the user logged on. Returns `None` if no sample in
/// `sample_index` was detected as a keyframe.
fn build_keyframe_chunk(
    entity_path: &EntityPath,
    sample_chunks: &HashMap<ChunkId, ChunkShared>,
    chosen_timeline: TimelineName,
    sample_index: &[SampleInfo],
) -> anyhow::Result<Option<Chunk>> {
    re_tracing::profile_function!();

    let keyframes: Vec<&SampleInfo> = sample_index.iter().filter(|s| s.is_start_of_gop).collect();
    let num_keyframes = keyframes.len();
    if num_keyframes == 0 {
        return Ok(None);
    }

    // All sample chunks for one entity share the same timeline set.
    let reference_chunk = sample_chunks
        .values()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no sample chunks"))?;

    let mut time_columns: Vec<TimeColumn> = Vec::with_capacity(reference_chunk.timelines().len());
    for (timeline_name, ref_tc) in reference_chunk.timelines() {
        let timeline = *ref_tc.timeline();
        let mut times: Vec<i64> = Vec::with_capacity(num_keyframes);
        for s in &keyframes {
            let src = &sample_chunks[&s.chunk_id];
            let src_tc = src.timelines().get(timeline_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "chunk {} missing timeline {timeline_name} \
                     (sample chunks must share the same timeline set)",
                    s.chunk_id,
                )
            })?;
            times.push(src_tc.times_raw()[s.row_index]);
        }
        // `build_sample_index` sorts by `chosen_timeline`, so the subset
        // on that timeline is monotonic. For other timelines, cross-chunk
        // interleaving may not preserve sort order — auto-detect.
        let is_sorted = (*timeline_name == chosen_timeline).then_some(true);
        time_columns.push(TimeColumn::new(
            is_sorted,
            timeline,
            ScalarBuffer::from(times),
        ));
    }

    // Build the component column as a single ListArray: `num_keyframes` rows,
    // each holding a one-element boolean batch with value `true`.
    let values: ArrayRef = Arc::new(BooleanArray::from(vec![true; num_keyframes]));
    let offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(1, num_keyframes));
    let field = Field::new("item", values.data_type().clone(), true);
    let list_array = ArrowListArray::try_new(field.into(), offsets, values, None)
        .map_err(|err| anyhow::anyhow!("failed to build keyframe list array: {err}"))?;

    let chunk = Chunk::from_columns(
        entity_path.clone(),
        time_columns,
        [(VideoStream::descriptor_is_keyframe(), list_array)],
    )
    .map_err(|err| anyhow::anyhow!("failed to build keyframe chunk: {err}"))?;

    Ok(Some(chunk))
}

/// Pick the best timeline for sorting video samples.
fn choose_timeline(sample_chunks: &HashMap<ChunkId, ChunkShared>) -> Option<Timeline> {
    let mut counts: HashMap<Timeline, u64> = Default::default();
    for chunk in sample_chunks.values() {
        for tc in chunk.timelines().values() {
            *counts.entry(*tc.timeline()).or_default() += chunk.num_rows() as u64;
        }
    }

    if counts.is_empty() {
        return None;
    }

    let timelines: Vec<_> = counts.keys().copied().collect();
    Some(Timeline::pick_best_timeline(&timelines, |t| {
        counts.get(t).copied().unwrap_or(0)
    }))
}

fn extract_codec(
    store: &ChunkStore,
    entity_path: &EntityPath,
    timeline_name: TimelineName,
) -> Option<VideoCodec> {
    let codec_component = VideoStream::descriptor_codec().component;

    let results = store.latest_at_relevant_chunks(
        ChunkTrackingMode::PanicOnMissing,
        &crate::LatestAtQuery::new(timeline_name, TimeInt::MAX),
        entity_path,
        codec_component,
    );

    results
        .chunks
        .iter()
        .flat_map(|chunk| chunk.iter_component::<VideoCodec>(codec_component))
        .find_map(|codec| codec.as_slice().first().copied())
}

/// Build an index of all video samples across all chunks for one entity.
///
/// Returns a flat list of [`SampleInfo`], sorted by time.
fn build_sample_index(
    is_start_of_gop: &dyn Fn(&[u8], VideoCodec) -> anyhow::Result<bool>,
    sample_chunks: &HashMap<ChunkId, ChunkShared>,
    timeline_name: TimelineName,
    codec: VideoCodec,
) -> anyhow::Result<Vec<SampleInfo>> {
    re_tracing::profile_function!();

    let sample_component = VideoStream::descriptor_sample().component;

    let mut sample_index = Vec::new();

    for chunk in sample_chunks.values() {
        if !chunk.timelines().contains_key(&timeline_name) {
            anyhow::bail!(
                "chunk {} has no values on timeline {timeline_name}",
                chunk.id()
            );
        }

        let chunk_id = chunk.id();

        // We need the positional row index to later extract rows via `taken()`.
        let row_id_to_index: HashMap<_, _> = chunk
            .row_ids()
            .enumerate()
            .map(|(idx, rid)| (rid, idx))
            .collect();

        // `iter_component_indices` only yields rows where the component is non-null,
        // which is exactly what we want — skip rows without a sample.
        for ((time, row_id), sample) in izip!(
            chunk.iter_component_indices(timeline_name, sample_component),
            chunk.iter_component::<VideoSample>(sample_component)
        ) {
            let Some(sample) = sample.as_slice().first() else {
                continue;
            };

            let row_index = row_id_to_index[&row_id];

            sample_index.push(SampleInfo {
                chunk_id,
                row_index,
                time,
                is_start_of_gop: is_start_of_gop(sample.0.inner().as_slice(), codec)?,
            });
        }
    }

    sample_index.sort_by_key(|sample| (sample.time, sample.chunk_id, sample.row_index));
    Ok(sample_index)
}

/// Split a sorted sample index into groups, one per GoP.
///
/// Each group starts at a keyframe sample, except possibly the first
/// group which collects any orphan frames before the first keyframe.
fn split_into_gop_groups<'a>(
    entity: &EntityPath,
    sample_index: &'a [SampleInfo],
) -> Vec<&'a [SampleInfo]> {
    re_tracing::profile_function!();

    if sample_index.is_empty() {
        return Vec::new();
    }

    // Find indices where new GoPs start.
    let mut split_points: Vec<usize> = sample_index
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_start_of_gop)
        .map(|(i, _)| i)
        .collect();

    // If the first sample isn't a keyframe, include the leading orphan group.
    if split_points.first().copied() != Some(0) {
        re_log::warn!(?entity, "first sample is not a keyframe");
        split_points.insert(0, 0);
    }

    split_points
        .windows(2)
        .map(|w| &sample_index[w[0]..w[1]])
        .chain(std::iter::once(
            &sample_index[*split_points.last().unwrap_or(&0)..],
        ))
        .filter(|group| !group.is_empty())
        .collect()
}

/// Materialize a GoP group into a single [`Chunk`] by extracting rows from source chunks.
///
/// Uses `Chunk::taken()` to batch-extract rows from the same source chunk,
/// then concatenates the per-source-chunk results.
fn chunk_from_gop(
    group: &[SampleInfo],
    chunks_by_id: &HashMap<ChunkId, ChunkShared>,
) -> anyhow::Result<Chunk> {
    re_tracing::profile_function!();

    // Group row indices by source chunk, preserving the order of first appearance.
    let mut rows_per_chunk: BTreeMap<ChunkId, Vec<i32>> = BTreeMap::new();
    let mut chunk_order: Vec<ChunkId> = Vec::new();
    for sample in group {
        let row_index = i32::try_from(sample.row_index)
            .map_err(|_err| anyhow::anyhow!("row index {} exceeds i32::MAX", sample.row_index))?;

        rows_per_chunk
            .entry(sample.chunk_id)
            .or_insert_with(|| {
                chunk_order.push(sample.chunk_id);
                Vec::new()
            })
            .push(row_index);
    }

    let mut result: Option<Chunk> = None;
    for chunk_id in &chunk_order {
        let source_chunk = &chunks_by_id[chunk_id];
        let indices = &rows_per_chunk[chunk_id];

        let indices_array = arrow::array::Int32Array::from(indices.clone());
        // `VideoStream:is_keyframe` is a deliberate exception to the convention
        // that components of the same archetype share a chunk (see compact.rs).
        // We split it off because keyframe queries should be cheap — pulling the
        // multi-MiB sample column to read a 1-bit-per-row signal defeats the point.
        // Optimize emits its own sparse marker chunk via `build_keyframe_chunk`.
        let extracted = source_chunk
            .taken(&indices_array)
            .component_dropped(VideoStream::descriptor_is_keyframe().component);

        result = Some(match result {
            None => extracted,
            Some(prev) => Chunk::concat_and_sort(&prev, &extracted)?,
        });
    }

    let mut chunk = result.ok_or_else(|| anyhow::anyhow!("GoP group is empty — this is a bug"))?;
    chunk.sort_by_row_ids_if_needed();
    Ok(chunk)
}

/// Merge consecutive GoP chunks together as long as the combined size stays within
/// `chunk_max_bytes`. If `chunk_max_bytes` is 0, no merging is done.
///
/// The input chunks must already be sorted by time.
fn merge_chunks(config: &ChunkStoreConfig, gop_chunks: Vec<Chunk>) -> anyhow::Result<Vec<Chunk>> {
    re_tracing::profile_function!();

    let chunk_max_bytes = config.chunk_max_bytes;

    re_log::debug!(
        num_gops = gop_chunks.len(),
        chunk_max_bytes = %format_bytes(chunk_max_bytes as _),
        "merging GoPs into chunks"
    );

    if chunk_max_bytes == 0 || gop_chunks.len() <= 1 {
        re_log::debug!("skipping merge (max_bytes=0 or ≤1 GoP)");
        return Ok(gop_chunks);
    }

    let mut merged: Vec<Chunk> = Vec::new();
    let mut accumulator: Option<Chunk> = None;
    let mut accumulator_bytes: u64 = 0;

    for gop in gop_chunks {
        let gop_bytes = gop.heap_size_bytes();

        if let Some(acc) = accumulator.take() {
            if accumulator_bytes + gop_bytes <= chunk_max_bytes {
                let combined = Chunk::concat_and_sort(&acc, &gop)?;
                accumulator_bytes += gop_bytes;
                accumulator = Some(combined);
                continue;
            } else {
                merged.push(acc);
            }
        }

        accumulator_bytes = gop_bytes;
        accumulator = Some(gop);
    }

    if let Some(acc) = accumulator {
        merged.push(acc);
    }

    re_log::debug!(
        num_gops_in = merged.iter().map(|c| c.num_rows()).sum::<usize>(),
        num_chunks_out = merged.len(),
        "merge complete"
    );

    Ok(merged)
}

fn log_gop_stats(entity_path: &EntityPath, gop_chunks: &[Chunk]) {
    if gop_chunks.is_empty() {
        return;
    }

    let num_gops = gop_chunks.len() as u64;

    let gop_frames: Vec<u64> = gop_chunks.iter().map(|c| c.num_rows() as u64).collect();
    let gop_bytes: Vec<u64> = gop_chunks.iter().map(|c| c.heap_size_bytes()).collect();

    let min_frames = gop_frames.iter().copied().min().unwrap_or(0);
    let max_frames = gop_frames.iter().copied().max().unwrap_or(0);
    let avg_frames = gop_frames.iter().sum::<u64>() / num_gops;

    let min_bytes = gop_bytes.iter().copied().min().unwrap_or(0);
    let max_bytes = gop_bytes.iter().copied().max().unwrap_or(0);
    let avg_bytes = gop_bytes.iter().sum::<u64>() / num_gops;

    re_log::info!(
        entity = %entity_path,
        num_gops,
        frames_per_gop = %format!("{min_frames}/{avg_frames}/{max_frames}"),
        bytes_per_gop = %format!(
            "{}/{}/{}",
            format_bytes(min_bytes as _),
            format_bytes(avg_bytes as _),
            format_bytes(max_bytes as _),
        ),
        "GoP stats (min/avg/max)"
    );
}

fn log_entity_chunk_stats(
    entity_path: &EntityPath,
    timeline_name: TimelineName,
    codec: VideoCodec,
    chunks: &[Chunk],
) {
    let num_chunks = chunks.len() as u64;
    if num_chunks == 0 {
        return;
    }

    let chunk_frames: Vec<u64> = chunks.iter().map(|c| c.num_rows() as u64).collect();
    let chunk_bytes: Vec<u64> = chunks.iter().map(|c| c.heap_size_bytes()).collect();

    let total_frames: u64 = chunk_frames.iter().sum();
    let min_frames = chunk_frames.iter().copied().min().unwrap_or(0);
    let max_frames = chunk_frames.iter().copied().max().unwrap_or(0);
    let avg_frames = total_frames / num_chunks;

    let total_bytes: u64 = chunk_bytes.iter().sum();
    let min_bytes = chunk_bytes.iter().copied().min().unwrap_or(0);
    let max_bytes = chunk_bytes.iter().copied().max().unwrap_or(0);
    let avg_bytes = total_bytes / num_chunks;

    re_log::info!(
        entity = %entity_path,
        timeline = %timeline_name,
        codec = ?codec,
        num_chunks,
        total_frames,
        frames_per_chunk = %format!("{min_frames}/{avg_frames}/{max_frames}"),
        bytes_per_chunk = %format!(
            "{}/{}/{}",
            format_bytes(min_bytes as _),
            format_bytes(avg_bytes as _),
            format_bytes(max_bytes as _),
        ),
        "rebatched video entity (min/avg/max)"
    );
}
