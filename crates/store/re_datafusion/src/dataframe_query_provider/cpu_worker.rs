//! CPU-side worker that consumes decoded chunks from the IO pipeline,
//! routes them to per-segment in-memory stores, drives the query handle,
//! and emits aligned `RecordBatch`es downstream.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch, RecordBatchOptions, StringArray};
use arrow::datatypes::Schema;
use re_dataframe::external::re_chunk::{Chunk, ChunkId, LatestAtQuery};
use re_dataframe::external::re_chunk_store::{
    ChunkStore, ChunkTrackingMode, GarbageCollectionOptions,
};
use re_dataframe::utils::align_record_batch_to_schema;
use re_dataframe::{
    ChunkStoreConfig, ChunkStoreHandle, QueryCache, QueryEngine, QueryExpression, QueryHandle,
    StorageEngine, TimelineName,
};
use re_log_types::{AbsoluteTimeRange, ApplicationId, StoreId, StoreKind, TimeInt};
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_protos::common::v1alpha1::ext::SegmentId;
use re_redap_client::{ApiError, ApiResult};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{Instrument as _, instrument};

use crate::chunk_fetcher::SortedChunksWithSegment;
use crate::dataframe_query_common::{
    DEFAULT_BATCH_BYTES, DEFAULT_BATCH_ROWS, IndexValuesMap, prepend_string_column_schema,
};
use crate::pipeline_budget::PipelineBudget;
use crate::segment_chunk_manifest::SegmentChunkManifest;

/// Per-batch caps used by `send_next_row_batch`.
///
/// Accumulating up to `DEFAULT_BATCH_ROWS` rows or `DEFAULT_BATCH_BYTES` bytes
/// (whichever first) amortizes per-batch overhead (alloc, schema align, async
/// channel send) while keeping batch memory bounded for wide columns
/// (e.g. images, large lists, replicated video blobs from retrofill).
///
/// These mirror the values used by `SizedCoalesceBatchesExec` so that the
/// downstream coalescer is mostly a pass-through.
const FLUSH_BATCH_ROWS: usize = DEFAULT_BATCH_ROWS;
const FLUSH_BATCH_BYTES: usize = DEFAULT_BATCH_BYTES as usize;

#[tracing::instrument(level = "trace", skip_all, fields(segment_id = %segment_id))]
async fn send_next_row_batch(
    query_handle: &mut QueryHandle<StorageEngine>,
    segment_id: &SegmentId,
    target_schema: &Arc<Schema>,
    output_channel: &Sender<RecordBatch>,
    rows_sent: &mut usize,
    limit_rows: Option<usize>,
) -> ApiResult<Option<()>> {
    // If we have already sent enough rows, stop early.
    if limit_rows.is_some_and(|l| *rows_sent >= l) {
        return Ok(None);
    }

    let max_rows_this_batch = limit_rows
        .map(|l| l.saturating_sub(*rows_sent).min(FLUSH_BATCH_ROWS))
        .unwrap_or(FLUSH_BATCH_ROWS);
    if max_rows_this_batch == 0 {
        return Ok(None);
    }

    let query_schema = Arc::clone(query_handle.schema());
    let num_fields = query_schema.fields.len();

    // `_next_n_rows` carries its own `profile_function!`, so no extra scope here.
    // Wrapping the `.await` in a `profile_scope!` would hold a non-`Send` guard
    // across the suspension point and break `Handle::spawn`'s `Send` bound.
    let next = query_handle
        .next_n_rows_async(max_rows_this_batch, FLUSH_BATCH_BYTES)
        .await;
    if next.num_rows == 0 {
        return Ok(None);
    }
    if num_fields != next.columns.len() {
        return Err(ApiError::internal(
            "Unexpected number of columns returned from query",
        ));
    }
    let total_rows = next.num_rows;

    let mut columns: Vec<ArrayRef> = Vec::with_capacity(num_fields + 1);
    let sid_array =
        Arc::new(StringArray::from(vec![segment_id.to_string(); total_rows])) as ArrayRef;
    columns.push(sid_array);
    columns.extend(next.columns);

    let output_batch = {
        re_tracing::profile_scope!("build_and_align_batch");
        let batch_schema = Arc::new(prepend_string_column_schema(
            &query_schema,
            ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
        ));

        let batch = RecordBatch::try_new_with_options(
            batch_schema,
            columns,
            &RecordBatchOptions::default().with_row_count(Some(total_rows)),
        )
        .map_err(|err| {
            ApiError::deserialization_with_source(
                None,
                err,
                "building output record batch from chunk-store rows",
            )
        })?;

        align_record_batch_to_schema(&batch, target_schema).map_err(|err| {
            ApiError::internal_with_source(None, err, "DataFusion schema mismatch error")
        })?
    };

    // Slice the batch to respect the row limit. We pre-cap `max_rows_this_batch`
    // by the limit, but a single `next_row()` call can return more than one row
    // (see `_next_row` for multi-row index values), so a final trim is needed.
    let output_batch = if let Some(limit_rows) = limit_rows {
        let remaining = limit_rows.saturating_sub(*rows_sent);
        if remaining == 0 {
            return Ok(None);
        }
        if output_batch.num_rows() > remaining {
            output_batch.slice(0, remaining)
        } else {
            output_batch
        }
    } else {
        output_batch
    };

    *rows_sent += output_batch.num_rows();

    output_channel
        .send(output_batch)
        .await
        .map_err(|err| ApiError::internal_with_source(None, err, "output channel closed"))?;

    Ok(Some(()))
}

/// Message type carried over the IO → CPU channel.
///
/// The IO side announces each segment's bookkeeping *before*
/// dispatching its fetches: a [`Self::SegmentChunkCount`] always, and
/// a [`Self::SegmentManifest`] whenever the query has a temporal
/// `filtered_index` and the server returned `:start` columns. The CPU
/// worker uses the count for completion detection and the manifest
/// for incremental safe-horizon emit + GC.
///
/// Ordering: under the current IO loop, both metadata messages for a
/// given segment are sent before any [`Self::Chunks`] for that
/// segment (the channel is FIFO, so the worker observes them in that
/// order). The [`Self::SegmentChunkCount`] handler is order-tolerant
/// — it can arrive after some [`Self::Chunks`] and still trigger
/// completion correctly. The [`Self::SegmentManifest`] handler is
/// **not** order-tolerant: it only updates `manifest`, so chunks that
/// arrived before the manifest were never recorded against it and
/// `safe_horizon` would stay pinned at their minimum `time_min`,
/// stalling incremental emit until the segment completes via the
/// count-based path.
pub(super) enum CpuWorkerMsg {
    /// Total number of chunks the server has announced for `segment_id`.
    /// Drives "segment-done, do the final drain" detection.
    SegmentChunkCount { segment_id: SegmentId, count: usize },

    /// Per-segment temporal manifest built from the
    /// `{filtered_timeline}:start` column on the `chunk_info` rows.
    /// Sent only when the query has a temporal `filtered_index` *and*
    /// the server returned `:start`. Drives incremental safe-horizon
    /// emit + GC. Absence means the worker falls back to
    /// "emit at completion" semantics for this segment.
    ///
    /// `Box`'d so the enum's max variant size stays compact — the
    /// manifest can hold thousands of entries on wide segments and
    /// would otherwise dominate the size of every `Chunks` message.
    SegmentManifest {
        segment_id: SegmentId,
        manifest: Box<SegmentChunkManifest>,
    },

    /// A batch of decoded chunks for one segment. Multiple `Chunks`
    /// messages may arrive per segment.
    Chunks(SortedChunksWithSegment),
}

/// Per-segment in-memory store used by the CPU worker, plus the state
/// needed to incrementally emit rows + GC chunks as the safe horizon
/// advances.
///
/// Holds an `Arc<PipelineBudget>` and refunds the bytes currently in
/// `store` to the budget on [`Drop`]. This covers every exit path
/// uniformly — `flush` success, `flush` error via `?`, worker
/// early-return on an upstream error, consumer hangup mid-segment,
/// panic — since `Drop` is guaranteed to run exactly once. Incremental
/// [`Self::gc_up_to_horizon`] calls also return freed bytes to the
/// budget as the horizon advances. Without the refund a `?`
/// early-return or cancellation would leak the reservation to sibling
/// partitions for the remainder of the query.
///
/// The `QueryHandle` *cannot* be cached across emit cycles:
/// `QueryHandle` snapshots the store's `view_chunks` at first
/// `next_n_rows` and never refreshes them, so any chunks inserted after
/// that first call would be invisible. [`Self::emit_up_to`] therefore
/// builds a fresh `QueryHandle` per call and uses
/// `filtered_index_range = (processed_through, horizon]` to avoid
/// re-emitting rows already produced by earlier cycles.
///
/// The `QueryEngine` *is* reusable — it's a thin wrapper around the
/// store + query cache handles, both of which are live views — so we
/// build it once in [`Self::new`] and reuse it across every emit cycle,
/// saving an `Arc::clone` pair per cycle.
struct CurrentStores {
    segment_id: SegmentId,
    store: ChunkStoreHandle,

    /// Built once and reused across every `emit_up_to` cycle. The
    /// store + cache handles inside are live views, so post-construction
    /// chunk inserts and cache updates are visible without rebuilding.
    engine: QueryEngine<StorageEngine>,

    /// Per-segment specialization of the worker's base `QueryExpression`,
    /// with `using_index_values` already applied. `emit_up_to` clones
    /// and mutates `filtered_index_range` on each call.
    query_expression: QueryExpression,

    /// Cached name of the query's `filtered_index` timeline. `None` for
    /// static-only queries; matches the manifest's gating on horizon
    /// emit.
    filtered_index_timeline: Option<TimelineName>,
    pipeline_budget: Arc<PipelineBudget>,

    /// Total number of chunks the server promised for this segment via
    /// [`CpuWorkerMsg::SegmentChunkCount`]. `None` until that message
    /// arrives.
    expected_chunks: Option<usize>,

    /// Cumulative chunks inserted into `store` so far. Compared against
    /// `expected_chunks` to detect segment completion.
    received_chunks: usize,

    /// Per-segment safe-horizon tracker. `None` for static-only
    /// queries or when the server returned no `:start` columns (old
    /// server fallback). Drives the horizon-based emit + GC paths
    /// inside [`Self::flush_incremental`].
    manifest: Option<SegmentChunkManifest>,

    /// Upper bound of the time range already processed by an
    /// [`Self::emit_up_to`] call (inclusive). Used as
    /// `filtered_index_range.min - 1` on the next emit cycle so we don't
    /// re-query rows that have already been considered. `None` means no
    /// emit cycle has run yet.
    ///
    /// Tracks the *processed* range, not the *emitted* row count: a
    /// cycle that finds zero matching rows in `(prev, horizon]` still
    /// advances this so the next cycle starts at `horizon + 1`. Without
    /// that, an empty cycle would re-scan the same range on every
    /// horizon tick.
    processed_through_time: Option<TimeInt>,

    /// Most recent value `safe_horizon` returned in `flush_incremental`.
    /// Carried only to back the `debug_assert!` that the horizon is
    /// monotonically non-decreasing — if it ever regresses we'd
    /// re-emit rows we already shipped, or `gc_up_to_horizon` would
    /// drop chunks whose rows haven't been emitted yet.
    last_horizon: Option<TimeInt>,

    /// Largest `time_range().max()` seen across every arrived chunk on
    /// the filtered timeline. `None` until the first temporal chunk
    /// arrives.
    ///
    /// Used by [`Self::flush_incremental`] as a cheap pre-check: if no
    /// arrived chunk has rows past `processed_through_time`, building a
    /// fresh `QueryHandle` cannot produce output, so we skip the build.
    /// `time_max` (not `time_min`) is the right bound because a single
    /// chunk's rows can straddle a horizon — a chunk at `time_min=10`,
    /// `time_max=100` still has emittable rows after `processed_through`
    /// crosses 10.
    max_arrived_time_max: Option<TimeInt>,

    /// Whether this segment has already left the segment-count gate.
    /// Completed segments can wait in `ready_pending` behind earlier
    /// segments, but they no longer need IO admission, so holding their
    /// segment slot would block the missing earlier segments that are
    /// required to make ordered emit progress.
    segment_slot_released: bool,

    /// Scratch storage for the `protected_chunks` set built by
    /// [`Self::gc_up_to_horizon`]. Kept on the struct (rather than
    /// allocated per call) so the underlying `HashMap` capacity is
    /// reused across the many GC ticks that fire once the horizon
    /// starts advancing. `.clear()` resets size without freeing the
    /// table; `std::mem::take` moves the populated set into
    /// `GarbageCollectionOptions` for the `gc()` call, then a swap
    /// restores ownership (and capacity) to this field.
    protected_chunks_scratch: ahash::HashSet<ChunkId>,
}

impl CurrentStores {
    #[tracing::instrument(level = "debug", skip_all, fields(segment_id = %segment_id))]
    fn new(
        segment_id: SegmentId,
        query_expression: &QueryExpression,
        index_values: &IndexValuesMap,
        pipeline_budget: Arc<PipelineBudget>,
    ) -> Self {
        let store_id = StoreId::random(
            StoreKind::Recording,
            ApplicationId::from(segment_id.as_ref()),
        );
        let config = ChunkStoreConfig::ALL_DISABLED; // Don't spend CPU time splitting and joining chunks. Trust the input.
        let store = ChunkStore::new_handle(store_id.clone(), config);
        let query_cache = QueryCache::new_handle(store.clone());
        let engine = QueryEngine::new(store.clone(), query_cache);

        let mut individual_query = query_expression.clone();
        let values = index_values
            .as_ref()
            .and_then(|index_values| index_values.get(&segment_id));
        if let Some(values) = values {
            individual_query.using_index_values = Some(values.clone());
        }
        let filtered_index_timeline = individual_query.filtered_index;

        Self {
            segment_id,
            store,
            engine,
            query_expression: individual_query,
            filtered_index_timeline,
            pipeline_budget,
            expected_chunks: None,
            received_chunks: 0,
            manifest: None,
            processed_through_time: None,
            last_horizon: None,
            max_arrived_time_max: None,
            segment_slot_released: false,
            protected_chunks_scratch: ahash::HashSet::default(),
        }
    }

    fn release_segment_slot(&mut self) {
        if !self.segment_slot_released {
            self.pipeline_budget
                .publish_segment_finalized(self.segment_id.as_str());
            self.segment_slot_released = true;
        }
    }

    /// Current decoded bytes held in `store`. Reads `ChunkStore` stats so
    /// the value reflects any post-construction inserts and any chunks
    /// reclaimed as the safe horizon advances.
    fn store_bytes(&self) -> u64 {
        self.store.read().stats().total().total_size_bytes
    }

    /// `true` once the IO side has announced the chunk count for this
    /// segment **and** every announced chunk has been inserted. Used by
    /// the CPU worker to decide when to fire the flush + drop cycle.
    fn is_complete(&self) -> bool {
        self.expected_chunks
            .is_some_and(|expected| self.received_chunks >= expected)
    }

    /// Record a chunk's arrival on the filtered timeline.
    /// No-op for static-only queries (no `filtered_index_timeline`) and
    /// for chunks with no data on that timeline.
    ///
    /// `max_arrived_time_max` is updated whenever the chunk is temporal
    /// on the filtered timeline, even if no manifest has been attached
    /// yet — `flush_incremental`'s path-1b fast-skip relies on it being
    /// monotonic across the segment's full arrival history. If a chunk
    /// arrived before the manifest (against current IO-loop ordering,
    /// but documented as a latent edge case) and we hadn't tracked it
    /// here, path-1b would later treat the segment as "no arrivals in
    /// range" and advance `processed_through_time` past rows that were
    /// actually emittable.
    ///
    /// `manifest.record_arrival` is only called when a manifest is
    /// present — the manifest's own bookkeeping doesn't exist without
    /// it. On manifest/chunk divergence (the `(entity, time_min)` pair
    /// was never announced in the `chunk_info`), `debug_panic!` fires
    /// in debug builds and `error_once!` in release. The chunk still
    /// inserts — silently dropping the chunk would be a worse choice
    /// than emitting it past a now-incorrect horizon — but the log
    /// surfaces the integrity issue so operators can investigate.
    fn record_arrival(&mut self, chunk: &Chunk) {
        let Some(timeline) = self.filtered_index_timeline.as_ref() else {
            return;
        };
        let Some(time_col) = chunk.timelines().get(timeline) else {
            return; // chunk has no data on this timeline (static, or other timelines only)
        };
        let entity_path = chunk.entity_path();
        let time_range = time_col.time_range();
        let time_min = time_range.min();
        let time_max = time_range.max();

        // Track the highest `time_max` across all arrivals so
        // `flush_incremental` can short-circuit when no arrived chunk
        // has rows past `processed_through_time`. Done outside the
        // manifest guard so pre-manifest arrivals still register.
        self.max_arrived_time_max = Some(
            self.max_arrived_time_max
                .map_or(time_max, |prev| prev.max(time_max)),
        );

        let Some(manifest) = self.manifest.as_mut() else {
            return;
        };

        if !manifest.record_arrival(entity_path, time_min) {
            re_log::debug_panic!(
                "manifest/chunk divergence: chunk for entity {entity_path} at time_min={} on \
                 timeline {timeline} was not announced in chunk_info; safe_horizon may have \
                 advanced past it, in which case its rows will be excluded by the row range \
                 filter and never emit",
                time_min.as_i64(),
            );
            re_log::error_once!(
                "manifest/chunk divergence: chunk for entity {entity_path} at time_min={} on \
                 timeline {timeline} not found in manifest; safe horizon may be inaccurate",
                time_min.as_i64(),
            );
        }
    }

    /// Run the safe-horizon emit + GC step for this segment.
    ///
    /// Two paths:
    /// 1. **Fast skip.** No manifest, or horizon hasn't advanced since
    ///    the last emit, or already at horizon = max → no work, no
    ///    `next_n_rows` call.
    /// 2. **Horizon emit + GC.** Manifest's `safe_horizon` advanced →
    ///    emit rows up to and including the new horizon, then drop
    ///    chunks strictly below it from the in-memory store and
    ///    release the freed bytes to the budget.
    ///
    /// Callers MUST only invoke this on the segment at the head of
    /// `emit_order`. Emitting rows from a non-head segment would
    /// violate the `[segment_id ASC, sort_index ASC]` ordering claim
    /// advertised by `SegmentStreamExec::try_new`.
    async fn flush_incremental(
        &mut self,
        projected_schema: &Arc<Schema>,
        output_channel: &Sender<RecordBatch>,
        rows_sent: &mut usize,
        limit_rows: Option<usize>,
    ) -> ApiResult<()> {
        // `using_index_values` overrides `filtered_index_range` inside
        // `QueryHandle::range_query` (see `re_dataframe::query`): the
        // emitted row set is derived from the explicit
        // `using_index_values` list, not from the per-cycle range we
        // set on `filtered_index_range`. Running the incremental path
        // here would therefore replay every `using_index_values` row
        // on every cycle and duplicate output. Skip the incremental path;
        // the later completion flush emits each row exactly once.
        // GC must be skipped too — without incremental emit, dropping
        // pre-horizon chunks would corrupt that final drain.
        if self.query_expression.using_index_values.is_some() {
            return Ok(());
        }
        let Some(horizon) = self.manifest.as_ref().and_then(|m| m.safe_horizon()) else {
            // No horizon info available — feeds the stall detector
            // because nothing else will here.
            self.pipeline_budget.notify_empty_emit();
            return Ok(()); // path 1 — no horizon info available
        };

        // The horizon is required to be monotonically non-decreasing
        // for the design to hold: a regression would imply either
        // re-emitting rows already shipped or GC'ing chunks whose
        // rows still need to emit. Catch it in debug builds while the
        // damage is recoverable; in release the existing range filter
        // makes the failure mode silent-but-survivable.
        re_log::debug_assert!(
            self.last_horizon.is_none_or(|prev| horizon >= prev),
            "safe_horizon regressed: prev={:?}, new={}",
            self.last_horizon.map(|h| h.as_i64()),
            horizon.as_i64(),
        );
        self.last_horizon = Some(horizon);

        // Path 1 (fast skip): horizon hasn't advanced past the range
        // already processed.
        if let Some(last) = self.processed_through_time
            && horizon <= last
        {
            self.pipeline_budget.notify_empty_emit();
            return Ok(());
        }

        // Path 1b (no-arrivals-in-range fast skip): no arrived chunk
        // has rows past `processed_through_time`, so the upcoming emit
        // cycle cannot produce output. Skip the `QueryHandle` build but
        // still run GC — the horizon advanced (per path 1's check
        // above), and GC at the new horizon may free chunks even when
        // no new rows are emittable. Advance `processed_through_time`
        // so the next cycle's `filtered_index_range.min` skips the
        // empty range we just confirmed; safe because the manifest
        // invariant guarantees no future chunk arrives with `time_min`
        // <= horizon.
        if self
            .max_arrived_time_max
            .is_none_or(|tmax| self.processed_through_time.is_some_and(|p| tmax <= p))
        {
            // No emittable rows in range — still an empty emit cycle,
            // so feed the stall detector before short-circuiting.
            self.pipeline_budget.notify_empty_emit();
            self.gc_up_to_horizon(horizon);
            self.processed_through_time = Some(horizon);
            return Ok(());
        }

        // Path 2: emit + GC up to the new horizon.
        let rows_before = *rows_sent;
        self.emit_up_to(
            Some(horizon),
            projected_schema,
            output_channel,
            rows_sent,
            limit_rows,
        )
        .await?;
        if *rows_sent > rows_before {
            self.pipeline_budget.notify_row_emitted();
        } else {
            self.pipeline_budget.notify_empty_emit();
        }
        self.gc_up_to_horizon(horizon);
        Ok(())
    }

    /// Build a fresh `QueryEngine` + `QueryHandle` constrained to
    /// `(processed_through_time, horizon]` on the filtered timeline,
    /// then drain rows from that handle through `send_next_row_batch`
    /// until it reports exhaustion. Updates `processed_through_time` to
    /// `horizon` on success, regardless of whether any rows shipped:
    /// the range has been *considered*, so the next cycle must not
    /// re-scan it.
    ///
    /// `horizon = None` means "up to `TimeInt::MAX`" — the final drain
    /// path. For queries with no `filtered_index` the range is silently
    /// ignored by `QueryExpression` (per its documented semantics);
    /// such queries are static-only and `processed_through_time` is set
    /// to `MAX` after the first call so subsequent invocations short-
    /// circuit.
    async fn emit_up_to(
        &mut self,
        horizon: Option<TimeInt>,
        projected_schema: &Arc<Schema>,
        output_channel: &Sender<RecordBatch>,
        rows_sent: &mut usize,
        limit_rows: Option<usize>,
    ) -> ApiResult<()> {
        // Range_min = processed_through + 1, defaulting to MIN on first
        // emit. `TimeInt::inc` handles the saturating add and the
        // `processed_through == MAX` edge — `.inc()` of `MAX` returns
        // `MAX`, so the `range_min > range_max` guard below catches it.
        let range_min = match self.processed_through_time {
            Some(t) => {
                if t == TimeInt::MAX {
                    return Ok(());
                }
                t.inc()
            }
            None => TimeInt::MIN,
        };
        let range_max = horizon.unwrap_or(TimeInt::MAX);
        if range_min > range_max {
            return Ok(());
        }

        // `QueryEngine::query(QueryExpression)` consumes the expression
        // by value, so we can't borrow `self.query_expression` here —
        // each cycle must hand the engine an owned copy. Cloning is
        // unavoidable until that API gains a by-ref variant; the small
        // per-cycle allocation cost is acceptable next to the much
        // larger `next_n_rows` work that follows.
        let mut q = self.query_expression.clone();
        q.filtered_index_range = Some(AbsoluteTimeRange::new(range_min, range_max));

        let mut handle: QueryHandle<StorageEngine> = self.engine.query(q);
        while send_next_row_batch(
            &mut handle,
            &self.segment_id,
            projected_schema,
            output_channel,
            rows_sent,
            limit_rows,
        )
        .await?
        .is_some()
        {}

        self.processed_through_time = Some(range_max);
        Ok(())
    }

    /// Drop chunks no longer needed once the safe horizon has
    /// advanced, returning the freed bytes to the pipeline budget.
    /// No-op for queries without a temporal `filtered_index`.
    ///
    /// **Carry-forward protection.** A naive "drop everything with
    /// `time_max < horizon`" would corrupt latest-at semantics:
    /// rerun queries resolve a row at time `T` to the *most recent*
    /// component value at or before `T`, so the chunk that holds an
    /// entity's last-known value before the horizon must stay around
    /// to keep supplying that value for rows past the horizon.
    /// Example: entity `/a` has its only chunk at `t=10`; entity
    /// `/b` has chunks at `t=20, 40`. With horizon `39`, dropping
    /// `/a@10` would make every row in `[10, 39]` and beyond emit
    /// `/a` as null instead of carrying its `t=10` value forward.
    ///
    /// To preserve that invariant, we ask the chunk store for the
    /// set of chunks that would satisfy
    /// `LatestAtQuery::new(timeline, horizon)` for every entity and
    /// add them to `protected_chunks`. Everything outside that set
    /// **and** outside `(horizon, +inf]` is fair game.
    fn gc_up_to_horizon(&mut self, horizon: TimeInt) {
        let Some(timeline_name) = self.filtered_index_timeline else {
            return;
        };

        let bytes_before = self.store_bytes();

        // Collect chunk IDs that supply latest-at carry-forward
        // values at the horizon — these must survive the GC even
        // though their entire time range may be ≤ horizon.
        //
        // Reuse `protected_chunks_scratch`: `.clear()` drops elements
        // but keeps the HashMap's allocated buckets. The set is then
        // `mem::take`'d into `GarbageCollectionOptions` (which needs
        // ownership for `gc(&options)`), and swapped back after the
        // call so the next GC tick inherits the capacity.
        self.protected_chunks_scratch.clear();
        {
            let store = self.store.read();
            let query = LatestAtQuery::new(timeline_name, horizon);
            for entity_path in store.all_entities() {
                let results = store.latest_at_relevant_chunks_for_all_components(
                    ChunkTrackingMode::Ignore,
                    &query,
                    &entity_path,
                    true, // include static
                );
                for chunk in &results.chunks {
                    self.protected_chunks_scratch.insert(chunk.id());
                }
            }
        }

        // Build `GarbageCollectionOptions` via `gc_everything()` so we
        // inherit the right `IntMap` hasher for `protected_time_ranges`
        // without needing a direct `nohash_hasher` dependency.
        let mut options = GarbageCollectionOptions::gc_everything();
        options.protected_chunks = std::mem::take(&mut self.protected_chunks_scratch);
        options.protected_time_ranges.insert(
            timeline_name,
            AbsoluteTimeRange::new(horizon.inc(), TimeInt::MAX),
        );
        options.perform_deep_deletions = true;
        // `gc` returns the list of removed chunks plus stats; we
        // measure freed bytes via `store_bytes()` before/after
        // instead, so the structured return value is intentionally
        // discarded.
        let _ = self.store.write().gc(&options);
        // Restore the populated set to the field so its capacity
        // survives for the next call. Contents are dropped on the
        // next entry via `.clear()`; only the table backing storage
        // is the reuse target.
        std::mem::swap(
            &mut self.protected_chunks_scratch,
            &mut options.protected_chunks,
        );

        let bytes_after = self.store_bytes();
        let freed = bytes_before.saturating_sub(bytes_after);
        if freed > 0 {
            self.pipeline_budget.release(freed as usize);
        }
    }

    /// Final drain. Emits everything still in `store` that's past
    /// `processed_through_time` through the output channel. Consumes `self`
    /// so the reservation is returned via `Drop` immediately after the
    /// last batch ships — and via the same `Drop` if a
    /// `send_next_row_batch` error short-circuits the loop via `?`.
    #[instrument(level = "debug", skip_all)]
    async fn flush(
        mut self,
        projected_schema: &Arc<Schema>,
        output_channel: &Sender<RecordBatch>,
        rows_sent: &mut usize,
        limit_rows: Option<usize>,
    ) -> ApiResult<()> {
        self.emit_up_to(
            None,
            projected_schema,
            output_channel,
            rows_sent,
            limit_rows,
        )
        .await?;
        Ok(())
    }
}

impl Drop for CurrentStores {
    fn drop(&mut self) {
        // Refund whatever the store currently holds. See the long
        // comment in the CPU worker about why `store_bytes >= reserved_sum`
        // and the resulting under-utilization is benign.
        self.pipeline_budget.release(self.store_bytes() as usize);

        // Free the segment's slot in the segment-count gate so a
        // parked higher-priority reserver can be admitted. The byte
        // refund above and the segment-gate slot are independent: the
        // slot must be vacated here regardless of how the bytes were
        // released.
        self.release_segment_slot();
    }
}

// TODO(#10781) - support for sending intermediate results/chunks
#[instrument(level = "info", skip_all)]
pub(super) async fn chunk_store_cpu_worker_thread(
    mut input_channel: Receiver<ApiResult<CpuWorkerMsg>>,
    output_channel: Sender<RecordBatch>,
    query_expression: QueryExpression,
    projected_schema: Arc<Schema>,
    index_values: IndexValuesMap,
    limit_rows: Option<usize>,
    pipeline_budget: Arc<PipelineBudget>,
) -> ApiResult<()> {
    // One in-memory store per in-flight segment. The previous design held
    // at most one (`Option<CurrentStores>`) and depended on the IO side
    // delivering chunks one segment at a time, gated by a reorder buffer.
    // The `HashMap` removes that constraint: chunks may interleave across
    // segments in arrival order, and each segment finalizes independently
    // when its `SegmentChunkCount` is satisfied.
    //
    // Emit order is then re-imposed by `emit_order` + `ready_pending` —
    // see those fields' comments. Without that gate, smaller / faster
    // segments could overtake earlier ones, violating the
    // `[segment_id ASC, sort_index ASC]` ordering advertised by
    // `SegmentStreamExec::try_new`.
    //
    // The release-accounting story below applies per segment, unchanged:
    // the `release` call uses `ChunkStore::stats().total().total_size_bytes`,
    // while the IO side reserves using `re_byte_size::SizeBytes::total_size_bytes`
    // summed per inserted chunk. Both internally invoke the same
    // `SizeBytes::total_size_bytes` on each `Arc<Chunk>` (see
    // `ChunkStoreChunkStats::from_chunk`), so per-chunk the metrics
    // are identical. They diverge only when the store performs chunk
    // compaction at insert time: an incoming chunk A may be merged with
    // an existing chunk B into `C = concat(A, B)`, after which the
    // store's stats reflect `+C - B = A + concat_overhead` while the IO
    // accounting only added `A`. `concat_overhead` is strictly
    // non-negative (concatenation allocates new arrow buffers; it
    // cannot shrink the data), so `store_bytes >= reserved_sum` a
    // priori. Empirically the overhead is ~0.3-0.5% of decoded data on
    // representative workloads (see PR #1736 review). This means
    // `release` may return slightly more than `reserve` charged, so
    // `current` saturates toward 0 marginally early — which
    // under-utilises the budget by the same fraction but poses no OOM
    // or deadlock risk. Treated as benign.
    let mut current_stores: HashMap<SegmentId, CurrentStores> = HashMap::new();
    // Segments whose `is_complete()` has fired (all announced chunks
    // inserted) but whose turn to emit has not yet come because an
    // earlier-in-announcement-order segment is still in flight.
    //
    // The worker preserves announcement order — which is segment_id ASC
    // since `group_chunk_infos_by_segment_id` sorts via a `BTreeMap` —
    // so the plan's `EquivalenceProperties::new_with_orderings(
    // [segment_id ASC, …])` claim from `SegmentStreamExec::try_new`
    // stays honest.
    //
    // Memory cost: a slow head-of-line segment pins later completed
    // segments here. Their reservations stay charged to the pipeline
    // budget via the per-segment `Drop` refund (it fires when each
    // `CurrentStores` is finally flushed and dropped), so OOM-safe.
    // Budget-side back-pressure (PR 5/6 in this stack) caps the depth.
    let mut ready_pending: HashMap<SegmentId, CurrentStores> = HashMap::new();
    // Segments whose flush has already fired and been emitted. Used to
    // log+drop any late-arriving `Chunks` or duplicate
    // `SegmentChunkCount` for that segment. The server is expected to
    // be authoritative on per-segment chunk counts; arriving here would
    // indicate a protocol mismatch or a retry. Dropping (rather than
    // erroring) keeps the rest of the query streaming.
    let mut completed_segments: HashSet<SegmentId> = HashSet::new();
    // Announcement order of segments — the order in which they must be
    // emitted downstream. Pushed once when a segment is first observed
    // (announce or first-`Chunks`, whichever arrives first), popped
    // from the front when that segment finally flushes.
    let mut emit_order: VecDeque<SegmentId> = VecDeque::new();
    let mut rows_sent: usize = 0;
    loop {
        // Time spent here = `cpu_worker` idle waiting for the IO pipeline to
        // deliver the next batch of chunks. Short consecutive spans = healthy
        // stream; one long dominating span = IO-starved worker.
        let recv_span = tracing::trace_span!("waiting_for_chunks");
        let Some(msg) = input_channel.recv().instrument(recv_span).await else {
            break;
        };

        match msg? {
            CpuWorkerMsg::SegmentChunkCount { segment_id, count } => {
                if completed_segments.contains(&segment_id)
                    || ready_pending.contains_key(&segment_id)
                {
                    tracing::warn!(
                        %segment_id,
                        count,
                        "duplicate SegmentChunkCount for already-completed segment; dropping. \
                         Server is expected to be authoritative; indicates protocol mismatch or retry."
                    );
                    continue;
                }
                if index_values
                    .as_ref()
                    .is_some_and(|iv| !iv.contains_key(&segment_id))
                {
                    continue;
                }
                let stores = match current_stores.entry(segment_id.clone()) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        emit_order.push_back(segment_id.clone());
                        e.insert(CurrentStores::new(
                            segment_id.clone(),
                            &query_expression,
                            &index_values,
                            pipeline_budget.clone(),
                        ))
                    }
                };
                if let Some(prev) = stores.expected_chunks {
                    if prev != count {
                        tracing::warn!(
                            %segment_id, prev, count,
                            "conflicting SegmentChunkCount for in-flight segment; keeping first. \
                             Indicates IO-side duplicate announce or server protocol mismatch."
                        );
                    }
                    // Either equal (benign duplicate) or conflicting
                    // (warned above): keep the first value to avoid
                    // moving the completion target under the worker.
                } else {
                    stores.expected_chunks = Some(count);
                }
                // If chunks already satisfied the count before the
                // announcement arrived (out-of-order on the channel),
                // park the segment as ready-to-emit and try to drain.
                if stores.is_complete() {
                    stores.release_segment_slot();
                    let taken = current_stores
                        .remove(&segment_id)
                        .expect("just inserted via entry()");
                    ready_pending.insert(segment_id, taken);
                    if maybe_emit_head(
                        &mut emit_order,
                        &mut current_stores,
                        &mut ready_pending,
                        &mut completed_segments,
                        &projected_schema,
                        &output_channel,
                        &mut rows_sent,
                        limit_rows,
                    )
                    .await?
                    {
                        return Ok(());
                    }
                }
            }
            CpuWorkerMsg::SegmentManifest {
                segment_id,
                manifest,
            } => {
                if completed_segments.contains(&segment_id)
                    || ready_pending.contains_key(&segment_id)
                {
                    tracing::warn!(
                        %segment_id,
                        "SegmentManifest for already-completed segment; dropping. \
                         Indicates a protocol mismatch or retry."
                    );
                    continue;
                }
                if index_values
                    .as_ref()
                    .is_some_and(|iv| !iv.contains_key(&segment_id))
                {
                    continue;
                }
                let stores = match current_stores.entry(segment_id.clone()) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        emit_order.push_back(segment_id.clone());
                        e.insert(CurrentStores::new(
                            segment_id.clone(),
                            &query_expression,
                            &index_values,
                            pipeline_budget.clone(),
                        ))
                    }
                };
                // Overwriting a manifest mid-segment is a protocol bug:
                // any `record_arrival` calls already made against the
                // first manifest are silently discarded with the
                // overwrite, which would inflate `outstanding_count` on
                // the replacement and pin `safe_horizon` until the
                // count-based completion path fires. Trip
                // `debug_panic!` in debug so the protocol mismatch
                // surfaces; in release, log once and drop the second
                // manifest rather than mute the first one.
                if stores.manifest.is_some() {
                    re_log::debug_panic!(
                        "duplicate SegmentManifest for segment {segment_id}; ignoring \
                         second manifest. Indicates a protocol mismatch in the IO loop."
                    );
                    re_log::error_once!(
                        "duplicate SegmentManifest for segment {segment_id}; ignoring \
                         second manifest. Indicates a protocol mismatch in the IO loop."
                    );
                } else {
                    stores.manifest = Some(*manifest);
                }
                // Drive a head-of-line pass: if this segment is the
                // head, the freshly attached manifest plus any chunks
                // that arrived alongside it (in the same recv burst)
                // may already let the horizon advance.
                //
                // Note: in the current IO loop, manifests for a
                // segment arrive before any of its chunks (FIFO send
                // order), so the in-store chunk count is typically
                // zero here and this call is a near no-op. If that
                // ordering ever changes, chunks that arrived before
                // the manifest will not have been recorded against
                // it, and the manifest's `safe_horizon` will stay
                // pinned at their minimum `time_min` — incremental
                // emit stalls until the count-based completion path
                // fires.
                if maybe_emit_head(
                    &mut emit_order,
                    &mut current_stores,
                    &mut ready_pending,
                    &mut completed_segments,
                    &projected_schema,
                    &output_channel,
                    &mut rows_sent,
                    limit_rows,
                )
                .await?
                {
                    return Ok(());
                }
            }
            CpuWorkerMsg::Chunks((segment_id, chunks)) => {
                if chunks.is_empty() {
                    continue;
                }
                if completed_segments.contains(&segment_id)
                    || ready_pending.contains_key(&segment_id)
                {
                    tracing::warn!(
                        %segment_id,
                        n = chunks.len(),
                        "received Chunks for already-completed segment; dropping. \
                         Server over-reported chunks past the announced count."
                    );
                    continue;
                }
                if index_values
                    .as_ref()
                    .is_some_and(|iv| !iv.contains_key(&segment_id))
                {
                    continue;
                }

                let n_chunks = chunks.len();
                let stores = match current_stores.entry(segment_id.clone()) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        emit_order.push_back(segment_id.clone());
                        e.insert(CurrentStores::new(
                            segment_id.clone(),
                            &query_expression,
                            &index_values,
                            pipeline_budget.clone(),
                        ))
                    }
                };

                // Scoped so both `_insert_span` and the puffin
                // `profile_scope!` drop before the `flush().await`
                // below — the puffin `ProfilerScope` is `!Send` and
                // would otherwise poison the surrounding future.
                {
                    let _insert_span = tracing::debug_span!(
                        "insert_chunks",
                        segment_id = %stores.segment_id,
                        n = n_chunks,
                    )
                    .entered();
                    re_tracing::profile_scope!("insert_chunks");
                    for chunk in chunks {
                        // Insert first, then record against the
                        // manifest only on success. If insert fails
                        // the whole worker propagates the error and
                        // the query stream short-circuits, but
                        // recording before insert would briefly leave
                        // the manifest claiming a chunk arrived that
                        // the store doesn't actually hold.
                        let chunk = Arc::new(chunk);
                        stores.store.write().insert_chunk(&chunk).map_err(|err| {
                            ApiError::internal_with_source(
                                None,
                                err,
                                "inserting chunk into in-memory store",
                            )
                        })?;
                        stores.record_arrival(&chunk);
                    }
                    stores.received_chunks += n_chunks;
                }

                let complete = stores.is_complete();
                if complete {
                    stores.release_segment_slot();
                    let taken = current_stores
                        .remove(&segment_id)
                        .expect("just inserted via entry()");
                    ready_pending.insert(segment_id, taken);
                }

                // Run the head-of-line emit pass on every chunk
                // arrival — incremental emit on the head segment
                // depends on it. `maybe_emit_head` is cheap when the
                // head's horizon hasn't advanced.
                if maybe_emit_head(
                    &mut emit_order,
                    &mut current_stores,
                    &mut ready_pending,
                    &mut completed_segments,
                    &projected_schema,
                    &output_channel,
                    &mut rows_sent,
                    limit_rows,
                )
                .await?
                {
                    return Ok(());
                }
            }
        }
    }

    // End-of-stream. Walk `emit_order` to preserve segment_id-ASC
    // order. Two cases per segment:
    //
    // 1. Segment is in `ready_pending` → fully complete, drain via
    //    `flush`. `Drop for CurrentStores` refunds the budget after
    //    (or on `?` short-circuit).
    //
    // 2. Segment is still in `current_stores` → IO erred partway,
    //    the count announcement never arrived, or fewer chunks
    //    arrived than promised. **Do not** flush these: a partial-
    //    phase flush under latest-at semantics emits rows that look
    //    complete (non-null) but actually contain carry-forward
    //    values whose downstream chunks never arrived. Log and
    //    drop the segment; `Drop for CurrentStores` refunds the
    //    budget reservation. Downstream sees fewer rows than
    //    expected — a loud failure mode that's recoverable —
    //    instead of silently corrupted rows.
    //
    // Severity for case 2 depends on *why* EOS happened:
    //
    // * Consumer hung up (output channel closed) → the stream was
    //   cancelled cleanly by the downstream operator; the IO loop
    //   responded by closing its sender. Incomplete segments are
    //   expected, so log at `debug!` to avoid noise.
    // * Output still open → IO closed its sender on its own
    //   (unexpected on a successful query path). Log at `warn!` so
    //   the protocol mismatch surfaces.
    //
    // (IO-side `Err`s never reach this block: `match msg?` in the
    // loop above propagates them out of the function before
    // end-of-stream is reached.)
    let consumer_cancelled = output_channel.is_closed();
    while let Some(seg) = emit_order.pop_front() {
        if let Some(stores) = ready_pending.remove(&seg) {
            stores
                .flush(
                    &projected_schema,
                    &output_channel,
                    &mut rows_sent,
                    limit_rows,
                )
                .await?;
            if limit_rows.is_some_and(|l| rows_sent >= l) {
                return Ok(());
            }
        } else if let Some(stores) = current_stores.remove(&seg) {
            let received = stores.received_chunks;
            let expected = stores
                .expected_chunks
                .map_or_else(|| "?".to_owned(), |n| n.to_string());
            if consumer_cancelled {
                re_log::debug!(
                    "abandoning incomplete segment {seg} after consumer cancellation: \
                     received {received}/{expected} announced chunks",
                );
            } else {
                re_log::warn!(
                    "abandoning incomplete segment {seg} at end-of-stream: \
                     received {received}/{expected} announced chunks; partial rows \
                     would risk emitting carry-forward values whose source chunks \
                     never arrived",
                );
            }
            // `Drop for CurrentStores` refunds the budget bytes.
            drop(stores);
        }
    }

    Ok(())
}

/// Drive emit progress from the head of `emit_order`.
///
/// On each iteration:
/// 1. If the head segment is in `ready_pending`, it is fully complete:
///    pop it, run final [`CurrentStores::flush`], and loop to expose
///    whatever segment is now at the front.
/// 2. Otherwise the head is still in flight in `current_stores`:
///    run [`CurrentStores::flush_incremental`] on it (a no-op when
///    the segment has no manifest or its horizon hasn't advanced),
///    then return — the head is not ready to retire yet, and
///    non-head segments may not emit out of order.
/// 3. If the head isn't tracked anywhere, leave `emit_order` alone
///    and return.
///
/// Returns `Ok(true)` if the row limit was hit and the caller should
/// short-circuit; `Ok(false)` otherwise.
async fn maybe_emit_head(
    emit_order: &mut VecDeque<SegmentId>,
    current_stores: &mut HashMap<SegmentId, CurrentStores>,
    ready_pending: &mut HashMap<SegmentId, CurrentStores>,
    completed_segments: &mut HashSet<SegmentId>,
    projected_schema: &Arc<Schema>,
    output_channel: &Sender<RecordBatch>,
    rows_sent: &mut usize,
    limit_rows: Option<usize>,
) -> ApiResult<bool> {
    loop {
        let Some(head) = emit_order.front().cloned() else {
            return Ok(false);
        };

        if let Some(stores) = ready_pending.remove(&head) {
            emit_order.pop_front().expect("front existed");
            completed_segments.insert(head);
            stores
                .flush(projected_schema, output_channel, rows_sent, limit_rows)
                .await?;
            if limit_rows.is_some_and(|l| *rows_sent >= l) {
                return Ok(true);
            }
            continue;
        }

        if let Some(stores) = current_stores.get_mut(&head) {
            stores
                .flush_incremental(projected_schema, output_channel, rows_sent, limit_rows)
                .await?;
            if limit_rows.is_some_and(|l| *rows_sent >= l) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
}

#[cfg(test)]
mod tests {
    use re_dataframe::external::re_chunk::Chunk;

    use super::*;

    /// `Drop for CurrentStores` returns the in-store bytes to the budget.
    /// Covers every exit path uniformly: `flush` success, worker `?`
    /// early-return on an upstream error, consumer hangup mid-segment,
    /// panic. Without the refund, the reservation would be pinned for
    /// the remainder of the query.
    #[tokio::test]
    async fn test_completed_segment_releases_gate_before_flush() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));

        let ready_segment = SegmentId::from("ready-pending");
        budget
            .reserve_with_priority(1, TimeInt::MAX, &[ready_segment.as_ref().to_owned()])
            .await;
        for i in 1..crate::pipeline_budget::MAX_CONCURRENT_SEGMENTS {
            budget
                .reserve_with_priority(1, TimeInt::MAX, &[format!("held-{i}")])
                .await;
        }

        let b = Arc::clone(&budget);
        let parked = tokio::spawn(async move {
            b.reserve_with_priority(1, TimeInt::MAX, &["new-segment".to_owned()])
                .await;
        });
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        assert!(!parked.is_finished());

        let mut stores = CurrentStores::new(
            ready_segment,
            &QueryExpression::default(),
            &None,
            Arc::clone(&budget),
        );
        stores.release_segment_slot();

        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        assert!(parked.is_finished());
        parked.await.unwrap();
    }

    #[tokio::test]
    async fn test_current_stores_drop_refunds_budget() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        let releases_before = budget.total_releases();

        {
            let _stores = CurrentStores::new(
                SegmentId::from("drop-refund-test"),
                &QueryExpression::default(),
                &None,
                budget.clone(),
            );
            // Dropping should invoke `release` exactly once.
        }

        assert_eq!(
            budget.total_releases(),
            releases_before + 1,
            "Drop must call release exactly once",
        );
    }

    /// `is_complete` flips only once both the IO-side count announcement
    /// and the chunk inserts have arrived. The receive order doesn't
    /// matter — the CPU worker tolerates a `SegmentChunkCount` that
    /// arrives after the last chunk has already been inserted.
    #[tokio::test]
    async fn test_current_stores_is_complete_gates_on_expected_chunks() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        let mut stores = CurrentStores::new(
            SegmentId::from("complete-gate-test"),
            &QueryExpression::default(),
            &None,
            budget.clone(),
        );

        // Nothing announced, nothing received → never complete.
        assert!(!stores.is_complete());

        // Chunks received but no announcement → still not complete,
        // because we don't yet know how many to expect.
        stores.received_chunks = 5;
        assert!(!stores.is_complete());

        // Announcement arrives matching the received count → complete.
        stores.expected_chunks = Some(5);
        assert!(stores.is_complete());

        // Announcement matches a count we haven't yet reached → not
        // complete; the missing chunks will land later.
        let mut other = CurrentStores::new(
            SegmentId::from("complete-gate-other"),
            &QueryExpression::default(),
            &None,
            budget.clone(),
        );
        other.expected_chunks = Some(10);
        other.received_chunks = 3;
        assert!(!other.is_complete());
    }

    /// Helper: build an empty `Chunk` for tests that only need to count
    /// arrivals through the worker (not exercise row emission).
    /// `insert_chunk` treats an empty chunk as a no-op (see
    /// `re_chunk_store::writes::insert_chunk`), so the per-segment
    /// `received_chunks` counter still advances and the worker's
    /// completion logic can be exercised without building real rows.
    fn empty_chunk() -> Chunk {
        use re_dataframe::external::re_chunk::Chunk as ChunkBuilder;
        use re_log_types::EntityPath;
        ChunkBuilder::builder(EntityPath::root()).build().unwrap()
    }

    /// Drive the worker concurrently with a background drainer of the
    /// output channel. Returns the worker's result plus the number of
    /// `RecordBatch`es emitted. Tests use this when they care about
    /// worker termination and crash-freedom, not specific output rows
    /// (the empty-chunk fixtures don't produce deterministic output
    /// since the underlying `QueryHandle` against an empty store can
    /// still emit a row depending on the projection).
    async fn drive_worker(
        input_rx: Receiver<ApiResult<CpuWorkerMsg>>,
        output_tx: Sender<RecordBatch>,
        mut output_rx: Receiver<RecordBatch>,
    ) -> (ApiResult<()>, usize) {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 4));
        let schema = Arc::new(Schema::empty());
        let worker = chunk_store_cpu_worker_thread(
            input_rx,
            output_tx,
            QueryExpression::default(),
            schema,
            None,
            None,
            budget,
        );
        let drainer = async {
            let mut n = 0;
            while output_rx.recv().await.is_some() {
                n += 1;
            }
            n
        };
        tokio::join!(worker, drainer)
    }

    /// Drive `chunk_store_cpu_worker_thread` with two segments whose
    /// chunks interleave on the input channel. Both must complete (and
    /// the worker must terminate cleanly) — this exercises the
    /// `HashMap<SegmentId, CurrentStores>` routing that replaced the
    /// old reorder-buffer + one-segment-at-a-time topology.
    #[tokio::test]
    async fn test_cpu_worker_handles_interleaved_segments() {
        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<ApiResult<CpuWorkerMsg>>(16);
        let (output_tx, output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(16);

        // Announce both segments first, then interleave their chunks.
        input_tx
            .send(Ok(CpuWorkerMsg::SegmentChunkCount {
                segment_id: SegmentId::from("A"),
                count: 2,
            }))
            .await
            .unwrap();
        input_tx
            .send(Ok(CpuWorkerMsg::SegmentChunkCount {
                segment_id: SegmentId::from("B"),
                count: 2,
            }))
            .await
            .unwrap();
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("A"),
                vec![empty_chunk()],
            ))))
            .await
            .unwrap();
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("B"),
                vec![empty_chunk()],
            ))))
            .await
            .unwrap();
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("A"),
                vec![empty_chunk()],
            ))))
            .await
            .unwrap(); // A complete
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("B"),
                vec![empty_chunk()],
            ))))
            .await
            .unwrap(); // B complete
        drop(input_tx);

        let (result, _n_batches) = drive_worker(input_rx, output_tx, output_rx).await;
        assert!(result.is_ok(), "worker must complete cleanly: {result:?}");
    }

    /// If more `Chunks` arrive for a segment than its announced
    /// `SegmentChunkCount`, the over-count batch must be logged and
    /// dropped — never panic, never re-flush the segment.
    #[tokio::test]
    async fn test_cpu_worker_drops_chunks_for_already_completed_segment() {
        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<ApiResult<CpuWorkerMsg>>(8);
        let (output_tx, output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(8);

        input_tx
            .send(Ok(CpuWorkerMsg::SegmentChunkCount {
                segment_id: SegmentId::from("A"),
                count: 1,
            }))
            .await
            .unwrap();
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("A"),
                vec![empty_chunk()],
            ))))
            .await
            .unwrap(); // A completes here
        // Over-count: server (hypothetically) over-reported, sends one
        // more chunk for A after the announced count was satisfied.
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("A"),
                vec![empty_chunk()],
            ))))
            .await
            .unwrap();
        // A duplicate SegmentChunkCount for the same segment must also
        // be dropped, not blow up the worker.
        input_tx
            .send(Ok(CpuWorkerMsg::SegmentChunkCount {
                segment_id: SegmentId::from("A"),
                count: 99,
            }))
            .await
            .unwrap();
        drop(input_tx);

        let (result, _n_batches) = drive_worker(input_rx, output_tx, output_rx).await;
        assert!(
            result.is_ok(),
            "worker must drop over-count chunks and a duplicate SegmentChunkCount without erroring: {result:?}"
        );
    }

    /// Head-of-line gate: when a later-announced segment completes
    /// before an earlier one, it must wait in `ready_pending` rather
    /// than emit out of order. Once the head completes, the cascade
    /// drains everything contiguously ready from the front of
    /// `emit_order`. This is the invariant that lets the worker honor
    /// the plan's `[segment_id ASC, …]` ordering claim.
    #[tokio::test]
    async fn test_maybe_emit_head_preserves_announcement_order() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 4));
        let schema = Arc::new(Schema::empty());
        let (output_tx, _output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(8);

        let mut emit_order: VecDeque<SegmentId> = VecDeque::new();
        emit_order.push_back(SegmentId::from("A"));
        emit_order.push_back(SegmentId::from("B"));
        emit_order.push_back(SegmentId::from("C"));

        let mut current_stores: HashMap<SegmentId, CurrentStores> = HashMap::new();
        let mut ready_pending: HashMap<SegmentId, CurrentStores> = HashMap::new();
        let mut completed_segments: HashSet<SegmentId> = HashSet::new();
        let mut rows_sent: usize = 0;

        // B and C complete first; A still in flight. Drain must be a no-op.
        ready_pending.insert(
            SegmentId::from("B"),
            CurrentStores::new(
                SegmentId::from("B"),
                &QueryExpression::default(),
                &None,
                budget.clone(),
            ),
        );
        ready_pending.insert(
            SegmentId::from("C"),
            CurrentStores::new(
                SegmentId::from("C"),
                &QueryExpression::default(),
                &None,
                budget.clone(),
            ),
        );
        let hit_limit = maybe_emit_head(
            &mut emit_order,
            &mut current_stores,
            &mut ready_pending,
            &mut completed_segments,
            &schema,
            &output_tx,
            &mut rows_sent,
            None,
        )
        .await
        .unwrap();
        assert!(!hit_limit);
        assert_eq!(
            emit_order.len(),
            3,
            "head A blocks B and C from draining: {emit_order:?}"
        );
        assert_eq!(ready_pending.len(), 2);
        assert!(completed_segments.is_empty());

        // A completes — cascade drains A, then B, then C in one call.
        ready_pending.insert(
            SegmentId::from("A"),
            CurrentStores::new(
                SegmentId::from("A"),
                &QueryExpression::default(),
                &None,
                budget.clone(),
            ),
        );
        let hit_limit = maybe_emit_head(
            &mut emit_order,
            &mut current_stores,
            &mut ready_pending,
            &mut completed_segments,
            &schema,
            &output_tx,
            &mut rows_sent,
            None,
        )
        .await
        .unwrap();
        assert!(!hit_limit);
        assert!(emit_order.is_empty(), "all three drained: {emit_order:?}");
        assert!(ready_pending.is_empty());
        assert_eq!(completed_segments.len(), 3);
    }

    /// `SegmentChunkCount` arriving *after* all chunks for the segment
    /// have already been inserted must still trigger the flush. This
    /// covers the channel-reordering edge case the worker's
    /// `is_complete()` branch on the count-announcement path is for.
    #[tokio::test]
    async fn test_cpu_worker_flushes_when_count_arrives_after_chunks() {
        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<ApiResult<CpuWorkerMsg>>(8);
        let (output_tx, output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(8);

        // Chunks first, count last.
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("A"),
                vec![empty_chunk()],
            ))))
            .await
            .unwrap();
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("A"),
                vec![empty_chunk()],
            ))))
            .await
            .unwrap();
        input_tx
            .send(Ok(CpuWorkerMsg::SegmentChunkCount {
                segment_id: SegmentId::from("A"),
                count: 2,
            }))
            .await
            .unwrap();
        drop(input_tx);

        let (result, _n_batches) = drive_worker(input_rx, output_tx, output_rx).await;
        assert!(
            result.is_ok(),
            "worker must tolerate count after chunks: {result:?}"
        );
    }

    // ----------------------------------------------------------------
    // gc_up_to_horizon + flush_incremental fast-skip tests
    // ----------------------------------------------------------------

    /// Build a single-row temporal chunk on `timeline_name` at `time`
    /// carrying one `MyLabel` component, so the chunk has non-zero
    /// stored bytes and the store's `latest_at` machinery has a
    /// component to find.
    fn temporal_chunk(entity: &str, timeline_name: &'static str, time: i64) -> Chunk {
        use re_dataframe::external::re_chunk::RowId;
        use re_log_types::Timeline;
        use re_log_types::example_components::{MyLabel, MyPoints};
        let timepoint = [(Timeline::new_sequence(timeline_name), time)];
        let labels = &[MyLabel(format!("{entity}@{time}"))];
        Chunk::builder(entity)
            .with_component_batches(
                RowId::new(),
                timepoint,
                [(MyPoints::descriptor_labels(), labels as _)],
            )
            .build()
            .unwrap()
    }

    /// Build a `CurrentStores` wired up with `filtered_index = Some(timeline)`
    /// and a locked, empty manifest (so `safe_horizon` returns `MAX`
    /// but the call paths still see "manifest present, timeline set").
    /// Tests then poke `manifest`/`processed_through_time` directly to
    /// drive specific paths.
    fn stores_with_timeline(
        segment_id: &str,
        timeline_name: &str,
        budget: Arc<PipelineBudget>,
    ) -> CurrentStores {
        let query_expression = QueryExpression {
            filtered_index: Some(TimelineName::try_new(timeline_name).unwrap()),
            ..Default::default()
        };
        let mut stores = CurrentStores::new(
            SegmentId::from(segment_id),
            &query_expression,
            &None,
            budget,
        );
        let mut manifest = SegmentChunkManifest::new();
        manifest.lock();
        stores.manifest = Some(manifest);
        stores
    }

    /// All store contents are required to carry the latest-at value at
    /// the horizon → GC must drop nothing and the budget must not be
    /// touched. Exercises the `protected_chunks` path plus the
    /// `protected_time_ranges = (horizon+1, MAX]` path together.
    ///
    /// Setup:
    /// - `/a` has its only chunk at `t=10` → latest-at at `t=50` for
    ///   `/a` is `/a@10`, protected via `protected_chunks`.
    /// - `/b` has chunks at `t=20` (latest-at carry-forward at `t=50`,
    ///   protected via `protected_chunks`) and `t=100` (sits past
    ///   horizon, protected via `protected_time_ranges`).
    #[test]
    fn test_gc_up_to_horizon_preserves_carry_forward() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        let mut stores = stores_with_timeline("seg", "frame", budget.clone());
        for chunk in [
            temporal_chunk("/a", "frame", 10),
            temporal_chunk("/b", "frame", 20),
            temporal_chunk("/b", "frame", 100),
        ] {
            stores.store.write().insert_chunk(&Arc::new(chunk)).unwrap();
        }

        let bytes_before = stores.store_bytes();
        let releases_before = budget.total_releases();
        let n_chunks_before = stores.store.read().num_physical_chunks();
        assert_eq!(n_chunks_before, 3);

        stores.gc_up_to_horizon(TimeInt::new_temporal(50));

        let bytes_after = stores.store_bytes();
        let n_chunks_after = stores.store.read().num_physical_chunks();
        assert_eq!(
            n_chunks_after, 3,
            "carry-forward chunks must survive GC under latest-at semantics",
        );
        assert_eq!(
            bytes_after, bytes_before,
            "no bytes freed when every chunk is protected",
        );
        assert_eq!(
            budget.total_releases(),
            releases_before,
            "release must not fire when freed == 0",
        );
    }

    /// Superseded chunks (older than the latest-at value at the horizon
    /// for their entity) are not protected and must be GC'd. Budget
    /// gets the freed bytes back.
    ///
    /// Setup:
    /// - `/a` has chunks at `t=10, 20, 30`. Horizon=50 → latest-at for
    ///   `/a` is `/a@30` (protected). `/a@10` and `/a@20` are
    ///   superseded → GC'd.
    #[test]
    fn test_gc_up_to_horizon_drops_superseded_chunks() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        let mut stores = stores_with_timeline("seg", "frame", budget.clone());
        for chunk in [
            temporal_chunk("/a", "frame", 10),
            temporal_chunk("/a", "frame", 20),
            temporal_chunk("/a", "frame", 30),
        ] {
            stores.store.write().insert_chunk(&Arc::new(chunk)).unwrap();
        }

        let bytes_before = stores.store_bytes();
        let releases_before = budget.total_releases();
        assert_eq!(stores.store.read().num_physical_chunks(), 3);

        stores.gc_up_to_horizon(TimeInt::new_temporal(50));

        let bytes_after = stores.store_bytes();
        assert_eq!(
            stores.store.read().num_physical_chunks(),
            1,
            "only the latest-at chunk (@30) must remain",
        );
        assert!(
            bytes_after < bytes_before,
            "GC must free bytes when chunks are dropped (before={bytes_before}, after={bytes_after})",
        );
        assert_eq!(
            budget.total_releases(),
            releases_before + 1,
            "release must fire exactly once when freed > 0",
        );
    }

    /// Across multiple `gc_up_to_horizon` calls, the
    /// `protected_chunks_scratch` `HashSet` must retain its allocated
    /// capacity (via `clear()` + `mem::swap` back from the
    /// `GarbageCollectionOptions`). Verifies the scratch reuse contract
    /// — a future refactor that re-allocates per call would silently
    /// regress the IO→CPU hot path.
    #[test]
    fn test_gc_up_to_horizon_reuses_scratch_capacity() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        let mut stores = stores_with_timeline("seg", "frame", budget);
        // Populate enough entities that `protected_chunks_scratch`
        // grabs a non-trivial capacity on the first call.
        for i in 0..64 {
            stores
                .store
                .write()
                .insert_chunk(&Arc::new(temporal_chunk(&format!("/e{i}"), "frame", 0)))
                .unwrap();
        }
        assert_eq!(
            stores.protected_chunks_scratch.capacity(),
            0,
            "fresh CurrentStores starts with zero scratch capacity",
        );

        stores.gc_up_to_horizon(TimeInt::new_temporal(50));
        let cap_after_first = stores.protected_chunks_scratch.capacity();
        assert!(
            cap_after_first > 0,
            "scratch must retain capacity after gc (got {cap_after_first})",
        );
        // gc returned; the swap-back restores ownership but clear()
        // happens at the *start* of the next call, so the set may
        // still hold the inserted IDs here. The contract under test
        // is capacity, not size.

        stores.gc_up_to_horizon(TimeInt::new_temporal(60));
        assert!(
            stores.protected_chunks_scratch.capacity() >= cap_after_first,
            "capacity must not shrink across calls (before={cap_after_first}, after={})",
            stores.protected_chunks_scratch.capacity(),
        );
    }

    /// Without a temporal `filtered_index` (`filtered_index_timeline ==
    /// None`), `gc_up_to_horizon` must short-circuit. The static-only
    /// query path otherwise has no timeline to feed to
    /// `LatestAtQuery::new`.
    #[test]
    fn test_gc_up_to_horizon_noop_without_filtered_index() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        // Construct without a filtered_index timeline.
        let mut stores = CurrentStores::new(
            SegmentId::from("seg"),
            &QueryExpression::default(),
            &None,
            budget.clone(),
        );
        let mut manifest = SegmentChunkManifest::new();
        manifest.lock();
        stores.manifest = Some(manifest);
        assert!(stores.filtered_index_timeline.is_none());

        stores
            .store
            .write()
            .insert_chunk(&Arc::new(temporal_chunk("/a", "frame", 10)))
            .unwrap();
        let bytes_before = stores.store_bytes();
        let releases_before = budget.total_releases();

        stores.gc_up_to_horizon(TimeInt::new_temporal(50));

        assert_eq!(
            stores.store_bytes(),
            bytes_before,
            "no-op leaves bytes unchanged"
        );
        assert_eq!(budget.total_releases(), releases_before, "no release call");
    }

    /// `flush_incremental` with no manifest must short-circuit at the
    /// first guard — never touch the output channel. This is the
    /// fallback path for queries without a temporal `filtered_index`
    /// (where `SegmentManifest` was never sent).
    #[tokio::test]
    async fn test_flush_incremental_fast_skip_without_manifest() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        let mut stores = CurrentStores::new(
            SegmentId::from("seg"),
            &QueryExpression::default(),
            &None,
            budget,
        );
        assert!(stores.manifest.is_none());

        let schema = Arc::new(Schema::empty());
        let (output_tx, mut output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(4);
        let mut rows_sent = 0usize;

        stores
            .flush_incremental(&schema, &output_tx, &mut rows_sent, None)
            .await
            .unwrap();

        assert_eq!(rows_sent, 0);
        // No message must have been queued.
        assert!(output_rx.try_recv().is_err());
    }

    /// `flush_incremental` with a manifest whose `safe_horizon` hasn't
    /// advanced past `processed_through_time` must also fast-skip
    /// without invoking the emit path. Exercises the "horizon
    /// unchanged" branch separately from "no manifest".
    #[tokio::test]
    async fn test_flush_incremental_fast_skip_when_horizon_not_advanced() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        let mut stores = stores_with_timeline("seg", "frame", budget);
        // Locked + empty manifest → safe_horizon = Some(MAX). Pretend
        // we already processed up to MAX so the fast-skip branch fires.
        stores.processed_through_time = Some(TimeInt::MAX);

        let schema = Arc::new(Schema::empty());
        let (output_tx, mut output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(4);
        let mut rows_sent = 0usize;

        stores
            .flush_incremental(&schema, &output_tx, &mut rows_sent, None)
            .await
            .unwrap();

        assert_eq!(rows_sent, 0);
        assert!(output_rx.try_recv().is_err());
    }

    /// Path 1b: no arrived chunk has rows past `processed_through_time`
    /// (the manifest is locked but no chunks have been inserted yet —
    /// the typical `SegmentManifest` handler entry state). The horizon
    /// emit must be skipped (no `QueryHandle` build), but
    /// `processed_through_time` must advance to the new horizon so
    /// subsequent ticks short-circuit on the cheaper path 1 fast skip
    /// instead of re-running this check.
    #[tokio::test]
    async fn test_flush_incremental_fast_skip_when_no_arrivals_in_range() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        let mut stores = stores_with_timeline("seg", "frame", budget);
        // Locked + empty manifest → safe_horizon = Some(MAX).
        // max_arrived_time_max is None because no chunks have arrived.
        assert!(stores.max_arrived_time_max.is_none());
        assert!(stores.processed_through_time.is_none());

        let schema = Arc::new(Schema::empty());
        let (output_tx, mut output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(4);
        let mut rows_sent = 0usize;

        stores
            .flush_incremental(&schema, &output_tx, &mut rows_sent, None)
            .await
            .unwrap();

        assert_eq!(rows_sent, 0);
        assert!(output_rx.try_recv().is_err());
        assert_eq!(
            stores.processed_through_time,
            Some(TimeInt::MAX),
            "path 1b must advance processed_through_time to the new horizon",
        );
    }

    // ----------------------------------------------------------------
    // gc_up_to_horizon scaling microbench
    // ----------------------------------------------------------------

    /// Measures per-call cost of [`CurrentStores::gc_up_to_horizon`] as
    /// the number of entities and chunks scales up. Run via:
    ///
    /// ```text
    /// cargo nextest run -p re_datafusion --all-features \
    ///   --run-ignored only --no-capture bench_gc_up_to_horizon_scaling
    /// ```
    ///
    /// `flush_incremental` calls `gc_up_to_horizon` on every chunk
    /// arrival once the safe horizon has moved, so the per-call cost
    /// flows directly into the IO → CPU hot path. Output lets the
    /// reviewer confirm the cost scales acceptably with entity count
    /// before this PR lands.
    ///
    /// Setup keeps GC work concentrated in `protected_chunks`
    /// construction (the suspected bottleneck): every entity has one
    /// chunk at `t=0` and the advancing horizon never drops anything,
    /// so each call iterates `all_entities()` and runs
    /// `latest_at_relevant_chunks_for_all_components` per entity, then
    /// performs an empty GC pass.
    #[test]
    #[ignore = "microbench — run explicitly with --run-ignored only --no-capture"]
    fn bench_gc_up_to_horizon_scaling() {
        use std::time::Instant;

        for n_entities in [100, 500] {
            for m_chunks_per_entity in [1_000, 2_000, 3_000, 4_000, 5_000] {
                let budget = Arc::new(PipelineBudget::new(1 << 40, 1));
                let mut stores = stores_with_timeline("seg", "frame", budget);

                let setup_start = Instant::now();
                for i in 0..n_entities {
                    let entity = format!("/e{i}");
                    for j in 0..m_chunks_per_entity {
                        // Stagger times so each entity has m distinct
                        // chunks, but keep them all well below the
                        // advancing horizon used below.
                        let t = j as i64;
                        stores
                            .store
                            .write()
                            .insert_chunk(&Arc::new(temporal_chunk(&entity, "frame", t)))
                            .unwrap();
                    }
                }
                let setup = setup_start.elapsed();

                // Warm-up call — first GC may pay one-off lazy-index
                // costs in the chunk store that we don't want to fold
                // into the per-call number.
                stores.gc_up_to_horizon(TimeInt::new_temporal(1_000));

                let n_calls: u32 = 20;
                let bench_start = Instant::now();
                for k in 0..n_calls {
                    // Horizon advances each iteration so the fast-skip
                    // path inside `flush_incremental` would never fire
                    // upstream of `gc_up_to_horizon` either.
                    let h = 1_000 + i64::from(k);
                    stores.gc_up_to_horizon(TimeInt::new_temporal(h));
                }
                let elapsed = bench_start.elapsed();
                let per_call = elapsed / n_calls;

                println!(
                    "n_entities={n_entities:>6}  chunks_per_entity={m_chunks_per_entity:>2}  \
                     total_chunks={total:>7}  setup={setup:?}  per_gc_call={per_call:?}  \
                     total_bench={elapsed:?}",
                    total = n_entities * m_chunks_per_entity,
                );
            }
        }
    }

    // ----------------------------------------------------------------
    // Divergence + end-of-stream abandon tests
    // ----------------------------------------------------------------

    /// `CurrentStores::record_arrival` on a chunk whose `(entity,
    /// time_min)` pair was never announced in the manifest must trip
    /// `debug_panic!` in debug builds. The `should_panic` predicate
    /// pins the message so a future rewording of the divergence
    /// branch can't quietly skip this contract.
    ///
    /// Release builds: `debug_panic!` degrades to no-op and `record_arrival`
    /// returns without panicking — the chunk insert continues at the
    /// call site (this test only covers the debug contract).
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "manifest/chunk divergence")]
    fn test_record_arrival_divergent_chunk_debug_panics() {
        use re_log_types::EntityPath;
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        let mut stores = CurrentStores::new(
            SegmentId::from("seg"),
            &QueryExpression {
                filtered_index: Some(TimelineName::from("frame")),
                ..Default::default()
            },
            &None,
            budget,
        );
        // Manifest expects (/a, time=10) only.
        let mut manifest = SegmentChunkManifest::new();
        manifest.expect_chunk(EntityPath::from("/a"), TimeInt::new_temporal(10));
        manifest.lock();
        stores.manifest = Some(manifest);

        // Divergent chunk: same entity but a `time_min` never announced.
        let chunk = temporal_chunk("/a", "frame", 20);
        stores.record_arrival(&chunk);
    }

    /// `record_arrival` must track `max_arrived_time_max` whenever the
    /// chunk is temporal on the filtered timeline — even before any
    /// manifest is attached. Otherwise `flush_incremental`'s path-1b
    /// fast-skip (which guards on `max_arrived_time_max`) would later
    /// treat the segment as "no arrivals in range" and advance
    /// `processed_through_time` past rows that are actually emittable.
    ///
    /// Drives the documented latent edge case where chunks arrive
    /// before the `SegmentManifest` message (against the current IO-
    /// loop ordering, but defended against in `record_arrival`'s
    /// guard split).
    #[test]
    fn test_record_arrival_tracks_time_max_without_manifest() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 1));
        let mut stores = CurrentStores::new(
            SegmentId::from("seg"),
            &QueryExpression {
                filtered_index: Some(TimelineName::from("frame")),
                ..Default::default()
            },
            &None,
            budget,
        );
        assert!(stores.manifest.is_none());
        assert!(stores.max_arrived_time_max.is_none());

        stores.record_arrival(&temporal_chunk("/a", "frame", 10));
        stores.record_arrival(&temporal_chunk("/b", "frame", 30));
        stores.record_arrival(&temporal_chunk("/c", "frame", 20));

        assert_eq!(
            stores.max_arrived_time_max,
            Some(TimeInt::new_temporal(30)),
            "max_arrived_time_max must reflect pre-manifest temporal arrivals",
        );
    }

    /// Variant of [`drive_worker`] that takes the budget from the
    /// caller, so a test can assert reservation accounting after the
    /// worker terminates. Mirrors the regular helper otherwise.
    async fn drive_worker_with_budget(
        budget: Arc<PipelineBudget>,
        input_rx: Receiver<ApiResult<CpuWorkerMsg>>,
        output_tx: Sender<RecordBatch>,
        mut output_rx: Receiver<RecordBatch>,
    ) -> (ApiResult<()>, usize) {
        let schema = Arc::new(Schema::empty());
        let worker = chunk_store_cpu_worker_thread(
            input_rx,
            output_tx,
            QueryExpression::default(),
            schema,
            None,
            None,
            budget,
        );
        let drainer = async {
            let mut n = 0;
            while output_rx.recv().await.is_some() {
                n += 1;
            }
            n
        };
        tokio::join!(worker, drainer)
    }

    /// End-of-stream with a segment whose announced chunk count was
    /// never satisfied: the worker must terminate cleanly (no error,
    /// no hang), the partial segment must NOT be flushed (partial
    /// rows would emit carry-forward values whose source chunks
    /// never arrived under latest-at semantics), and any bytes the
    /// store accumulated must be refunded to the pipeline budget via
    /// `Drop for CurrentStores`.
    #[tokio::test]
    async fn test_cpu_worker_abandons_incomplete_segment_on_eos() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 4));
        let releases_before = budget.total_releases();

        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<ApiResult<CpuWorkerMsg>>(8);
        let (output_tx, output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(8);

        // Announce 5 chunks, deliver 2, then close the input channel.
        // Worker reaches end-of-stream with `current_stores` still
        // holding "incomplete" → warn + drop path.
        input_tx
            .send(Ok(CpuWorkerMsg::SegmentChunkCount {
                segment_id: SegmentId::from("A"),
                count: 5,
            }))
            .await
            .unwrap();
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("A"),
                vec![empty_chunk(), empty_chunk()],
            ))))
            .await
            .unwrap();
        drop(input_tx);

        let (result, n_batches) =
            drive_worker_with_budget(budget.clone(), input_rx, output_tx, output_rx).await;

        result.expect("worker must terminate cleanly on incomplete segment EOS");
        assert_eq!(
            n_batches, 0,
            "incomplete segment must not be flushed at end-of-stream",
        );
        // `Drop for CurrentStores` unconditionally calls
        // `pipeline_budget.release(store_bytes)`; even when the
        // refunded amount is zero, the call still increments the
        // `total_releases` counter. Exactly one release call proves
        // that `Drop` ran once (no leak, no double-drop) for the
        // abandoned segment.
        assert_eq!(
            budget.total_releases(),
            releases_before + 1,
            "Drop for CurrentStores must run exactly once for the abandoned segment",
        );
    }

    /// Two `SegmentManifest` messages for the same segment is a
    /// protocol bug. In debug builds the worker must `debug_panic!`
    /// rather than silently overwrite the first manifest — overwriting
    /// would discard any `record_arrival` decrements already applied,
    /// inflate `outstanding_count` against the replacement, and pin
    /// `safe_horizon` until completion. `#[should_panic]` pins the
    /// message so a future rewording can't quietly skip this contract.
    #[tokio::test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "duplicate SegmentManifest")]
    async fn test_duplicate_segment_manifest_debug_panics() {
        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<ApiResult<CpuWorkerMsg>>(8);
        let (output_tx, output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(8);

        // Send two manifests back-to-back for the same segment.
        for _ in 0..2 {
            input_tx
                .send(Ok(CpuWorkerMsg::SegmentManifest {
                    segment_id: SegmentId::from("A"),
                    manifest: Box::new(SegmentChunkManifest::new()),
                }))
                .await
                .unwrap();
        }
        drop(input_tx);

        // Worker panics inside `tokio::join!`, so the contract is
        // captured by `#[should_panic]`. The tuple is intentionally
        // discarded; the `Result` element never resolves to `Err`
        // because the panic short-circuits the join.
        let (_result, _n_batches) = drive_worker(input_rx, output_tx, output_rx).await;
    }

    /// Variant of the incomplete-segment EOS test where the
    /// downstream consumer (output `Receiver`) is dropped *before*
    /// the worker terminates. The worker must still tear down
    /// cleanly and refund the budget; the abandon path takes the
    /// `consumer_cancelled` branch and logs at `debug!` rather than
    /// `warn!` so cancelled streams don't spam log lines. The actual
    /// severity downgrade isn't observable from outside the tracing
    /// subscriber, so this test pins the structural behavior the
    /// branch promises: clean termination, no batches emitted, one
    /// budget release.
    #[tokio::test]
    async fn test_cpu_worker_abandons_incomplete_segment_on_consumer_cancel() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 4));
        let releases_before = budget.total_releases();

        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<ApiResult<CpuWorkerMsg>>(8);
        let (output_tx, output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(8);

        input_tx
            .send(Ok(CpuWorkerMsg::SegmentChunkCount {
                segment_id: SegmentId::from("A"),
                count: 5,
            }))
            .await
            .unwrap();
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("A"),
                vec![empty_chunk()],
            ))))
            .await
            .unwrap();
        // Simulate consumer cancellation: drop the receiver so the
        // worker observes `output_channel.is_closed() == true` when
        // it reaches the EOS abandon block.
        drop(output_rx);
        drop(input_tx);

        let schema = Arc::new(Schema::empty());
        let result = chunk_store_cpu_worker_thread(
            input_rx,
            output_tx,
            QueryExpression::default(),
            schema,
            None,
            None,
            budget.clone(),
        )
        .await;

        result.expect("worker must terminate cleanly on consumer cancellation");
        assert_eq!(
            budget.total_releases(),
            releases_before + 1,
            "Drop for CurrentStores must run exactly once for the cancelled segment",
        );
    }

    /// In production `SegmentStreamExec::execute` spawns the worker on the
    /// process-wide CPU runtime and the stream holds its `JoinHandle`; when
    /// the stream is dropped mid-flight the handle is dropped too, which
    /// *detaches* the task rather than aborting it. Nothing joins or aborts
    /// the detached worker — it must self-terminate via channel closure and
    /// refund its budget reservation, without any runtime teardown.
    #[tokio::test]
    async fn test_detached_cpu_worker_winds_down_after_stream_drop() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 4));
        let releases_before = budget.total_releases();

        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<ApiResult<CpuWorkerMsg>>(8);
        let (output_tx, output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(8);

        // Announce more chunks than we deliver, so a reservation is held and
        // the segment never completes on its own.
        input_tx
            .send(Ok(CpuWorkerMsg::SegmentChunkCount {
                segment_id: SegmentId::from("A"),
                count: 5,
            }))
            .await
            .unwrap();
        input_tx
            .send(Ok(CpuWorkerMsg::Chunks((
                SegmentId::from("A"),
                vec![empty_chunk()],
            ))))
            .await
            .unwrap();

        // Spawn the worker as a detached task; dropping the `JoinHandle` matches `execute`'s detach behavior.
        let join = tokio::spawn(chunk_store_cpu_worker_thread(
            input_rx,
            output_tx,
            QueryExpression::default(),
            Arc::new(Schema::empty()),
            None,
            None,
            budget.clone(),
        ));

        // Simulate the stream being dropped mid-flight: the consumer hangs up,
        // the IO side finishes, and the join handle is dropped (detach, not
        // abort). None of this tears down the runtime the task runs on.
        drop(output_rx);
        drop(input_tx);
        drop(join);

        // The detached worker must wind down on its own and refund the budget.
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while budget.total_releases() == releases_before {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("detached worker must terminate and refund its budget after the stream is dropped");

        assert_eq!(
            budget.total_releases(),
            releases_before + 1,
            "exactly one budget release for the abandoned segment",
        );
    }
}
