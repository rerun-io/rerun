//! Per-segment in-memory store + horizon-driven emit + carry-forward-safe GC.
//!
//! This is the latest-at correctness core of the streaming dataset-query
//! pipeline, shared by the v1 CPU worker (wrapped in its `CurrentStores`,
//! which layers the pipeline-budget accounting and the per-entity manifest
//! on top) and by the upcoming v2 per-segment driver (`PIPELINE_V2.md`).
//!
//! By design this module knows nothing about *when* it is safe to emit —
//! callers supply the safe horizon (v1: `SegmentChunkManifest`; v2: plan
//! cursor) — and nothing about memory accounting: methods report byte deltas
//! back to the caller instead of talking to a budget.

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
use tokio::sync::mpsc::Sender;

use crate::dataframe_query_common::{
    DEFAULT_BATCH_BYTES, DEFAULT_BATCH_ROWS, IndexValuesMap, prepend_string_column_schema,
    schema_with_array_datatypes,
};

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
    origin: &re_uri::Origin,
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
            origin,
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
        let batch_schema = Arc::new(schema_with_array_datatypes(
            &prepend_string_column_schema(
                &query_schema,
                ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
            ),
            &columns,
        ));

        let batch = RecordBatch::try_new_with_options(
            batch_schema,
            columns,
            &RecordBatchOptions::default().with_row_count(Some(total_rows)),
        )
        .map_err(|err| {
            ApiError::deserialization_with_source(
                origin,
                None,
                err,
                "building output record batch from chunk-store rows",
            )
        })?;

        align_record_batch_to_schema(&batch, target_schema).map_err(|err| {
            ApiError::internal_with_source(origin, None, err, "DataFusion schema mismatch error")
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

    output_channel.send(output_batch).await.map_err(|err| {
        ApiError::internal_with_source(origin, None, err, "output channel closed")
    })?;

    Ok(Some(()))
}

/// Outcome of one [`SegmentStore::flush_incremental_to`] cycle, reported
/// back so the caller can drive its own accounting (v1: pipeline-budget
/// release + stall-detector notifies).
#[derive(Default)]
#[must_use]
pub struct IncrementalFlushOutcome {
    /// Rows emitted this cycle (0 on the fast-skip paths).
    pub rows_emitted: usize,

    /// Decoded bytes freed by the GC step this cycle.
    pub freed_bytes: u64,
}

/// Per-segment in-memory store, plus the state needed to incrementally emit
/// rows + GC chunks as the safe horizon advances.
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
pub struct SegmentStore {
    /// The server this query is running against, named in the errors we produce.
    origin: re_uri::Origin,

    pub segment_id: SegmentId,
    pub store: ChunkStoreHandle,

    /// Built once and reused across every `emit_up_to` cycle. The
    /// store + cache handles inside are live views, so post-construction
    /// chunk inserts and cache updates are visible without rebuilding.
    engine: QueryEngine<StorageEngine>,

    /// Per-segment specialization of the caller's base `QueryExpression`,
    /// with `using_index_values` already applied. `emit_up_to` clones
    /// and mutates `filtered_index_range` on each call.
    pub query_expression: QueryExpression,

    /// Cached name of the query's `filtered_index` timeline. `None` for
    /// static-only queries.
    pub filtered_index_timeline: Option<TimelineName>,

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
    pub processed_through_time: Option<TimeInt>,

    /// Most recent horizon seen by `flush_incremental_to`.
    /// Carried only to back the `debug_assert!` that the horizon is
    /// monotonically non-decreasing — if it ever regresses we'd
    /// re-emit rows we already shipped, or `gc_up_to_horizon` would
    /// drop chunks whose rows haven't been emitted yet.
    last_horizon: Option<TimeInt>,

    /// Largest `time_range().max()` seen across every arrived chunk on
    /// the filtered timeline. `None` until the first temporal chunk
    /// arrives.
    ///
    /// Used by [`Self::flush_incremental_to`] as a cheap pre-check: if no
    /// arrived chunk has rows past `processed_through_time`, building a
    /// fresh `QueryHandle` cannot produce output, so we skip the build.
    /// `time_max` (not `time_min`) is the right bound because a single
    /// chunk's rows can straddle a horizon — a chunk at `time_min=10`,
    /// `time_max=100` still has emittable rows after `processed_through`
    /// crosses 10.
    pub max_arrived_time_max: Option<TimeInt>,

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

impl SegmentStore {
    pub fn new(
        origin: re_uri::Origin,
        segment_id: SegmentId,
        query_expression: &QueryExpression,
        index_values: &IndexValuesMap,
    ) -> Self {
        // The application id of this throwaway store is only used for debugging.
        let application_id = ApplicationId::new_or_unknown(segment_id.as_ref());
        let store_id = StoreId::random(StoreKind::Recording, application_id);
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
            origin,
            segment_id,
            store,
            engine,
            query_expression: individual_query,
            filtered_index_timeline,
            processed_through_time: None,
            last_horizon: None,
            max_arrived_time_max: None,
            protected_chunks_scratch: ahash::HashSet::default(),
        }
    }

    /// Current decoded bytes held in `store`. Reads `ChunkStore` stats so
    /// the value reflects any post-construction inserts and any chunks
    /// reclaimed as the safe horizon advances.
    pub fn store_bytes(&self) -> u64 {
        self.store.read().stats().total().total_size_bytes
    }

    /// Insert one decoded chunk into the store and update the
    /// arrival high-water mark on the filtered timeline.
    ///
    /// `max_arrived_time_max` is updated whenever the chunk is temporal
    /// on the filtered timeline — `flush_incremental_to`'s path-1b
    /// fast-skip relies on it being monotonic across the segment's full
    /// arrival history.
    ///
    /// Callers that track per-chunk arrivals for their horizon source
    /// (v1 manifest, v2 plan cursor) must record them *after* this
    /// returns success: recording before insert would briefly claim a
    /// chunk arrived that the store doesn't actually hold.
    pub fn insert_chunk(&mut self, chunk: &Arc<Chunk>) -> ApiResult<()> {
        self.store.write().insert_chunk(chunk).map_err(|err| {
            ApiError::internal_with_source(
                &self.origin,
                None,
                err,
                "inserting chunk into in-memory store",
            )
        })?;

        if let Some(timeline) = self.filtered_index_timeline.as_ref()
            && let Some(time_col) = chunk.timelines().get(timeline)
        {
            let time_max = time_col.time_range().max();
            self.max_arrived_time_max = Some(
                self.max_arrived_time_max
                    .map_or(time_max, |prev| prev.max(time_max)),
            );
        }

        Ok(())
    }

    /// Run the safe-horizon emit + GC step for this segment, up to the
    /// caller-supplied `horizon`.
    ///
    /// Two paths:
    /// 1. **Fast skip.** Horizon hasn't advanced since the last emit,
    ///    or already at horizon = max → no work, no `next_n_rows` call.
    /// 2. **Horizon emit + GC.** The horizon advanced → emit rows up to
    ///    and including the new horizon, then drop chunks strictly
    ///    below it from the in-memory store; the freed bytes are
    ///    reported in the returned outcome.
    ///
    /// Callers MUST only invoke this on the segment currently at the
    /// head of the emission order. Emitting rows from a non-head
    /// segment would violate the `[segment_id ASC, sort_index ASC]`
    /// ordering claim advertised by `SegmentStreamExec::try_new`.
    ///
    /// Callers are also responsible for the two mode guards that live
    /// above this layer: `using_index_values` segments must never take
    /// the incremental path (the explicit value list overrides
    /// `filtered_index_range`, so every cycle would replay all rows —
    /// debug-asserted below), and the horizon itself comes from the
    /// caller's tracking structure.
    pub async fn flush_incremental_to(
        &mut self,
        horizon: TimeInt,
        projected_schema: &Arc<Schema>,
        output_channel: &Sender<RecordBatch>,
        rows_sent: &mut usize,
        limit_rows: Option<usize>,
    ) -> ApiResult<IncrementalFlushOutcome> {
        // Catch a caller that failed to gate the incremental path on
        // `using_index_values` (see this method's docs). In release the
        // failure is silent: duplicated rows plus carry-forward values
        // the GC below has already stripped.
        re_log::debug_assert!(
            self.query_expression.using_index_values.is_none(),
            "flush_incremental_to called on a using_index_values segment",
        );

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
            return Ok(IncrementalFlushOutcome::default());
        }

        // Path 1b (no-arrivals-in-range fast skip): no arrived chunk
        // has rows past `processed_through_time`, so the upcoming emit
        // cycle cannot produce output. Skip the `QueryHandle` build but
        // still run GC — the horizon advanced (per path 1's check
        // above), and GC at the new horizon may free chunks even when
        // no new rows are emittable. Advance `processed_through_time`
        // so the next cycle's `filtered_index_range.min` skips the
        // empty range we just confirmed; safe because the caller's
        // horizon invariant guarantees no future chunk arrives with
        // `time_min` <= horizon.
        if self
            .max_arrived_time_max
            .is_none_or(|tmax| self.processed_through_time.is_some_and(|p| tmax <= p))
        {
            let freed_bytes = self.gc_up_to_horizon(horizon);
            self.processed_through_time = Some(horizon);
            return Ok(IncrementalFlushOutcome {
                rows_emitted: 0,
                freed_bytes,
            });
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
        let freed_bytes = self.gc_up_to_horizon(horizon);
        Ok(IncrementalFlushOutcome {
            rows_emitted: *rows_sent - rows_before,
            freed_bytes,
        })
    }

    /// Build a fresh `QueryHandle` constrained to
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
            &self.origin,
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
    /// advanced, returning the number of freed bytes.
    /// No-op (returning 0) for queries without a temporal
    /// `filtered_index`.
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
    pub fn gc_up_to_horizon(&mut self, horizon: TimeInt) -> u64 {
        let Some(timeline_name) = self.filtered_index_timeline else {
            return 0;
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
        bytes_before.saturating_sub(bytes_after)
    }

    /// Final drain. Emits everything still in `store` that's past
    /// `processed_through_time` through the output channel.
    pub async fn flush(
        &mut self,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataframe_query_provider::test_utils::temporal_chunk;

    fn store_with_timeline(segment_id: &str, timeline_name: &str) -> SegmentStore {
        let query_expression = QueryExpression {
            filtered_index: Some(TimelineName::try_new(timeline_name).unwrap()),
            ..Default::default()
        };
        SegmentStore::new(
            re_uri::Origin::test(),
            SegmentId::from(segment_id),
            &query_expression,
            &None,
        )
    }

    /// All store contents are required to carry the latest-at value at
    /// the horizon → GC must drop nothing and report zero freed bytes.
    /// Exercises the `protected_chunks` path plus the
    /// `protected_time_ranges = (horizon+1, MAX]` path together.
    ///
    /// Setup:
    /// - `/a` has its only chunk at `t=10` → latest-at at `t=50` for
    ///   `/a` is `/a@10`, protected via `protected_chunks`.
    /// - `/b` has chunks at `t=20` (latest-at carry-forward at `t=50`,
    ///   protected via `protected_chunks`) and `t=100` (sits past
    ///   horizon, protected via `protected_time_ranges`).
    #[test]
    fn gc_up_to_horizon_preserves_carry_forward() {
        let mut segment = store_with_timeline("seg", "frame");
        for chunk in [
            temporal_chunk("/a", "frame", 10),
            temporal_chunk("/b", "frame", 20),
            temporal_chunk("/b", "frame", 100),
        ] {
            segment.insert_chunk(&Arc::new(chunk)).unwrap();
        }

        let bytes_before = segment.store_bytes();
        assert_eq!(segment.store.read().num_physical_chunks(), 3);

        let freed = segment.gc_up_to_horizon(TimeInt::new_temporal(50));

        assert_eq!(
            segment.store.read().num_physical_chunks(),
            3,
            "carry-forward chunks must survive GC under latest-at semantics",
        );
        assert_eq!(
            segment.store_bytes(),
            bytes_before,
            "no bytes freed when every chunk is protected",
        );
        assert_eq!(freed, 0, "freed bytes must be zero when nothing dropped");
    }

    /// Superseded chunks (older than the latest-at value at the horizon
    /// for their entity) are not protected and must be GC'd, with the
    /// freed bytes reported back.
    ///
    /// Setup:
    /// - `/a` has chunks at `t=10, 20, 30`. Horizon=50 → latest-at for
    ///   `/a` is `/a@30` (protected). `/a@10` and `/a@20` are
    ///   superseded → GC'd.
    #[test]
    fn gc_up_to_horizon_drops_superseded_chunks() {
        let mut segment = store_with_timeline("seg", "frame");
        for chunk in [
            temporal_chunk("/a", "frame", 10),
            temporal_chunk("/a", "frame", 20),
            temporal_chunk("/a", "frame", 30),
        ] {
            segment.insert_chunk(&Arc::new(chunk)).unwrap();
        }

        let bytes_before = segment.store_bytes();
        assert_eq!(segment.store.read().num_physical_chunks(), 3);

        let freed = segment.gc_up_to_horizon(TimeInt::new_temporal(50));

        let bytes_after = segment.store_bytes();
        assert_eq!(
            segment.store.read().num_physical_chunks(),
            1,
            "only the latest-at chunk (@30) must remain",
        );
        assert!(
            bytes_after < bytes_before,
            "GC must free bytes when chunks are dropped (before={bytes_before}, after={bytes_after})",
        );
        assert_eq!(
            freed,
            bytes_before - bytes_after,
            "reported freed bytes must match the store-stats delta",
        );
    }

    /// Across multiple `gc_up_to_horizon` calls, the
    /// `protected_chunks_scratch` `HashSet` must retain its allocated
    /// capacity (via `clear()` + `mem::swap` back from the
    /// `GarbageCollectionOptions`). Verifies the scratch reuse contract
    /// — a future refactor that re-allocates per call would silently
    /// regress the IO→CPU hot path.
    #[test]
    fn gc_up_to_horizon_reuses_scratch_capacity() {
        let mut segment = store_with_timeline("seg", "frame");
        // Populate enough entities that `protected_chunks_scratch`
        // grabs a non-trivial capacity on the first call.
        for i in 0..64 {
            segment
                .insert_chunk(&Arc::new(temporal_chunk(&format!("/e{i}"), "frame", 0)))
                .unwrap();
        }
        assert_eq!(
            segment.protected_chunks_scratch.capacity(),
            0,
            "fresh SegmentStore starts with zero scratch capacity",
        );

        segment.gc_up_to_horizon(TimeInt::new_temporal(50));
        let cap_after_first = segment.protected_chunks_scratch.capacity();
        assert!(
            cap_after_first > 0,
            "scratch must retain capacity after gc (got {cap_after_first})",
        );
        // gc returned; the swap-back restores ownership but clear()
        // happens at the *start* of the next call, so the set may
        // still hold the inserted IDs here. The contract under test
        // is capacity, not size.

        segment.gc_up_to_horizon(TimeInt::new_temporal(60));
        assert!(
            segment.protected_chunks_scratch.capacity() >= cap_after_first,
            "capacity must not shrink across calls (before={cap_after_first}, after={})",
            segment.protected_chunks_scratch.capacity(),
        );
    }

    /// Without a temporal `filtered_index` (`filtered_index_timeline ==
    /// None`), `gc_up_to_horizon` must short-circuit. The static-only
    /// query path otherwise has no timeline to feed to
    /// `LatestAtQuery::new`.
    #[test]
    fn gc_up_to_horizon_noop_without_filtered_index() {
        let mut segment = SegmentStore::new(
            re_uri::Origin::test(),
            SegmentId::from("seg"),
            &QueryExpression::default(),
            &None,
        );
        assert!(segment.filtered_index_timeline.is_none());

        segment
            .insert_chunk(&Arc::new(temporal_chunk("/a", "frame", 10)))
            .unwrap();
        let bytes_before = segment.store_bytes();

        let freed = segment.gc_up_to_horizon(TimeInt::new_temporal(50));

        assert_eq!(
            segment.store_bytes(),
            bytes_before,
            "no-op leaves bytes unchanged"
        );
        assert_eq!(freed, 0, "no-op reports zero freed bytes");
    }

    /// `insert_chunk` must advance `max_arrived_time_max` monotonically
    /// on the filtered timeline, and ignore chunks with no data on it.
    #[test]
    fn insert_chunk_tracks_max_arrived_time_max() {
        let mut segment = store_with_timeline("seg", "frame");
        assert!(segment.max_arrived_time_max.is_none());

        segment
            .insert_chunk(&Arc::new(temporal_chunk("/a", "frame", 20)))
            .unwrap();
        assert_eq!(
            segment.max_arrived_time_max,
            Some(TimeInt::new_temporal(20))
        );

        // A later chunk with an earlier time must not regress the max.
        segment
            .insert_chunk(&Arc::new(temporal_chunk("/a", "frame", 10)))
            .unwrap();
        assert_eq!(
            segment.max_arrived_time_max,
            Some(TimeInt::new_temporal(20))
        );

        // A chunk on a different timeline leaves it untouched.
        segment
            .insert_chunk(&Arc::new(temporal_chunk("/a", "other", 99)))
            .unwrap();
        assert_eq!(
            segment.max_arrived_time_max,
            Some(TimeInt::new_temporal(20))
        );
    }
}
