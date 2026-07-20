//! IO-side pipeline: batches the server's chunk-info rows into `FetchChunks`
//! requests, dispatches them concurrently (gRPC + direct-URL hybrid where
//! available), and feeds decoded chunks into the CPU worker via the
//! [`CpuWorkerMsg`] channel.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use arrow::array::{RecordBatch, StringArray};
use futures::StreamExt as _;
use re_dataframe::TimelineName;
use re_log_types::TimeInt;
use re_protos::cloud::v1alpha1::ext::QueryDatasetDataframe;

use re_redap_client::{ApiError, ApiResult};
use re_types_core::SegmentId;
use tokio::sync::mpsc::Sender;
use tracing::Instrument as _;

use crate::analytics::{QueryErrorKind, TaskFetchStats};
use crate::chunk_fetcher::{
    ChunksWithSegment, SortedChunksWithSegment, batch_byte_size, batch_byte_size_uncompressed,
    batch_has_any_direct_urls, fetch_batch_direct, fetch_batch_group_via_grpc,
    split_batch_by_direct_url,
};
use crate::dataframe_query_common::{DataframeClientAPI, force_grpc};
use crate::metrics_capture::QueryMetrics;
use crate::pipeline_budget::{MAX_CONCURRENT_SEGMENTS, PipelineBudget};
use crate::segment_chunk_manifest::SegmentChunkManifest;
use re_dataframe::external::re_chunk::{Chunk, TimeColumn};

use super::cpu_worker::CpuWorkerMsg;

/// Target batch size in bytes for grouping segments together in requests.
/// This reduces the number of round-trips while keeping memory usage bounded (as long
/// as the concurrency is also bounded).
const TARGET_BATCH_SIZE_BYTES: usize = 8 * 1024 * 1024; // 8 MB

/// How many concurrent requests to make to the server when fetching chunks.
const GRPC_BATCH_SIZE: usize = 12;

/// Max batch-level futures in-flight at once in the IO pipeline.
/// This bounds both concurrency and the reorder buffer size.
const IO_PIPELINE_BUFFER: usize = 24;

/// Connect timeout for direct-URL (S3) fetches. Bounds DNS + TCP + TLS
/// setup so a hung connect cannot indefinitely hold one of the scarce
/// process-global direct-fetch permits. Mirrors the gRPC client's 10s.
const DIRECT_FETCH_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Idle read timeout for direct-URL fetches: the max gap between received
/// bytes. Idle-based (not a total-request cap) so a large-but-progressing
/// 16 MB merged range is never falsely cancelled, while a stalled body
/// still releases its permit.
const DIRECT_FETCH_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Build a [`SegmentChunkManifest`] per segment from the `:start` column
/// the server attaches to each `chunk_info` row when the query has a
/// temporal `filtered_index`.
///
/// Returns an empty map if `filtered_timeline` is `None` (static-only
/// query — no horizon math is possible) or if **any** batch is missing
/// the `:start` column (old server, or mixed-schema response). In both
/// fallback cases the CPU worker falls back to "emit at completion"
/// semantics for every segment, governed by
/// [`CpuWorkerMsg::SegmentChunkCount`].
///
/// All-or-nothing across batches is load-bearing: if we built entries
/// only from the batches that *do* carry `:start`, segments split
/// across mixed batches would end up with partial entries; `lock()`
/// would then commit a manifest that under-counts the segment, and
/// every later `record_arrival` for a chunk that came from a
/// no-`:start` batch would fire `debug_panic!` and pin `safe_horizon`
/// at the laggard's `time_min` forever.
///
/// Static chunks (rows with `chunk_is_static = true`) and rows with a
/// null `:start` value are deliberately skipped: they carry no time
/// semantics on the filtered timeline, so they can never gate or
/// advance the safe horizon.
fn build_segment_manifests(
    chunk_infos: &[RecordBatch],
    filtered_timeline: Option<TimelineName>,
) -> ApiResult<HashMap<SegmentId, SegmentChunkManifest>> {
    let mut manifests: HashMap<SegmentId, SegmentChunkManifest> = HashMap::new();
    let Some(timeline_name) = filtered_timeline else {
        return Ok(manifests);
    };
    // `TimelineName: Display` renders its interned string, no extra allocation.
    let start_col_name = format!("{timeline_name}:start");

    // All-or-nothing pre-check: if any batch lacks `:start`, fall back
    // to completion-only flush for the whole query. See doc comment for
    // why partial-batch builds are unsafe.
    if chunk_infos
        .iter()
        .any(|rb| rb.column_by_name(&start_col_name).is_none())
    {
        return Ok(manifests);
    }

    for rb in chunk_infos {
        let start_col = rb
            .column_by_name(&start_col_name)
            .expect("pre-check above guarantees presence on every batch");
        // `:start` carries the raw `i64` for the timeline's `time_min`. The OSS server emits
        // `Int64`; other servers may emit `TimestampNanosecondArray` / `Time64NanosecondArray` /
        // `DurationNanosecondArray` matching the timeline's native dtype. All four are i64 under
        // the hood, so we go through `TimeColumn::read_nullable_array` rather than downcasting.
        let (start_values, start_nulls) = TimeColumn::read_nullable_array(start_col.as_ref())
            .map_err(|err| {
                ApiError::internal(format!(
                    "`{start_col_name}` column has unsupported type: {err}"
                ))
            })?;
        let segment_ids = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID
            .extract(rb)
            .map_err(ApiError::internal_quiver)?;
        let entity_paths = QueryDatasetDataframe::COLUMN_CHUNK_ENTITY_PATH
            .extract(rb)
            .map_err(ApiError::internal_quiver)?;
        let is_statics = QueryDatasetDataframe::COLUMN_CHUNK_IS_STATIC
            .extract(rb)
            .map_err(ApiError::internal_quiver)?;

        for i in 0..rb.num_rows() {
            if start_nulls.as_ref().is_some_and(|n| n.is_null(i)) {
                continue;
            }
            if is_statics.value(i) {
                continue;
            }
            let time_min = TimeInt::saturated_temporal_i64(start_values[i]);
            // Defense-in-depth: `saturated_temporal_i64` currently can't
            // produce `STATIC` because `NonMinI64` clamps to `MIN+1`, but
            // the manifest's correctness depends on that invariant
            // holding forever. `STATIC` sorts below every temporal value
            // under the derived `Ord`, so a single leak would pin
            // `safe_horizon` and stall the segment. Drop instead of
            // letting `SegmentChunkManifest::expect_chunk` fire its
            // `debug_panic!`.
            if time_min.is_static() {
                continue;
            }
            let seg = segment_ids.value_owned(i);
            let entity = entity_paths.value_owned(i);
            manifests
                .entry(seg)
                .or_default()
                .expect_chunk(entity, time_min);
        }
    }

    // Every manifest we built has now seen every announced chunk for
    // its segment — lock so the worker can start consulting
    // `safe_horizon`. Segments with NO temporal chunks (all static, or
    // `:start` absent for them) won't appear here and the worker
    // handles them via the count-based path.
    for m in manifests.values_mut() {
        m.lock();
    }

    Ok(manifests)
}

/// Count chunks per segment across the raw `chunk_info` batches.
///
/// The CPU worker uses these counts to detect segment completion — it
/// fires the per-segment flush as soon as the inserted-chunks count
/// reaches the announced number, rather than waiting for the channel
/// to close. Result ordering is not load-bearing: the worker is keyed
/// on `segment_id` and tolerates announce/chunk arrival in any order.
/// First-encounter order is preserved purely for trace readability.
fn count_chunks_per_segment(chunk_infos: &[RecordBatch]) -> ApiResult<Vec<(SegmentId, usize)>> {
    let mut counts: HashMap<SegmentId, usize> = HashMap::new();
    let mut order: Vec<SegmentId> = Vec::new();
    for rb in chunk_infos {
        let segment_ids = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID
            .extract(rb)
            .map_err(ApiError::internal_quiver)?;
        for seg in &segment_ids {
            // Avoid the `SegmentId::from` allocation on the hot repeated-segment path:
            // only take it when we are actually inserting a new entry.
            if let Some(c) = counts.get_mut(seg) {
                *c += 1;
            } else {
                let seg = SegmentId::from(seg);
                order.push(seg.clone());
                counts.insert(seg, 1);
            }
        }
    }
    Ok(order
        .into_iter()
        .map(|s| {
            let c = counts.remove(&s).unwrap_or(0);
            (s, c)
        })
        .collect())
}

/// Extend `seen` / `order` with the distinct `segment_id`s present
/// in a fetch batch, preserving first-seen order. Used by the IO
/// side to feed the budget's segment-count gate atomically with the
/// byte reservation. Taking a shared `seen` lets a multi-batch fetch
/// dedup across batches in one pass instead of building per-batch
/// `Vec`s and re-deduping at the caller.
fn extend_distinct_segment_ids(
    batch: &RecordBatch,
    seen: &mut HashSet<String>,
    order: &mut Vec<String>,
) -> ApiResult<()> {
    let seg_col = batch
        .column_by_name(QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID_NAME)
        .ok_or_else(|| ApiError::internal("missing segment_id column in fetch batch"))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| ApiError::internal("segment_id column is not a string array"))?;
    for i in 0..batch.num_rows() {
        let s = seg_col.value(i);
        if !seen.contains(s) {
            seen.insert(s.to_owned());
            order.push(s.to_owned());
        }
    }
    Ok(())
}

/// Smallest `time_min` (i.e. earliest `:start` value) across the
/// chunks in a fetch batch, on the query's `filtered_index`
/// timeline. Used by the IO side as the budget's priority key —
/// reservations with the lowest `task_time_min` wake first so the
/// horizon-advancing chunk preempts later-time chunks under
/// saturation.
///
/// Returns [`TimeInt::MAX`] (back-of-the-queue priority) when:
/// * `filtered_timeline` is `None` (static-only query),
/// * the `{timeline}:start` column is absent (old server),
/// * the `{timeline}:start` column has an unsupported dtype, or
/// * every row in the batch has a null `:start` (static chunks only).
///
/// `:start` carries the raw `i64` for the timeline's `time_min`. The OSS
/// server emits `Int64`; other servers may emit `TimestampNanosecondArray`
/// / `Time64NanosecondArray` / `DurationNanosecondArray` matching the
/// timeline's native dtype. All four are i64 under the hood, so we go
/// through [`TimeColumn::read_nullable_array`] rather than downcasting —
/// matches what [`build_segment_manifests`] does for the same column.
fn extract_task_time_min(batch: &RecordBatch, filtered_timeline: Option<&str>) -> TimeInt {
    let Some(timeline) = filtered_timeline else {
        return TimeInt::MAX;
    };
    let col_name = format!("{timeline}:start");
    let Some(start_col) = batch.column_by_name(&col_name) else {
        return TimeInt::MAX;
    };
    let Ok((start_values, start_nulls)) = TimeColumn::read_nullable_array(start_col.as_ref())
    else {
        return TimeInt::MAX;
    };
    let mut min_seen: Option<i64> = None;
    for i in 0..start_values.len() {
        if start_nulls.as_ref().is_some_and(|n| n.is_null(i)) {
            continue;
        }
        let v = start_values[i];
        min_seen = Some(min_seen.map_or(v, |m| m.min(v)));
    }
    min_seen.map_or(TimeInt::MAX, TimeInt::saturated_temporal_i64)
}

/// Extract segment ID from a `chunk_info` `RecordBatch`. Each `chunk_info` batch contains
/// chunks *for a single segment*, hence we can just take the first row's `segment_id`. This is
/// guaranteed by the implementation in `group_chunk_infos_by_segment_id`.
fn extract_segment_id(chunk_info: &RecordBatch) -> ApiResult<SegmentId> {
    let segment_ids = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID
        .extract(chunk_info)
        .map_err(ApiError::internal_quiver)?;

    Ok(segment_ids.value_owned(0))
}

/// Extract chunk sizes (`chunk_byte_len` values) from a `chunk_info` `RecordBatch`.
fn extract_chunk_sizes(chunk_info: &RecordBatch) -> ApiResult<quiver::Column<u64>> {
    QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN
        .extract(chunk_info)
        .map_err(ApiError::internal_quiver)
}

type BatchingResult = (Vec<RecordBatch>, Vec<SegmentId>);

enum FetchTask {
    Direct(RecordBatch),
    Grpc(RecordBatch),
}

fn split_batches_into_fetch_tasks(batches: &[RecordBatch]) -> (Vec<FetchTask>, usize, usize) {
    let mut work_items: Vec<FetchTask> = Vec::new();
    let mut n_direct = 0usize;
    let mut n_grpc = 0usize;
    for batch in batches {
        let (direct_batch, grpc_batch) = split_batch_by_direct_url(batch);
        if let Some(b) = direct_batch {
            n_direct += 1;
            work_items.push(FetchTask::Direct(b));
        }
        if let Some(b) = grpc_batch {
            n_grpc += 1;
            work_items.push(FetchTask::Grpc(b));
        }
    }
    (work_items, n_direct, n_grpc)
}

/// Groups `chunk_infos` into batches targeting the specified size, with special handling
/// for segments larger than the target size (which get split). Batches smaller than `target_size`
/// are merged together to reduce the number of requests.
///
/// Returns (batches, `segment_order`) where:
/// - batches: list of merged `RecordBatch`es, each representing a `target_size` request
/// - `segment_order`: Original order of segments for preserving segment order
#[tracing::instrument(
    level = "info",
    skip_all,
    fields(
        num_chunk_infos = chunk_infos.len(),
        target_size_bytes,
        output_batches,
        byte_target_flushes,
        segment_limit_flushes,
        large_segment_batches,
        end_of_input_batches,
    )
)]
fn create_request_batches(
    chunk_infos: Vec<RecordBatch>,
    target_size_bytes: u64,
) -> ApiResult<BatchingResult> {
    re_tracing::profile_function!();
    let merge_err = |err: arrow::error::ArrowError, ctx: &'static str| {
        ApiError::deserialization_with_source(None, err, ctx)
    };

    let mut request_batches = Vec::new();
    let mut current_batch = Vec::new();
    let mut current_batch_size = 0u64;
    let mut current_batch_segments: HashSet<SegmentId> = HashSet::new();
    let mut segment_order = Vec::new();
    let mut segment_seen = HashSet::new();
    let mut byte_target_flushes = 0usize;
    let mut segment_limit_flushes = 0usize;
    let mut large_segment_batches = 0usize;
    let mut end_of_input_batches = 0usize;

    for chunk_info in chunk_infos {
        let segment_id = extract_segment_id(&chunk_info)?;
        let chunk_sizes = extract_chunk_sizes(&chunk_info)?;
        let segment_size: u64 = chunk_sizes.iter().sum();

        // Track original segment order
        if segment_seen.insert(segment_id.clone()) {
            segment_order.push(segment_id.clone());
        }

        // Check if this chunk_info would push the current batch past
        // either the byte target OR the segment-count cap. The
        // segment-count check matters when small segments would
        // otherwise merge >`MAX_CONCURRENT_SEGMENTS` distinct
        // segments into a single fetch: the resulting reservation
        // could never satisfy the segment-count gate in
        // `PipelineBudget::try_admit` and would deadlock.
        let adds_new_segment = !current_batch_segments.contains(&segment_id);
        let would_exceed_size = current_batch_size + segment_size > target_size_bytes;
        let would_exceed_segments =
            adds_new_segment && current_batch_segments.len() >= MAX_CONCURRENT_SEGMENTS;
        if !current_batch.is_empty() && (would_exceed_size || would_exceed_segments) {
            if would_exceed_segments {
                segment_limit_flushes += 1;
            } else {
                byte_target_flushes += 1;
            }
            // Merge current batch and add to results
            let merged_batch = re_arrow_util::concat_polymorphic_batches(&current_batch)
                .map_err(|err| merge_err(err, "merging chunk-info batches"))?;
            request_batches.push(merged_batch);
            current_batch = Vec::new();
            current_batch_size = 0;
            current_batch_segments.clear();
        }

        // Split the large segment into multiple requests
        if segment_size > target_size_bytes {
            // If current batch is not empty, merge and send it first
            if !current_batch.is_empty() {
                byte_target_flushes += 1;
                let merged_batch = re_arrow_util::concat_polymorphic_batches(&current_batch)
                    .map_err(|err| merge_err(err, "merging chunk-info batches"))?;
                request_batches.push(merged_batch);
                current_batch = Vec::new();
                current_batch_size = 0;
                current_batch_segments.clear();
            }

            let split_batches =
                split_large_segments(&segment_id, &chunk_info, target_size_bytes, &chunk_sizes)?;
            large_segment_batches += split_batches.len();

            // Split batches are already individual RecordBatches, add them directly
            for split_batch in split_batches {
                request_batches.push(split_batch);
            }
        } else {
            current_batch.push(chunk_info);
            current_batch_size += segment_size;
            current_batch_segments.insert(segment_id);
        }
    }

    // Don't forget to merge the last batch
    if !current_batch.is_empty() {
        let merged_batch = re_arrow_util::concat_polymorphic_batches(&current_batch)
            .map_err(|err| merge_err(err, "merging final chunk-info batch"))?;
        request_batches.push(merged_batch);
        end_of_input_batches += 1;
    }

    re_log::debug_assert_eq!(
        request_batches.len(),
        byte_target_flushes + segment_limit_flushes + large_segment_batches + end_of_input_batches,
        "every planned batch must have exactly one flush reason"
    );

    let span = tracing::Span::current();
    span.record("output_batches", request_batches.len());
    span.record("byte_target_flushes", byte_target_flushes);
    span.record("segment_limit_flushes", segment_limit_flushes);
    span.record("large_segment_batches", large_segment_batches);
    span.record("end_of_input_batches", end_of_input_batches);

    tracing::debug!(
        "Batching complete: {} segments → {} batches (target_size={}KB)",
        segment_order.len(),
        request_batches.len(),
        target_size_bytes / 1024
    );

    Ok((request_batches, segment_order))
}

/// Split segment larger than target size into multiple smaller requests. Each request will contain
/// a subset of the chunks from the original segment, targeting approximately the desired size.
fn split_large_segments(
    segment_id: &SegmentId,
    chunk_info: &RecordBatch,
    target_size: u64,
    chunk_sizes: &quiver::Column<u64>,
) -> ApiResult<Vec<RecordBatch>> {
    re_tracing::profile_function!();
    let take_err = |err: arrow::error::ArrowError| {
        ApiError::deserialization_with_source(None, err, "slicing large segment into sub-batches")
    };

    let mut result_batches = Vec::new();
    let mut current_indices = Vec::new();
    let mut current_size = 0u64;

    for row_idx in 0..chunk_info.num_rows() {
        let chunk_size = chunk_sizes[row_idx];

        // Always include at least one chunk per batch (even if it exceeds target)
        if current_indices.is_empty() || current_size + chunk_size <= target_size {
            current_indices.push(row_idx);
            current_size += chunk_size;
        } else {
            // Create batch from current indices
            let batch =
                re_arrow_util::take_record_batch(chunk_info, &current_indices).map_err(take_err)?;
            result_batches.push(batch);

            // Start new batch with current chunk
            current_indices = vec![row_idx];
            current_size = chunk_size;
        }
    }

    // Don't forget the last batch
    if !current_indices.is_empty() {
        let batch =
            re_arrow_util::take_record_batch(chunk_info, &current_indices).map_err(take_err)?;
        result_batches.push(batch);
    }

    tracing::debug!(
        "Split large segment '{}' ({}) into {} requests",
        segment_id,
        re_format::format_bytes(chunk_sizes.iter().sum::<u64>() as _),
        result_batches.len()
    );

    Ok(result_batches)
}

/// Helper function to sort chunks by segment order.
/// This function handles the fact we send concurrent requests where sometimes even a
/// single request can contain chunks from multiple segments (due to batching) and the more
/// important fact that server provides no ordering guarantees.
fn sort_chunks_by_segment_order(
    chunks: Vec<ChunksWithSegment>,
    segment_order: &[SegmentId],
) -> Vec<SortedChunksWithSegment> {
    // Collect all individual chunks grouped by segment ID (we don't care about ordering of individual
    // chunks within a segment here)
    let mut segment_groups: HashMap<SegmentId, Vec<Chunk>> = HashMap::default();

    // Extract all chunks and group by segment
    for chunks_with_segment in chunks {
        for (chunk, segment_id_opt) in chunks_with_segment {
            let Some(segment_id) = segment_id_opt else {
                continue;
            };
            segment_groups.entry(segment_id).or_default().push(chunk);
        }
    }

    // Rebuild chunks in the correct segment order
    segment_order
        .iter()
        .filter_map(|segment_id| segment_groups.remove_entry(segment_id))
        .collect()
}

/// Helper to sort and send chunks through the output channel, preserving segment order.
/// Returns `false` if the output channel is closed (consumer dropped).
///
/// The in-batch sort by `global_segment_order` is retained because it's
/// cheap and keeps related segments contiguous within a single fetch
/// result, which makes traces easier to read. The CPU worker no longer
/// requires it for correctness — the `HashMap` routing handles
/// arbitrary interleave across batches.
async fn send_sorted_chunks(
    chunks: Vec<ChunksWithSegment>,
    global_segment_order: &[SegmentId],
    output_channel: &Sender<ApiResult<CpuWorkerMsg>>,
) -> bool {
    let sorted = {
        let _span = tracing::info_span!("sort_chunks").entered();
        sort_chunks_by_segment_order(chunks, global_segment_order)
    };
    let n_sorted = sorted.len();
    async {
        for chunk in sorted {
            if output_channel
                .send(Ok(CpuWorkerMsg::Chunks(chunk)))
                .await
                .is_err()
            {
                return false;
            }
        }
        true
    }
    .instrument(tracing::info_span!("send_chunks", n = n_sorted))
    .await
}

/// Group gRPC fetch batches without ever exceeding the segment-count gate.
///
/// `create_request_batches` guarantees each individual batch contains at most
/// `MAX_CONCURRENT_SEGMENTS` distinct segments, but the gRPC path reserves budget
/// once for a group of batches. Preserve the same invariant at the group level so
/// the reservation can always be admitted by `PipelineBudget` once enough prior
/// segments finalize.
fn create_grpc_batch_groups(batches: &[RecordBatch]) -> ApiResult<Vec<&[RecordBatch]>> {
    let mut groups = Vec::new();
    let mut group_start = 0usize;
    let mut group_segments: HashSet<String> = HashSet::new();

    for (idx, batch) in batches.iter().enumerate() {
        let mut batch_seen = HashSet::new();
        let mut batch_segments = Vec::new();
        extend_distinct_segment_ids(batch, &mut batch_seen, &mut batch_segments)?;

        if batch_segments.len() > MAX_CONCURRENT_SEGMENTS {
            return Err(ApiError::internal(format!(
                "single gRPC fetch batch spans {} distinct segments, exceeding the cap of {}",
                batch_segments.len(),
                MAX_CONCURRENT_SEGMENTS,
            )));
        }

        let would_exceed_batch_count = idx - group_start >= GRPC_BATCH_SIZE;
        let new_segments = batch_segments
            .iter()
            .filter(|segment_id| !group_segments.contains(*segment_id))
            .count();
        let would_exceed_segments = group_segments.len() + new_segments > MAX_CONCURRENT_SEGMENTS;

        if idx > group_start && (would_exceed_batch_count || would_exceed_segments) {
            groups.push(&batches[group_start..idx]);
            group_start = idx;
            group_segments.clear();
        }

        group_segments.extend(batch_segments);
    }

    if group_start < batches.len() {
        groups.push(&batches[group_start..]);
    }

    Ok(groups)
}

fn distinct_segment_ids_for_batch(batch: &RecordBatch) -> ApiResult<Vec<String>> {
    distinct_segment_ids_for_batches(std::iter::once(batch))
}

fn distinct_segment_ids_for_batches<'a>(
    batches: impl IntoIterator<Item = &'a RecordBatch>,
) -> ApiResult<Vec<String>> {
    let mut seen = HashSet::new();
    let mut segment_ids = Vec::new();
    for batch in batches {
        extend_distinct_segment_ids(batch, &mut seen, &mut segment_ids)?;
    }
    Ok(segment_ids)
}

fn segment_wave_index(
    segment_id: &str,
    segment_to_wave: &HashMap<String, usize>,
) -> ApiResult<usize> {
    segment_to_wave.get(segment_id).copied().ok_or_else(|| {
        ApiError::internal(format!(
            "fetch batch references segment {segment_id} that is not present in global segment order"
        ))
    })
}

fn batches_by_segment_wave(
    batches: &[RecordBatch],
    global_segment_order: &[SegmentId],
) -> ApiResult<(Vec<Vec<RecordBatch>>, usize, usize)> {
    let n_waves = global_segment_order.len().div_ceil(MAX_CONCURRENT_SEGMENTS);
    let mut segment_to_wave = HashMap::new();
    for (idx, segment_id) in global_segment_order.iter().enumerate() {
        segment_to_wave.insert(
            segment_id.as_ref().to_owned(),
            idx / MAX_CONCURRENT_SEGMENTS,
        );
    }

    let mut waves = vec![Vec::new(); n_waves];
    let mut wave_segment_ids = vec![HashSet::new(); n_waves];
    let mut max_segments_per_batch = 0;
    for batch in batches {
        let segment_ids = distinct_segment_ids_for_batch(batch)?;
        let Some(first_segment) = segment_ids.first() else {
            continue;
        };
        max_segments_per_batch = max_segments_per_batch.max(segment_ids.len());
        let wave_idx = segment_wave_index(first_segment, &segment_to_wave)?;
        for segment_id in &segment_ids[1..] {
            let other_wave_idx = segment_wave_index(segment_id, &segment_to_wave)?;
            if other_wave_idx != wave_idx {
                return Err(ApiError::internal(format!(
                    "fetch batch spans segment waves: first segment {first_segment} is in wave \
                     {wave_idx}, but segment {segment_id} is in wave {other_wave_idx}"
                )));
            }
        }
        wave_segment_ids[wave_idx].extend(segment_ids);
        waves[wave_idx].push(batch.clone());
    }
    let max_segments_per_wave = wave_segment_ids
        .into_iter()
        .map(|segment_ids| segment_ids.len())
        .max()
        .unwrap_or(0);

    Ok((waves, max_segments_per_batch, max_segments_per_wave))
}

async fn admit_segment_wave(
    wave_segments: &[SegmentId],
    wave_batches: &[RecordBatch],
    segment_chunk_counts: &HashMap<SegmentId, usize>,
    manifests: &mut HashMap<SegmentId, SegmentChunkManifest>,
    output_channel: &Sender<ApiResult<CpuWorkerMsg>>,
    pipeline_budget: &PipelineBudget,
    filtered_index_timeline: Option<&str>,
) -> ApiResult<bool> {
    if wave_segments.is_empty() {
        return Ok(true);
    }

    let task_time_min = wave_batches
        .iter()
        .map(|b| extract_task_time_min(b, filtered_index_timeline))
        .min()
        .unwrap_or(TimeInt::MAX);
    let segment_ids = wave_segments
        .iter()
        .map(|segment_id| segment_id.as_ref().to_owned())
        .collect();

    // Admit the segment wave before it can allocate CPU-worker state or enter
    // the fetch buffer. This keeps the segment-count gate's O(open segments)
    // memory/CPU invariant, while avoiding the deadlock where non-admissible
    // segment waiters occupy all `buffer_unordered` slots needed by already-open
    // segments. The zero-byte guard is intentionally committed only after the
    // CPU metadata is queued; if the channel closes while sending metadata,
    // dropping the guard vacates the segment slots that the CPU worker will never
    // observe.
    let admission_guard = pipeline_budget
        .reserve_guarded_with_priority(0, task_time_min, segment_ids)
        .await;

    for segment_id in wave_segments {
        let count = segment_chunk_counts
            .get(segment_id)
            .copied()
            .ok_or_else(|| {
                ApiError::internal(format!(
                    "missing chunk count for admitted segment {segment_id}"
                ))
            })?;
        if output_channel
            .send(Ok(CpuWorkerMsg::SegmentChunkCount {
                segment_id: segment_id.clone(),
                count,
            }))
            .await
            .is_err()
        {
            return Ok(false);
        }

        if let Some(manifest) = manifests.remove(segment_id)
            && output_channel
                .send(Ok(CpuWorkerMsg::SegmentManifest {
                    segment_id: segment_id.clone(),
                    manifest: Box::new(manifest),
                }))
                .await
                .is_err()
        {
            return Ok(false);
        }
    }

    admission_guard.commit(0);
    Ok(true)
}

/// Fetch remaining batches via batched gRPC (groups of `GRPC_BATCH_SIZE`),
/// preserving ordering.
async fn fetch_remaining_via_grpc<T: DataframeClientAPI>(
    batches: &[RecordBatch],
    client: &T,
    global_segment_order: &[SegmentId],
    filtered_index_timeline: Option<&str>,
    output_channel: &Sender<ApiResult<CpuWorkerMsg>>,
    pipeline_budget: &PipelineBudget,
    metrics: &QueryMetrics,
) -> ApiResult<()> {
    let total_batches = batches.len();
    let mut batches_completed = 0usize;
    for batch_group in create_grpc_batch_groups(batches)? {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let bytes: u64 = batch_group.iter().map(batch_byte_size).sum();
            crate::chunk_fetcher::metrics::record_grpc_no_direct_urls(bytes);
        }

        let estimated = batch_group
            .iter()
            .map(|b| batch_byte_size_uncompressed(b).unwrap_or_else(|| batch_byte_size(b)))
            .sum::<u64>() as usize;
        let task_time_min = batch_group
            .iter()
            .map(|b| extract_task_time_min(b, filtered_index_timeline))
            .min()
            .unwrap_or(TimeInt::MAX);
        let mut segment_ids: Vec<String> = Vec::new();
        {
            let mut seen: HashSet<String> = HashSet::new();
            for b in batch_group {
                extend_distinct_segment_ids(b, &mut seen, &mut segment_ids)?;
            }
        }
        re_log::debug_assert!(
            segment_ids.len() <= MAX_CONCURRENT_SEGMENTS,
            "gRPC batch group exceeded segment gate cap"
        );
        let guard = pipeline_budget
            .reserve_guarded_with_priority(estimated, task_time_min, segment_ids)
            .await;
        // `?` here is safe: `guard` returns the reservation on drop —
        // both the reserved bytes and the segment-count gate slots — so
        // an error from the gRPC fetch does not leak headroom to the
        // shared cross-partition budget.
        let mut stats = TaskFetchStats::default();
        let all_chunks = fetch_batch_group_via_grpc(
            batch_group,
            client,
            &metrics.fetch_grpc_requests,
            &mut stats,
        )
        .await?;

        let actual: usize = all_chunks
            .iter()
            .flat_map(|segment_chunks| {
                segment_chunks
                    .iter()
                    .map(|(chunk, _)| re_byte_size::SizeBytes::total_size_bytes(chunk) as usize)
            })
            .sum();
        guard.commit(actual);

        // Flush bytes before handing chunks downstream so a LIMIT-triggered
        // snapshot cannot observe delivered rows without their fetch metrics.
        // Issued-request counts are updated at the transport boundary and are
        // therefore already visible even if this future is cancelled.
        stats.flush_into(metrics);

        batches_completed += batch_group.len();
        if !send_sorted_chunks(all_chunks, global_segment_order, output_channel).await {
            // Consumer (CPU worker) hung up — typically because a row LIMIT was hit
            // or the surrounding plan was cancelled. Any remaining batches will go
            // unfetched. Log at `info` (not `debug`) so the cancellation is visible
            // in production logs alongside the matching server-side
            // `terminated_by="cancelled"` analytics; not a `warn` because this is
            // expected when the caller actually wanted to stop.
            tracing::info!(
                total_batches,
                batches_completed,
                batches_skipped = total_batches.saturating_sub(batches_completed),
                "FetchChunks IO loop short-circuited: downstream consumer closed (likely LIMIT or plan cancellation)"
            );
            return Ok(());
        }
    }
    Ok(())
}

/// This is the function that will run on the IO (main) tokio runtime that will listen
/// to the gRPC channel for chunks coming in from the catalog server. This loop is started
/// up by the execute fn of the physical plan, so we will start one per output DataFusion partition,
/// which is different from the Rerun `segment_id`. The sorting by time index will happen within
/// the cpu worker thread.
///
/// `chunk_infos` is a list of batches with chunk information where each batch has info for
/// a *single segment*. We also expect these to be previously sorted by segment id, otherwise
/// our suggestion to the query planner that inputs are sorted by segment id will be incorrect.
/// See `group_chunk_infos_by_segment_id` and `execute` for more details.
///
/// In order to improve performance, while maintaining ordering, we batch requests to the server
/// and process them concurrently in groups. After data for each group is collected, it is sorted
/// by the input segment order before being sent to the CPU worker thread.
#[tracing::instrument(
    level = "info",
    skip_all,
    fields(
        n_chunks,
        n_batches,
        n_segments,
        n_waves,
        segment_admission_limit,
        max_segments_per_batch,
        max_segments_per_wave,
        fetch_strategy,
    )
)]
pub(super) async fn chunk_stream_io_loop<T: DataframeClientAPI>(
    client: T,
    chunk_infos: Vec<RecordBatch>,
    filtered_index_timeline: Option<TimelineName>,
    output_channel: Sender<ApiResult<CpuWorkerMsg>>,
    pending_analytics: crate::PendingQueryAnalytics,
    pipeline_budget: Arc<PipelineBudget>,
) -> ApiResult<()> {
    let target_size_bytes = TARGET_BATCH_SIZE_BYTES as u64;
    let metrics = Arc::clone(pending_analytics.metrics());

    // One row per chunk in each `RecordBatch` of chunk-info.
    let n_chunks: usize = chunk_infos.iter().map(|rb| rb.num_rows()).sum();

    let segment_chunk_counts: HashMap<SegmentId, usize> = count_chunks_per_segment(&chunk_infos)?
        .into_iter()
        .collect();

    // When a temporal `filtered_index` is set, build per-segment manifests from the
    // `{timeline}:start` columns. Metadata is sent to the CPU worker wave-by-wave,
    // only after the corresponding segment IDs have been admitted by the budget's
    // segment-count gate; sending it all up front would let the CPU worker allocate
    // per-segment state for hundreds of not-yet-admissible segments.
    let filtered_index_timeline_str: Option<Arc<str>> = filtered_index_timeline
        .as_ref()
        .map(|t| Arc::<str>::from(t.as_str()));
    let mut manifests = build_segment_manifests(&chunk_infos, filtered_index_timeline)?;

    let (request_batches, global_segment_order) =
        create_request_batches(chunk_infos, target_size_bytes)?;
    let (request_batches_by_wave, max_segments_per_batch, max_segments_per_wave) =
        batches_by_segment_wave(&request_batches, &global_segment_order)?;

    metrics
        .planned_fetch_batches
        .fetch_add(request_batches.len() as u64, Ordering::Relaxed);
    metrics
        .planned_segment_waves
        .fetch_add(request_batches_by_wave.len() as u64, Ordering::Relaxed);
    metrics
        .segment_admission_limit
        .fetch_max(MAX_CONCURRENT_SEGMENTS as u64, Ordering::Relaxed);
    metrics
        .max_segments_per_fetch_batch
        .fetch_max(max_segments_per_batch as u64, Ordering::Relaxed);
    metrics
        .max_segments_per_wave
        .fetch_max(max_segments_per_wave as u64, Ordering::Relaxed);

    let span = tracing::Span::current();
    span.record("n_chunks", n_chunks);
    span.record("n_batches", request_batches.len());
    span.record("n_segments", global_segment_order.len());
    span.record("n_waves", request_batches_by_wave.len());
    span.record("segment_admission_limit", MAX_CONCURRENT_SEGMENTS);
    span.record("max_segments_per_batch", max_segments_per_batch);
    span.record("max_segments_per_wave", max_segments_per_wave);

    re_log::debug!(
        "Fetching {n_chunks} chunks in {} batches ({} segments)",
        request_batches.len(),
        global_segment_order.len()
    );

    // Allow overriding the fetch strategy via environment variable.
    let force_grpc = force_grpc();

    // Process segment waves sequentially. Within a wave, fetch concurrency is still
    // high and byte-bounded by PipelineBudget. Across waves, segment admission happens
    // outside the fetch buffer so non-admissible future segments cannot occupy every
    // `buffer_unordered` slot and starve the fetches needed to complete the currently
    // admitted segments.
    let grpc_only = force_grpc || !request_batches.iter().any(batch_has_any_direct_urls);
    if grpc_only {
        let reason = if force_grpc {
            "grpc_forced"
        } else {
            "no_direct_urls"
        };
        span.record("fetch_strategy", reason);
        re_log::debug!(
            "{reason}, fetching all {} chunks via FetchChunks gRPC in segment waves",
            request_batches.len()
        );

        let result = async {
            for (wave_idx, wave_batches) in request_batches_by_wave.iter().enumerate() {
                let start = wave_idx * MAX_CONCURRENT_SEGMENTS;
                let end = (start + MAX_CONCURRENT_SEGMENTS).min(global_segment_order.len());
                let wave_segments = &global_segment_order[start..end];
                if !admit_segment_wave(
                    wave_segments,
                    wave_batches,
                    &segment_chunk_counts,
                    &mut manifests,
                    &output_channel,
                    &pipeline_budget,
                    filtered_index_timeline_str.as_deref(),
                )
                .await?
                {
                    return Ok(());
                }

                fetch_remaining_via_grpc(
                    wave_batches,
                    &client,
                    &global_segment_order,
                    filtered_index_timeline_str.as_deref(),
                    &output_channel,
                    &pipeline_budget,
                    &metrics,
                )
                .await?;
            }
            Ok(())
        }
        .await;

        // Fetch stats are flushed per batch group inside
        // `fetch_remaining_via_grpc`, before the corresponding chunks are
        // handed downstream. Only the error needs recording here.
        if result.is_err() {
            pending_analytics.record_error(QueryErrorKind::GrpcFetch);
        }

        return result;
    }

    let total_n_direct: usize = request_batches
        .iter()
        .filter_map(|batch| split_batch_by_direct_url(batch).0)
        .count();
    let total_n_grpc: usize = request_batches
        .iter()
        .filter_map(|batch| split_batch_by_direct_url(batch).1)
        .count();
    if total_n_grpc == 0 {
        span.record("fetch_strategy", "direct");
    } else {
        span.record(
            "fetch_strategy",
            format!("hybrid(direct={total_n_direct},grpc={total_n_grpc})"),
        );
    }
    re_log::debug!("Fetch tasks: {total_n_direct} direct, {total_n_grpc} gRPC fallback");

    let http_client = reqwest::Client::builder()
        .connect_timeout(DIRECT_FETCH_CONNECT_TIMEOUT)
        .read_timeout(DIRECT_FETCH_READ_TIMEOUT)
        .build()
        .expect("static reqwest client config is valid");
    let total_tasks = total_n_direct + total_n_grpc;
    let mut tasks_completed: usize = 0;

    for (wave_idx, wave_batches) in request_batches_by_wave.into_iter().enumerate() {
        let start = wave_idx * MAX_CONCURRENT_SEGMENTS;
        let end = (start + MAX_CONCURRENT_SEGMENTS).min(global_segment_order.len());
        let wave_segments = &global_segment_order[start..end];
        if !admit_segment_wave(
            wave_segments,
            &wave_batches,
            &segment_chunk_counts,
            &mut manifests,
            &output_channel,
            &pipeline_budget,
            filtered_index_timeline_str.as_deref(),
        )
        .await?
        {
            return Ok(());
        }

        let (work_items, _n_direct, _n_grpc) = split_batches_into_fetch_tasks(&wave_batches);
        let fetch_stream = futures::stream::iter(work_items.into_iter().enumerate())
            .map(|(task_idx, task)| {
                let http_client = http_client.clone();
                let client = client.clone();
                let pending_analytics = pending_analytics.clone();
                let pipeline_budget = Arc::clone(&pipeline_budget);
                let metrics = Arc::clone(&metrics);
                let filtered_index_timeline = filtered_index_timeline_str.as_ref().map(Arc::clone);
                async move {
                    // Task-local stats buffer — flushed once to the shared atomics
                    // at the end of this task to avoid cross-core cache-line
                    // contention on the hot counters.
                    let mut stats = TaskFetchStats::default();

                    let batch_ref = match &task {
                        FetchTask::Direct(b) | FetchTask::Grpc(b) => b,
                    };
                    let estimated = batch_byte_size_uncompressed(batch_ref)
                        .unwrap_or_else(|| batch_byte_size(batch_ref))
                        as usize;
                    let task_time_min =
                        extract_task_time_min(batch_ref, filtered_index_timeline.as_deref());
                    let mut seen: HashSet<String> = HashSet::new();
                    let mut segment_ids: Vec<String> = Vec::new();
                    extend_distinct_segment_ids(batch_ref, &mut seen, &mut segment_ids)?;
                    // RAII guard: refunds the byte reservation on any early
                    // `return Err(_)`. Segment slots are already admitted by the
                    // wave guard above; since these are existing active segments,
                    // committing keeps them active until the CPU worker finalizes
                    // each segment.
                    let guard = pipeline_budget
                        .reserve_guarded_with_priority(estimated, task_time_min, segment_ids)
                        .await;

                    let chunks = match task {
                        FetchTask::Direct(batch) => {
                            match fetch_batch_direct(
                                &batch,
                                &http_client,
                                &metrics.fetch_direct_requests,
                                &mut stats,
                                &pending_analytics,
                            )
                            .await
                            {
                                Ok(chunks) => chunks,
                                Err(err) => {
                                    stats.try_flush_into(
                                        &pending_analytics,
                                        Err(QueryErrorKind::DirectFetch),
                                    );
                                    return Err(err);
                                }
                            }
                        }
                        FetchTask::Grpc(batch) => {
                            let bytes = batch_byte_size(&batch);
                            #[cfg(not(target_arch = "wasm32"))]
                            crate::chunk_fetcher::metrics::record_grpc_no_direct_urls(bytes);
                            match fetch_batch_group_via_grpc(
                                std::slice::from_ref(&batch),
                                &client,
                                &metrics.fetch_grpc_requests,
                                &mut stats,
                            )
                            .await
                            {
                                Ok(chunks) => chunks,
                                Err(err) => {
                                    stats.try_flush_into(
                                        &pending_analytics,
                                        Err(QueryErrorKind::GrpcFetch),
                                    );
                                    return Err(err);
                                }
                            }
                        }
                    };

                    stats.try_flush_into(&pending_analytics, Ok(()));
                    let actual: usize = chunks
                        .iter()
                        .flat_map(|seg| {
                            seg.iter()
                                .map(|(c, _)| re_byte_size::SizeBytes::total_size_bytes(c) as usize)
                        })
                        .sum();
                    guard.commit(actual);

                    Ok::<_, ApiError>(chunks)
                }
                .instrument(tracing::info_span!("fetch_task", wave_idx, task_idx))
            })
            .buffer_unordered(IO_PIPELINE_BUFFER);

        tokio::pin!(fetch_stream);

        while let Some(result) = fetch_stream.next().await {
            let chunks = result?;
            tasks_completed += 1;
            if !send_sorted_chunks(chunks, &global_segment_order, &output_channel).await {
                // Consumer hung up — log task progress so cancel timing is
                // visible. `buffer_unordered` will RST_STREAM any in-flight
                // fetches when its driver task is dropped.
                tracing::info!(
                    total_tasks,
                    tasks_completed,
                    in_flight_or_pending = total_tasks.saturating_sub(tasks_completed),
                    "FetchChunks IO loop short-circuited (hybrid path): downstream consumer closed (likely LIMIT or plan cancellation)"
                );
                return Ok(());
            }
        }
    }

    // Fetch stats are already recorded per-task into pending_analytics.
    // The combined event will be sent when the last PendingQueryAnalytics clone is dropped.

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow::array::{
        Array as _, BooleanArray, FixedSizeBinaryBuilder, Int64Array, RecordBatchOptions,
        StringArray, UInt64Array,
    };
    use arrow::datatypes::{Field, Schema};
    use re_log_types::EntityPath;

    use super::*;

    /// Extract segment ID from a chunk result (test helper)
    fn extract_segment_id_from_chunk((segment_id, _chunks): &SortedChunksWithSegment) -> &str {
        segment_id.as_ref()
    }

    /// Helper to create a test `RecordBatch` with chunk info for testing
    fn create_test_chunk_info(segment_id: &str, chunk_sizes: &[u64]) -> RecordBatch {
        let num_chunks = chunk_sizes.len();

        // Create segment ID column (all rows have same segment)
        let segment_ids = StringArray::from(vec![segment_id; num_chunks]);

        // Create chunk sizes column
        let sizes = UInt64Array::from(chunk_sizes.to_vec());

        // Create dummy chunk IDs
        let mut chunk_id_builder = FixedSizeBinaryBuilder::with_capacity(num_chunks, 16);
        for i in 0..num_chunks {
            let mut id_bytes = [0u8; 16];
            id_bytes[0..4].copy_from_slice(&(i as u32).to_le_bytes());
            chunk_id_builder.append_value(id_bytes).unwrap();
        }
        let chunk_ids = chunk_id_builder.finish();

        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.arrow_field(),
                Field::new(
                    QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN_NAME,
                    arrow::datatypes::DataType::UInt64,
                    false,
                ),
                QueryDatasetDataframe::COLUMN_CHUNK_ID.arrow_field(),
            ],
            HashMap::default(),
        ));

        RecordBatch::try_new_with_options(
            schema,
            vec![Arc::new(segment_ids), Arc::new(sizes), Arc::new(chunk_ids)],
            &RecordBatchOptions::new().with_row_count(Some(num_chunks)),
        )
        .unwrap()
    }

    fn segment_order_as_strs(segment_order: &[SegmentId]) -> Vec<&str> {
        segment_order.iter().map(SegmentId::as_ref).collect()
    }

    #[test]
    fn test_create_request_batches_single_small_segment() {
        let chunk_info = create_test_chunk_info("seg1", &[100, 200, 300]); // 600 bytes total
        let target_size = 1000; // 1KB target

        let (batches, segment_order) =
            create_request_batches(vec![chunk_info], target_size).unwrap();

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].num_rows(), 3);
        assert_eq!(segment_order_as_strs(&segment_order), vec!["seg1"]);
    }

    #[test]
    fn test_create_request_batches_single_large_segment() {
        let chunk_info = create_test_chunk_info("seg1", &[300, 400, 500, 600]); // 1800 bytes total
        let target_size = 1000; // 1KB target

        let (batches, segment_order) =
            create_request_batches(vec![chunk_info], target_size).unwrap();

        // should be split into 3 as each batch should be under 1KB
        assert_eq!(batches.len(), 3);
        assert_eq!(segment_order_as_strs(&segment_order), vec!["seg1"]);
    }

    #[test]
    fn test_create_request_batches_multiple_small_segments() {
        let chunk_infos = vec![
            create_test_chunk_info("seg1", &[100, 150]), // 250 bytes
            create_test_chunk_info("seg2", &[200, 250]), // 450 bytes
            create_test_chunk_info("seg3", &[300]),      // 300 bytes
            create_test_chunk_info("seg4", &[100]),      // 100 bytes
        ];
        let target_size = 800; // Should fit seg1+seg2 in first batch, seg3+seg4 in second

        let (batches, segment_order) = create_request_batches(chunk_infos, target_size).unwrap();

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].num_rows(), 4);
        assert_eq!(batches[1].num_rows(), 2);
        assert_eq!(
            segment_order_as_strs(&segment_order),
            vec!["seg1", "seg2", "seg3", "seg4"]
        );
    }

    /// Many tiny segments must not all merge into a single fetch
    /// batch: the merged batch's distinct-segment count would exceed
    /// `MAX_CONCURRENT_SEGMENTS` and the segment-count gate in
    /// `PipelineBudget::try_admit` would deadlock on it. Verify the
    /// merger flushes the batch at the cap regardless of byte
    /// headroom.
    #[test]
    fn test_create_request_batches_caps_segments_per_batch() {
        // Six tiny segments, target size large enough that all six
        // would otherwise pack into one batch.
        let chunk_infos = vec![
            create_test_chunk_info("seg1", &[10]),
            create_test_chunk_info("seg2", &[10]),
            create_test_chunk_info("seg3", &[10]),
            create_test_chunk_info("seg4", &[10]),
            create_test_chunk_info("seg5", &[10]),
            create_test_chunk_info("seg6", &[10]),
        ];
        let target_size = 100_000; // far more than 6 * 10

        let (batches, segment_order) = create_request_batches(chunk_infos, target_size).unwrap();

        // 6 segments / MAX_CONCURRENT_SEGMENTS=3 = exactly 2 batches.
        assert_eq!(batches.len(), 2);
        // Each batch's distinct-segment count must not exceed the cap.
        for batch in &batches {
            let seg_col = batch
                .column_by_name(QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID_NAME)
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let distinct: HashSet<&str> = (0..seg_col.len()).map(|i| seg_col.value(i)).collect();
            assert!(
                distinct.len() <= MAX_CONCURRENT_SEGMENTS,
                "batch has {} distinct segments, cap is {}",
                distinct.len(),
                MAX_CONCURRENT_SEGMENTS,
            );
        }
        assert_eq!(
            segment_order_as_strs(&segment_order),
            vec!["seg1", "seg2", "seg3", "seg4", "seg5", "seg6"]
        );
    }

    #[test]
    fn test_many_tiny_segments_expose_serial_wave_shape() {
        const NUM_SEGMENTS: usize = 3_990;
        let chunk_infos = (0..NUM_SEGMENTS)
            .map(|idx| create_test_chunk_info(&format!("seg{idx:04}"), &[10]))
            .collect();

        let (batches, segment_order) =
            create_request_batches(chunk_infos, TARGET_BATCH_SIZE_BYTES as u64).unwrap();
        let (waves, max_segments_per_batch, max_segments_per_wave) =
            batches_by_segment_wave(&batches, &segment_order).unwrap();

        let expected = NUM_SEGMENTS.div_ceil(MAX_CONCURRENT_SEGMENTS);
        assert_eq!(segment_order.len(), NUM_SEGMENTS);
        assert_eq!(batches.len(), expected);
        assert_eq!(waves.len(), expected);
        assert_eq!(max_segments_per_batch, MAX_CONCURRENT_SEGMENTS);
        assert_eq!(max_segments_per_wave, MAX_CONCURRENT_SEGMENTS);
        assert!(waves.iter().all(|wave| wave.len() == 1));
        assert!(
            batches
                .iter()
                .all(|batch| batch.num_rows() == MAX_CONCURRENT_SEGMENTS)
        );
    }

    fn distinct_segments_in_batches<'a>(
        batches: impl IntoIterator<Item = &'a RecordBatch>,
    ) -> HashSet<String> {
        let mut seen = HashSet::new();
        let mut order = Vec::new();
        for batch in batches {
            extend_distinct_segment_ids(batch, &mut seen, &mut order).unwrap();
        }
        seen
    }

    #[test]
    fn test_batches_by_segment_wave_keeps_each_wave_within_segment_cap() {
        let chunk_infos = vec![
            create_test_chunk_info("seg1", &[100]),
            create_test_chunk_info("seg2", &[100]),
            create_test_chunk_info("seg3", &[100]),
            create_test_chunk_info("seg4", &[100]),
            create_test_chunk_info("seg5", &[100]),
        ];
        let (batches, segment_order) = create_request_batches(chunk_infos, 10_000).unwrap();

        let (waves, _, _) = batches_by_segment_wave(&batches, &segment_order).unwrap();

        assert_eq!(waves.len(), 2);
        assert_eq!(waves[0].len(), 1);
        assert_eq!(waves[1].len(), 1);
        assert_eq!(
            distinct_segments_in_batches(&waves[0]),
            HashSet::from_iter(["seg1".to_owned(), "seg2".to_owned(), "seg3".to_owned()])
        );
        assert_eq!(
            distinct_segments_in_batches(&waves[1]),
            HashSet::from_iter(["seg4".to_owned(), "seg5".to_owned()])
        );
    }

    #[test]
    fn test_create_grpc_batch_groups_preserves_segment_cap() {
        let chunk_infos = vec![
            create_test_chunk_info("seg1", &[10]),
            create_test_chunk_info("seg2", &[10]),
            create_test_chunk_info("seg3", &[10]),
            create_test_chunk_info("seg4", &[10]),
            create_test_chunk_info("seg5", &[10]),
            create_test_chunk_info("seg6", &[10]),
        ];
        let (batches, _) = create_request_batches(chunk_infos, 100_000).unwrap();
        assert_eq!(batches.len(), 2);

        let groups = create_grpc_batch_groups(&batches).unwrap();

        assert_eq!(groups.len(), 2);
        for group in groups {
            assert!(group.len() <= GRPC_BATCH_SIZE);
            assert!(distinct_segments_in_batches(group.iter()).len() <= MAX_CONCURRENT_SEGMENTS);
        }
    }

    #[test]
    fn test_create_grpc_batch_groups_keeps_same_segment_batches_together() {
        let chunk_info = create_test_chunk_info("seg1", &[10; GRPC_BATCH_SIZE + 1]);
        let (batches, _) = create_request_batches(vec![chunk_info], 10).unwrap();
        assert_eq!(batches.len(), GRPC_BATCH_SIZE + 1);

        let groups = create_grpc_batch_groups(&batches).unwrap();

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), GRPC_BATCH_SIZE);
        assert_eq!(groups[1].len(), 1);
        assert_eq!(distinct_segments_in_batches(groups[0].iter()).len(), 1);
        assert_eq!(distinct_segments_in_batches(groups[1].iter()).len(), 1);
    }

    #[test]
    fn test_create_request_batches_mixed_small_and_large() {
        let chunk_infos = vec![
            create_test_chunk_info("seg1", &[100, 200]), // 300 bytes - small
            create_test_chunk_info("seg2", &[800, 900, 700]), // 2400 bytes - large, needs splitting
            create_test_chunk_info("seg3", &[150]),      // 150 bytes - small
        ];
        let target_size = 1000;

        let (batches, segment_order) = create_request_batches(chunk_infos, target_size).unwrap();

        // Should have: [seg1], [seg2_part1], [seg2_part2], [seg2_part3], [seg3]
        assert_eq!(batches.len(), 5);
        assert_eq!(
            segment_order_as_strs(&segment_order),
            vec!["seg1", "seg2", "seg3"]
        );
    }

    #[test]
    fn test_skewed_segment_sizes_preserve_rows_order_and_caps() {
        let mut chunk_infos: Vec<_> = (0..30)
            .map(|idx| create_test_chunk_info(&format!("tiny{idx:02}"), &[1]))
            .collect();
        chunk_infos.push(create_test_chunk_info("large", &[600, 600, 600]));

        let expected_rows: usize = chunk_infos.iter().map(RecordBatch::num_rows).sum();
        let (batches, segment_order) = create_request_batches(chunk_infos, 1_000).unwrap();
        let (waves, max_segments_per_batch, max_segments_per_wave) =
            batches_by_segment_wave(&batches, &segment_order).unwrap();

        assert_eq!(segment_order.len(), 31);
        assert_eq!(segment_order.last().unwrap().as_ref(), "large");
        assert_eq!(batches.len(), 13);
        assert_eq!(waves.len(), 11);
        assert_eq!(max_segments_per_batch, MAX_CONCURRENT_SEGMENTS);
        assert_eq!(max_segments_per_wave, MAX_CONCURRENT_SEGMENTS);
        assert_eq!(
            batches.iter().map(RecordBatch::num_rows).sum::<usize>(),
            expected_rows
        );
        assert!(batches.iter().all(|batch| {
            distinct_segment_ids_for_batch(batch).unwrap().len() <= MAX_CONCURRENT_SEGMENTS
        }));
        assert!(waves.iter().all(|wave| {
            distinct_segments_in_batches(wave.iter()).len() <= MAX_CONCURRENT_SEGMENTS
        }));
    }

    #[test]
    fn test_segment_order_within_batches_is_preserved() {
        let chunk_infos = vec![
            create_test_chunk_info("segA", &[100]), // First in input
            create_test_chunk_info("segB", &[200]), // Second in input
            create_test_chunk_info("segC", &[300]), // Third in input
        ];
        let target_size = 1000; // All segments fit in one batch

        let (batches, segment_order) = create_request_batches(chunk_infos, target_size).unwrap();

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].num_rows(), 3);
        assert_eq!(
            segment_order_as_strs(&segment_order),
            vec!["segA", "segB", "segC"]
        );

        // Verify that segments within the batch maintain input order
        let segment_id_column = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID
            .extract(&batches[0])
            .unwrap();

        let batch_segment_ids: Vec<&str> = segment_id_column.iter().collect();
        assert_eq!(batch_segment_ids, ["segA", "segB", "segC"]);
    }

    #[test]
    fn test_sort_chunks_by_segment_order_simple_case() {
        use re_dataframe::external::re_chunk::Chunk;
        use re_log_types::EntityPath;

        // Simple case: one segment per response
        let empty_chunk = Chunk::builder(EntityPath::root()).build().unwrap();
        let segment_order: Vec<SegmentId> = vec!["segA".into(), "segB".into(), "segC".into()];

        let chunks: Vec<ChunksWithSegment> = vec![
            vec![(empty_chunk.clone(), Some("segC".into()))],
            vec![(empty_chunk.clone(), Some("segA".into()))],
            vec![(empty_chunk.clone(), Some("segB".into()))],
        ];

        let sorted_chunks = sort_chunks_by_segment_order(chunks, &segment_order);

        // Verify chunks are sorted according to segment order
        let sorted_segments: Vec<&str> = sorted_chunks
            .iter()
            .map(extract_segment_id_from_chunk)
            .collect();

        assert_eq!(sorted_segments, vec!["segA", "segB", "segC"]);
    }

    #[test]
    fn test_sort_chunks_by_segment_order_multi_segment_response() {
        use re_dataframe::external::re_chunk::Chunk;
        use re_log_types::EntityPath;

        let empty_chunk = Chunk::builder(EntityPath::root()).build().unwrap();
        let segment_order: Vec<SegmentId> = vec!["segA".into(), "segB".into(), "segC".into()];

        let chunks: Vec<ChunksWithSegment> = vec![
            // Single response containing segments in wrong order: segC, segA, segB
            vec![
                (empty_chunk.clone(), Some("segC".into())),
                (empty_chunk.clone(), Some("segC".into())), // Multiple chunks for segC
                (empty_chunk.clone(), Some("segA".into())),
                (empty_chunk.clone(), Some("segB".into())),
                (empty_chunk.clone(), Some("segB".into())), // Multiple chunks for segB
                (empty_chunk.clone(), Some("segA".into())), // More chunks for segA
                (empty_chunk.clone(), Some("segB".into())), // More chunks for segB
            ],
        ];

        let sorted_chunks = sort_chunks_by_segment_order(chunks, &segment_order);

        // After sorting, we should have segments in correct order: segA, segB, segC
        // And the function should have split the multi-segment response into separate responses
        assert_eq!(sorted_chunks.len(), 3);
        let sorted_segments: Vec<&str> = sorted_chunks
            .iter()
            .map(extract_segment_id_from_chunk)
            .collect();

        assert_eq!(sorted_segments, vec!["segA", "segB", "segC"]);

        // Verify each segment has the correct number of chunks
        let seg_a_chunks = sorted_chunks[0].1.len();
        let seg_b_chunks = sorted_chunks[1].1.len();
        let seg_c_chunks = sorted_chunks[2].1.len();

        assert_eq!(seg_a_chunks, 2);
        assert_eq!(seg_b_chunks, 3);
        assert_eq!(seg_c_chunks, 2);
    }

    #[test]
    fn test_sort_chunks_by_segment_order_mixed_responses() {
        use re_dataframe::external::re_chunk::Chunk;
        use re_log_types::EntityPath;

        // We have some single-segment responses, some multi-segment responses
        let empty_chunk = Chunk::builder(EntityPath::root()).build().unwrap();
        let segment_order: Vec<SegmentId> = vec!["segA".into(), "segB".into(), "segC".into()];

        let chunks: Vec<ChunksWithSegment> = vec![
            // Single segment response
            vec![(empty_chunk.clone(), Some("segC".into()))],
            // Multi-segment response
            vec![
                (empty_chunk.clone(), Some("segB".into())),
                (empty_chunk.clone(), Some("segA".into())),
            ],
            // Another single segment response
            vec![(empty_chunk.clone(), Some("segB".into()))],
        ];

        let sorted_chunks = sort_chunks_by_segment_order(chunks, &segment_order);

        // Should be sorted: segA, segB (grouped together), segC
        assert_eq!(sorted_chunks.len(), 3);

        let sorted_segments: Vec<&str> = sorted_chunks
            .iter()
            .map(extract_segment_id_from_chunk)
            .collect();

        assert_eq!(sorted_segments, vec!["segA", "segB", "segC"]);

        // Verify segB has 2 chunks (they should be grouped together)
        let seg_b_chunks = sorted_chunks[1].1.len();
        assert_eq!(seg_b_chunks, 2);
    }

    /// `count_chunks_per_segment` produces one entry per distinct
    /// `segment_id`, preserves first-encounter order, and sums rows
    /// across the input `chunk_info` batches.
    #[test]
    fn test_count_chunks_per_segment_basic() {
        let chunk_infos = vec![
            create_test_chunk_info("segA", &[10, 20, 30]),
            create_test_chunk_info("segB", &[40]),
            create_test_chunk_info("segC", &[50, 60]),
        ];
        let counts = count_chunks_per_segment(&chunk_infos).unwrap();
        assert_eq!(
            counts,
            vec![
                (SegmentId::from("segA"), 3),
                (SegmentId::from("segB"), 1),
                (SegmentId::from("segC"), 2),
            ]
        );
    }

    /// Multiple `chunk_info` batches that target the same segment must
    /// have their row counts summed under a single entry, not produce
    /// two entries with conflicting counts.
    #[test]
    fn test_count_chunks_per_segment_sums_across_batches() {
        let chunk_infos = vec![
            create_test_chunk_info("segA", &[1, 2]),
            create_test_chunk_info("segB", &[3]),
            create_test_chunk_info("segA", &[4, 5, 6]),
        ];
        let counts = count_chunks_per_segment(&chunk_infos).unwrap();
        // First-encounter order: segA before segB. Counts sum: A=5, B=1.
        assert_eq!(
            counts,
            vec![(SegmentId::from("segA"), 5), (SegmentId::from("segB"), 1),]
        );
    }

    // ----------------------------------------------------------------
    // build_segment_manifests tests
    // ----------------------------------------------------------------

    /// One row per `(segment_id, entity_path, is_static, start)` tuple.
    /// `start = None` produces a null entry in the `{timeline}:start`
    /// column.
    fn create_chunk_info_with_starts(
        timeline_name: &str,
        rows: &[(&str, &str, bool, Option<i64>)],
    ) -> RecordBatch {
        let num_rows = rows.len();
        let segment_ids = StringArray::from(rows.iter().map(|r| r.0).collect::<Vec<_>>());
        let entity_paths = StringArray::from(rows.iter().map(|r| r.1).collect::<Vec<_>>());
        let is_static = BooleanArray::from(rows.iter().map(|r| r.2).collect::<Vec<_>>());
        let starts = Int64Array::from(rows.iter().map(|r| r.3).collect::<Vec<_>>());

        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.arrow_field(),
                QueryDatasetDataframe::COLUMN_CHUNK_ENTITY_PATH.arrow_field(),
                QueryDatasetDataframe::COLUMN_CHUNK_IS_STATIC.arrow_field(),
                re_protos::cloud::v1alpha1::QueryDatasetResponse::field_timeline_start(
                    timeline_name,
                )
                .as_ref()
                .clone(),
            ],
            HashMap::default(),
        ));

        RecordBatch::try_new_with_options(
            schema,
            vec![
                Arc::new(segment_ids),
                Arc::new(entity_paths),
                Arc::new(is_static),
                Arc::new(starts),
            ],
            &RecordBatchOptions::new().with_row_count(Some(num_rows)),
        )
        .unwrap()
    }

    /// `filtered_timeline = None` (static-only query) → empty manifest
    /// map, no error. Worker then falls back to completion-only flush.
    #[test]
    fn test_build_segment_manifests_no_timeline_yields_empty() {
        let rb = create_chunk_info_with_starts("time", &[("seg1", "/a", false, Some(10))]);
        let manifests = build_segment_manifests(&[rb], None).unwrap();
        assert!(manifests.is_empty());
    }

    /// `:start` column absent on every batch (old server) → empty map.
    #[test]
    fn test_build_segment_manifests_missing_start_column_yields_empty() {
        // Reuse the `create_test_chunk_info` helper, which produces a
        // batch *without* a `{timeline}:start` column.
        let rb = create_test_chunk_info("seg1", &[100]);
        let manifests = build_segment_manifests(&[rb], Some(TimelineName::from("time"))).unwrap();
        assert!(manifests.is_empty());
    }

    /// Mixed batches: one batch has `:start`, the other doesn't. Building
    /// a partial manifest from only the `:start` batch and locking would
    /// leave segments split across both with under-counted manifests →
    /// `record_arrival` for chunks from the no-`:start` batch would
    /// `debug_panic!` and pin `safe_horizon` at the laggard's `time_min`
    /// forever. The all-or-nothing pre-check must reject the whole input
    /// and yield an empty map.
    #[test]
    fn test_build_segment_manifests_mixed_batches_yields_empty() {
        let with_start = create_chunk_info_with_starts("time", &[("seg1", "/a", false, Some(10))]);
        let without_start = create_test_chunk_info("seg1", &[20]);
        let manifests = build_segment_manifests(
            &[with_start, without_start],
            Some(TimelineName::from("time")),
        )
        .unwrap();
        assert!(
            manifests.is_empty(),
            "any batch missing `:start` must trigger full fallback, even if other batches carry it",
        );
    }

    /// Null `:start`, `is_static = true`, and the `i64::MIN` saturation
    /// edge are all filtered correctly. Only the two ordinary temporal
    /// rows reach the manifest.
    #[test]
    fn test_build_segment_manifests_filters_null_and_static() {
        let rb = create_chunk_info_with_starts(
            "time",
            &[
                ("seg1", "/a", false, None),           // null :start → skip
                ("seg1", "/b", true, Some(10)),        // is_static → skip
                ("seg1", "/c", false, Some(20)),       // keep
                ("seg1", "/d", false, Some(i64::MIN)), // saturates to TimeInt::MIN, kept
            ],
        );
        let manifests = build_segment_manifests(&[rb], Some(TimelineName::from("time"))).unwrap();
        let m = manifests
            .get(&SegmentId::from("seg1"))
            .expect("seg1 has at least one temporal chunk");
        assert_eq!(m.outstanding_count(), 2);
        // Locked by `build_segment_manifests` itself.
        assert!(m.is_locked());
        // Laggard is `/d` at saturated MIN → horizon = MIN (saturating sub).
        assert_eq!(m.safe_horizon(), Some(TimeInt::MIN));
    }

    /// `EntityPath` keys produced by `build_segment_manifests` must
    /// match what `record_arrival` looks up. This is the regression
    /// test for the `String → EntityPath` migration in PR 2 (commit
    /// 28c54f498c): if the ingest path passed strings, this lookup
    /// would silently miss and the worker would log divergence on
    /// every chunk arrival.
    #[test]
    fn test_build_segment_manifests_entity_path_keys_roundtrip() {
        let rb = create_chunk_info_with_starts("time", &[("seg1", "/foo/bar", false, Some(42))]);
        let mut manifests =
            build_segment_manifests(&[rb], Some(TimelineName::from("time"))).unwrap();
        let m = manifests.get_mut(&SegmentId::from("seg1")).unwrap();
        // Looking up via `EntityPath::from(&str)` must hit the entry.
        assert!(m.record_arrival(
            &EntityPath::from("/foo/bar"),
            TimeInt::saturated_temporal_i64(42)
        ));
        assert!(m.is_complete());
    }

    /// A segment that has only static rows on the filtered timeline
    /// produces no manifest entry at all. The worker handles such
    /// segments via the count-based path.
    #[test]
    fn test_build_segment_manifests_all_static_segment_absent() {
        let rb = create_chunk_info_with_starts(
            "time",
            &[
                ("segA", "/a", true, Some(10)),
                ("segA", "/b", true, Some(20)),
                ("segB", "/c", false, Some(30)),
            ],
        );
        let manifests = build_segment_manifests(&[rb], Some(TimelineName::from("time"))).unwrap();
        assert!(!manifests.contains_key(&SegmentId::from("segA")));
        let b = &manifests[&SegmentId::from("segB")];
        assert_eq!(b.outstanding_count(), 1);
    }

    /// Non-OSS servers may emit `:start` as `TimestampNanosecondArray`
    /// (or any other nanosecond-typed array) instead of `Int64`. Both
    /// shapes are i64 under the hood and must be accepted.
    #[test]
    fn test_build_segment_manifests_accepts_timestamp_ns_start_column() {
        use arrow::array::TimestampNanosecondArray;

        let num_rows = 2;
        let segment_ids = StringArray::from(vec!["seg1", "seg1"]);
        let entity_paths = StringArray::from(vec!["/a", "/b"]);
        let is_static = BooleanArray::from(vec![false, false]);
        let starts = TimestampNanosecondArray::from(vec![Some(100_i64), Some(200_i64)]);

        // The OSS schema contract nominally types `:start` as Int64, but the
        // server sends a TimestampNanosecond column. Build the field as
        // Timestamp here (`RecordBatch` requires field type == array type) to
        // validate `TimeColumn::read_nullable_array` accepts i64-backed
        // timestamp arrays.
        let schema = Arc::new(Schema::new_with_metadata(
            vec![
                QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.arrow_field(),
                QueryDatasetDataframe::COLUMN_CHUNK_ENTITY_PATH.arrow_field(),
                QueryDatasetDataframe::COLUMN_CHUNK_IS_STATIC.arrow_field(),
                Field::new(
                    "time:start",
                    arrow::datatypes::DataType::Timestamp(
                        arrow::datatypes::TimeUnit::Nanosecond,
                        None,
                    ),
                    true,
                ),
            ],
            HashMap::default(),
        ));

        let rb = RecordBatch::try_new_with_options(
            schema,
            vec![
                Arc::new(segment_ids),
                Arc::new(entity_paths),
                Arc::new(is_static),
                Arc::new(starts),
            ],
            &RecordBatchOptions::new().with_row_count(Some(num_rows)),
        )
        .unwrap();

        let manifests = build_segment_manifests(&[rb], Some(TimelineName::from("time"))).unwrap();
        let m = manifests
            .get(&SegmentId::from("seg1"))
            .expect("seg1 has temporal chunks");
        assert_eq!(m.outstanding_count(), 2);
        // safe_horizon = laggard.predecessor → 100 - 1 = 99.
        assert_eq!(m.safe_horizon(), Some(TimeInt::new_temporal(99)));
    }
}
