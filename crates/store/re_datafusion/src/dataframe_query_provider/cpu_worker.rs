//! CPU-side worker that consumes decoded chunks from the IO pipeline,
//! routes them to per-segment in-memory stores, drives the query handle,
//! and emits aligned `RecordBatch`es downstream.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch, RecordBatchOptions, StringArray};
use arrow::datatypes::Schema;
use re_dataframe::external::re_chunk_store::ChunkStore;
use re_dataframe::utils::align_record_batch_to_schema;
use re_dataframe::{
    ChunkStoreConfig, ChunkStoreHandle, QueryCache, QueryEngine, QueryExpression, QueryHandle,
    StorageEngine,
};
use re_log_types::{ApplicationId, StoreId, StoreKind};
use re_protos::{cloud::v1alpha1::ScanSegmentTableResponse, common::v1alpha1::ext::SegmentId};
use re_redap_client::{ApiError, ApiResult};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{Instrument as _, instrument};

use crate::chunk_fetcher::SortedChunksWithSegment;
use crate::dataframe_query_common::{
    DEFAULT_BATCH_BYTES, DEFAULT_BATCH_ROWS, IndexValuesMap, prepend_string_column_schema,
};
use crate::pipeline_budget::PipelineBudget;

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
            ScanSegmentTableResponse::FIELD_SEGMENT_ID,
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
/// The IO side announces each segment's total chunk count *before*
/// dispatching its fetches, so the CPU worker can detect segment
/// completion by counting arrivals without needing to wait for the
/// channel to close. This replaces the prior implicit "`segment_id`
/// changes" signal, which broke once multiple segments could be in
/// flight on the CPU side at once.
pub(super) enum CpuWorkerMsg {
    /// Total number of chunks the server has announced for `segment_id`.
    /// Sent once per segment from the IO loop; under normal ordering it
    /// arrives before any [`Self::Chunks`] for that segment, but the
    /// worker tolerates either order.
    SegmentChunkCount { segment_id: SegmentId, count: usize },

    /// A batch of decoded chunks for one segment. Multiple `Chunks`
    /// messages may arrive per segment.
    Chunks(SortedChunksWithSegment),
}

/// Per-segment in-memory store + query handle used by the CPU worker.
///
/// Holds an `Arc<PipelineBudget>` and refunds the bytes currently in
/// `store` to the budget on [`Drop`]. This covers every exit path
/// uniformly — `flush` success, `flush` error via `?`, worker
/// early-return on an upstream error, consumer hangup mid-segment,
/// panic — since `Drop` is guaranteed to run exactly once. Without the
/// refund a `?` early-return or cancellation would leak the reservation
/// to sibling partitions for the remainder of the query.
///
/// Tracks `expected_chunks` and `received_chunks` so the worker can fire
/// the flush as soon as every chunk the server promised for this segment
/// has been inserted, rather than waiting for a segment-boundary signal
/// from the channel.
struct CurrentStores {
    segment_id: SegmentId,
    store: ChunkStoreHandle,
    query_handle: QueryHandle<StorageEngine>,
    pipeline_budget: Arc<PipelineBudget>,

    /// Total number of chunks the server promised for this segment via
    /// [`CpuWorkerMsg::SegmentChunkCount`]. `None` until that message
    /// arrives; [`Self::is_complete`] returns `false` while it is `None`
    /// so the segment can still be drained safely on end-of-stream
    /// cleanup but is not prematurely flushed mid-stream.
    expected_chunks: Option<usize>,

    /// Cumulative chunks inserted into `store` so far. Compared against
    /// `expected_chunks` to detect segment completion.
    received_chunks: usize,
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

        let query_engine = QueryEngine::new(store.clone(), QueryCache::new_handle(store.clone()));
        let mut individual_query = query_expression.clone();

        let values = index_values
            .as_ref()
            .and_then(|index_values| index_values.get(&segment_id));
        if let Some(values) = values {
            individual_query.using_index_values = Some(values.clone());
        }

        let query_handle = query_engine.query(individual_query);

        Self {
            segment_id,
            store,
            query_handle,
            pipeline_budget,
            expected_chunks: None,
            received_chunks: 0,
        }
    }

    /// Current decoded bytes held in `store`. Reads `ChunkStore` stats so
    /// the value reflects any post-construction inserts (and, once
    /// horizon-driven GC lands, any chunks reclaimed as the safe horizon
    /// advances).
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

    /// Drain every remaining row through the output channel. Consumes
    /// `self` so the reservation is returned via `Drop` immediately
    /// after the last batch ships — and via the same `Drop` if a
    /// `send_next_row_batch` error short-circuits the loop via `?`.
    #[instrument(level = "debug", skip_all)]
    async fn flush(
        mut self,
        projected_schema: &Arc<Schema>,
        output_channel: &Sender<RecordBatch>,
        rows_sent: &mut usize,
        limit_rows: Option<usize>,
    ) -> ApiResult<()> {
        while send_next_row_batch(
            &mut self.query_handle,
            &self.segment_id,
            projected_schema,
            output_channel,
            rows_sent,
            limit_rows,
        )
        .await?
        .is_some()
        {}
        Ok(())
    }
}

impl Drop for CurrentStores {
    fn drop(&mut self) {
        // Refund whatever the store currently holds. See the long
        // comment in the CPU worker about why `store_bytes >= reserved_sum`
        // and the resulting under-utilization is benign.
        self.pipeline_budget.release(self.store_bytes() as usize);
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
                    let taken = current_stores
                        .remove(&segment_id)
                        .expect("just inserted via entry()");
                    ready_pending.insert(segment_id, taken);
                    if drain_head_ready(
                        &mut emit_order,
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
                        stores
                            .store
                            .write()
                            .insert_chunk(&Arc::new(chunk))
                            .map_err(|err| {
                                ApiError::internal_with_source(
                                    None,
                                    err,
                                    "inserting chunk into in-memory store",
                                )
                            })?;
                    }
                    stores.received_chunks += n_chunks;
                }
                let complete = stores.is_complete();
                if complete {
                    let taken = current_stores
                        .remove(&segment_id)
                        .expect("just inserted via entry()");
                    ready_pending.insert(segment_id, taken);
                    if drain_head_ready(
                        &mut emit_order,
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
    }

    // End-of-stream: drain everything still pending in announcement
    // order. Two cases reach here:
    //  * Segments in `ready_pending` blocked behind an incomplete head
    //    (happens when an IO-side error or short-circuit left the head's
    //    chunk count unsatisfied).
    //  * Segments still in `current_stores` whose `expected_chunks`
    //    never arrived or never satisfied — same reasons.
    // Walking `emit_order` preserves segment_id-ASC order to match the
    // plan's ordering claim. `Drop for CurrentStores` refunds the budget
    // after each flush (or on `?` short-circuit).
    while let Some(seg) = emit_order.pop_front() {
        let stores = ready_pending
            .remove(&seg)
            .or_else(|| current_stores.remove(&seg));
        if let Some(stores) = stores {
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
        }
    }

    Ok(())
}

/// Pop and flush every `ready_pending` segment sitting at the front of
/// `emit_order`. Stops at the first front-of-queue segment that is not
/// yet ready, preserving the announcement-order emit contract.
///
/// Returns `Ok(true)` if the row limit was hit and the caller should
/// short-circuit; `Ok(false)` otherwise.
async fn drain_head_ready(
    emit_order: &mut VecDeque<SegmentId>,
    ready_pending: &mut HashMap<SegmentId, CurrentStores>,
    completed_segments: &mut HashSet<SegmentId>,
    projected_schema: &Arc<Schema>,
    output_channel: &Sender<RecordBatch>,
    rows_sent: &mut usize,
    limit_rows: Option<usize>,
) -> ApiResult<bool> {
    while emit_order
        .front()
        .is_some_and(|head| ready_pending.contains_key(head))
    {
        let head = emit_order.pop_front().expect("front existed");
        let stores = ready_pending
            .remove(&head)
            .expect("contains_key check above");
        completed_segments.insert(head);
        stores
            .flush(projected_schema, output_channel, rows_sent, limit_rows)
            .await?;
        if limit_rows.is_some_and(|l| *rows_sent >= l) {
            return Ok(true);
        }
    }
    Ok(false)
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
    async fn test_drain_head_ready_preserves_announcement_order() {
        let budget = Arc::new(PipelineBudget::new(1 << 30, 4));
        let schema = Arc::new(Schema::empty());
        let (output_tx, _output_rx) = tokio::sync::mpsc::channel::<RecordBatch>(8);

        let mut emit_order: VecDeque<SegmentId> = VecDeque::new();
        emit_order.push_back(SegmentId::from("A"));
        emit_order.push_back(SegmentId::from("B"));
        emit_order.push_back(SegmentId::from("C"));

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
        let hit_limit = drain_head_ready(
            &mut emit_order,
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
        let hit_limit = drain_head_ready(
            &mut emit_order,
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
}
