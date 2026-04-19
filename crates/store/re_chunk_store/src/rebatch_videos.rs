use std::collections::BTreeMap;
use std::sync::Arc;

use ahash::{HashMap, HashSet};
use itertools::izip;
use re_byte_size::SizeBytes as _;
use re_chunk::{Chunk, ChunkId, ChunkShared, EntityPath, Timeline, TimelineName};
use re_format::format_bytes;
use re_log_types::TimeInt;
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::{VideoCodec, VideoSample};

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
) -> anyhow::Result<ChunkStore> {
    re_tracing::profile_function!();

    let sample_component = VideoStream::descriptor_sample().component;

    // Collect all temporal chunks that contain video samples, grouped by entity.
    let mut sample_chunks_per_entity: HashMap<EntityPath, HashMap<ChunkId, ChunkShared>> =
        Default::default();
    for chunk in store.iter_physical_chunks() {
        if !chunk.is_static() && chunk.components().contains_component(sample_component) {
            sample_chunks_per_entity
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

    re_log::info!(
        num_video_entities = sample_chunks_per_entity.len(),
        "found video entities for GoP realignment"
    );

    for (entity_path, sample_chunks) in &sample_chunks_per_entity {
        match rebatch_video_entity(store, config, is_start_of_gop, entity_path, sample_chunks) {
            Ok(new_entity_chunks) => {
                replaced_chunk_ids.extend(sample_chunks.keys().copied());
                new_chunks.extend(new_entity_chunks);
            }
            Err(err) => {
                re_log::warn!(entity = %entity_path, %err, "failed to rebatch video entity, skipping");
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

/// Rebatch a single video entity's chunks along GoP boundaries.
///
/// Returns the new chunks that replace the old ones.
fn rebatch_video_entity(
    store: &ChunkStore,
    config: &ChunkStoreConfig,
    is_start_of_gop: &dyn Fn(&[u8], VideoCodec) -> anyhow::Result<bool>,
    entity_path: &EntityPath,
    sample_chunks: &HashMap<ChunkId, ChunkShared>,
) -> anyhow::Result<Vec<Chunk>> {
    re_tracing::profile_function!();

    let timeline_name =
        choose_timeline(sample_chunks).ok_or_else(|| anyhow::anyhow!("no timeline found"))?;

    let codec = extract_codec(store, entity_path, timeline_name)
        .ok_or_else(|| anyhow::anyhow!("couldn't resolve video codec"))?;

    let sample_index = build_sample_index(is_start_of_gop, sample_chunks, timeline_name, codec)?;

    anyhow::ensure!(!sample_index.is_empty(), "no video samples found");

    let gop_groups = split_into_gop_groups(entity_path, &sample_index);

    // Materialize each GoP into its own chunk:
    let gop_chunks: Vec<Chunk> = gop_groups
        .iter()
        .map(|group| chunk_from_gop(group, sample_chunks))
        .collect::<anyhow::Result<_, _>>()?;

    log_gop_stats(entity_path, &gop_chunks);

    // Merge consecutive GoP chunks as long as the total stays within chunk_max_bytes.
    let merged = merge_chunks(config, gop_chunks)?;

    log_entity_chunk_stats(entity_path, timeline_name, codec, &merged);

    Ok(merged)
}

/// Pick the best timeline for sorting video samples.
fn choose_timeline(sample_chunks: &HashMap<ChunkId, ChunkShared>) -> Option<TimelineName> {
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
    let best = Timeline::pick_best_timeline(&timelines, |t| counts.get(t).copied().unwrap_or(0));
    Some(*best.name())
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
                vec![row_index]
            })
            .push(row_index);
    }

    let mut result: Option<Chunk> = None;
    for chunk_id in &chunk_order {
        let source_chunk = &chunks_by_id[chunk_id];
        let indices = &rows_per_chunk[chunk_id];

        let indices_array = arrow::array::Int32Array::from(indices.clone());
        let extracted = source_chunk.taken(&indices_array);

        result = Some(match result {
            None => extracted,
            Some(prev) => prev.concatenated(&extracted)?,
        });
    }

    let mut chunk = result.ok_or_else(|| anyhow::anyhow!("GoP group is empty — this is a bug"))?;
    chunk.sort_if_unsorted();
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
                let combined = acc.concatenated(&gop)?;
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
