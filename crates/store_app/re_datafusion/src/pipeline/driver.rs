//! The per-partition driver: one owned async task that runs the whole v2
//! tree — issue fetches through `buffered(N)`, route deliveries to
//! per-segment state, and emit rows head-of-line in global segment order.
//!
//! A single loop with a head pointer, not per-segment streams under an
//! ordered concat: a real concat polls only the head, which would stop
//! successor segments from doing their insert work while the head emits.
//! Here successors insert (CPU pipelining) but only the head emits/GCs,
//! which is what keeps the `[segment_id ASC, sort_index ASC]`
//! output-ordering claim honest.
//!
//! One owned future ⇒ one cancellation path: dropping the driver's task
//! drops the `buffered` stream (aborting in-flight spawned fetches via
//! their `AbortOnDropHandle`s) and every `SegmentState` (refunding window
//! bytes via ledger `Drop`).

use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::datatypes::Schema;
use futures::StreamExt as _;
use re_dataframe::QueryExpression;
use re_redap_client::{ApiError, ApiResult};
use tokio::sync::mpsc::Sender;

use super::executor::{Delivered, FetchExecutor};
use super::plan::{FetchPlan, PlannedSegment, SegmentEmitMode};
use super::segment::SegmentState;
use crate::analytics::QueryErrorKind;
use crate::dataframe_query_common::{DataframeClientAPI, IndexValuesMap};

/// Result of a head-of-line pass.
enum Progress {
    Continue,
    LimitReached,
}

/// Measured-memory counterpart to the issuance window, reported on the
/// partition span once the driver is done.
///
/// `W` bounds *plan-estimate* bytes, which is not the same as bounding
/// memory: estimates fall back to the compressed `chunk_byte_len` when the
/// uncompressed size is missing, so they understate decoded size by an
/// unmeasured factor. Pairing what the window was asked for against what the
/// stores actually held is the only way to know what `W` was really worth —
/// and, with [`super::window::OverdraftAudit`], whether it bounded anything
/// at all.
#[derive(Default)]
struct MemoryAudit {
    /// Σ plan-estimate bytes charged to the window for delivered chunks.
    est_bytes: u64,

    /// Σ measured decoded bytes those same chunks added to their stores.
    measured_bytes: u64,

    /// High-water mark of measured bytes resident across all live segments.
    peak_measured_bytes: u64,

    /// Running total behind `peak_measured_bytes`.
    live_measured_bytes: u64,
}

impl MemoryAudit {
    fn inserted(&mut self, measured: u64) {
        self.measured_bytes += measured;
        self.live_measured_bytes += measured;
        self.peak_measured_bytes = self.peak_measured_bytes.max(self.live_measured_bytes);
    }

    /// Chunks left a store, via GC or via the whole segment being dropped.
    fn released(&mut self, measured: u64) {
        self.live_measured_bytes = self.live_measured_bytes.saturating_sub(measured);
    }
}

/// One DataFusion partition's v2 tree: issues the plan's batches through the
/// executor and emits rows head-of-line in plan order.
pub struct PartitionDriver<T> {
    pub plan: Arc<FetchPlan>,
    pub executor: FetchExecutor<T>,

    /// The `N` in `buffered(N)`. Must be at least 1.
    pub fetch_concurrency: usize,
    pub origin: re_uri::Origin,
    pub query_expression: QueryExpression,
    pub index_values: IndexValuesMap,
    pub projected_schema: Arc<Schema>,

    /// Stop after this many rows; `None` is unlimited.
    pub limit_rows: Option<usize>,
}

impl<T: DataframeClientAPI> PartitionDriver<T> {
    /// Run the tree to completion, emitting rows into `tx`.
    ///
    /// Consumer hangup (`tx` closed — LIMIT downstream, plan cancellation)
    /// surfaces as an emit error from the shared store code; the bridge
    /// treats it like any other termination since the consumer is gone.
    #[tracing::instrument(level = "info", skip_all, fields(
        n_segments = self.plan.segments.len(),
        n_batches = self.plan.batches.len(),
        n_buffer_all_segments = self.plan.n_buffer_all_segments(),
        max_segment_est_bytes = self.plan.max_segment_est_bytes(),
        window_bytes = self.executor.window.capacity_bytes(),
        est_bytes = tracing::field::Empty,
        measured_bytes = tracing::field::Empty,
        peak_measured_bytes = tracing::field::Empty,
        overdraft_grants = tracing::field::Empty,
        overdraft_bytes = tracing::field::Empty,
        peak_overdraft_bytes = tracing::field::Empty,
    ))]
    pub async fn run(self, tx: Sender<RecordBatch>) -> ApiResult<()> {
        let window = Arc::clone(&self.executor.window);
        let mut audit = MemoryAudit::default();

        // Recorded on the way out rather than at each return so the error
        // paths — which are exactly the runs worth explaining — report too.
        let result = self.run_inner(tx, &mut audit).await;

        let span = tracing::Span::current();
        span.record("est_bytes", audit.est_bytes);
        span.record("measured_bytes", audit.measured_bytes);
        span.record("peak_measured_bytes", audit.peak_measured_bytes);
        let overdraft = window.overdraft_audit();
        span.record("overdraft_grants", overdraft.grants);
        span.record("overdraft_bytes", overdraft.granted_bytes);
        span.record("peak_overdraft_bytes", overdraft.peak_live_bytes);

        // Span fields recorded at close reach no fmt-layer output; log them
        // for `RUST_LOG` runs.
        re_log::debug!(
            est_bytes = audit.est_bytes,
            measured_bytes = audit.measured_bytes,
            peak_measured_bytes = audit.peak_measured_bytes,
            window_bytes = window.capacity_bytes(),
            overdraft_grants = overdraft.grants,
            overdraft_bytes = overdraft.granted_bytes,
            peak_overdraft_bytes = overdraft.peak_live_bytes,
            "v2 partition driver finished",
        );

        result
    }

    async fn run_inner(self, tx: Sender<RecordBatch>, audit: &mut MemoryAudit) -> ApiResult<()> {
        let Self {
            plan,
            executor,
            fetch_concurrency,
            origin,
            query_expression,
            index_values,
            projected_schema,
            limit_rows,
        } = self;

        // Per-segment state, keyed by plan segment index, created lazily on
        // first delivery. `head` walks `plan.segments` in order.
        let mut states: HashMap<usize, SegmentState> = HashMap::new();
        let mut head: usize = 0;
        let mut rows_sent: usize = 0;
        // `buffered` yields in plan order, so this doubles as "every batch
        // with a lower index has been delivered".
        let mut batches_delivered: usize = 0;

        // Shared with every in-flight fetch: the batch this loop is blocked
        // on takes its window bytes without waiting.
        let front = Arc::clone(&executor.front);

        let mut deliveries = futures::stream::iter(
            plan.batches
                .iter()
                .cloned()
                .enumerate()
                .map(|(task_idx, batch)| executor.issue(task_idx, batch)),
        )
        // `buffered` (not `buffer_unordered`): deliveries arrive in plan
        // order, and a straggler delays delivery of at most N-1 completed
        // batches. `batches_delivered`, under-delivery verdicts in
        // `advance_head`, and the `DeliveryFront` all rely on that order: out
        // of order, a segment could be judged under-delivered while its
        // chunks are still in flight.
        .buffered(fetch_concurrency);

        while let Some(delivered) = deliveries.next().await {
            let Delivered { mut lease, chunks } = delivered?;
            batches_delivered += 1;
            front.advance_to(batches_delivered);

            // ── router ──────────────────────────────────────────────
            for (chunk, segment_id) in chunks {
                let chunk_id = chunk.id();
                let Some(segment_id) = segment_id else {
                    re_log::error_once!(
                        "fetched chunk carries no segment id; dropping (server anomaly)"
                    );
                    continue;
                };
                let Some(segment_idx) = plan.segment_index(&segment_id) else {
                    re_log::error_once!(
                        "plan/delivery divergence: a chunk of an unplanned segment arrived; dropping"
                    );
                    continue;
                };
                if segment_idx < head {
                    // `advance_head` never revisits an index below `head`, so
                    // a state re-created here could neither emit nor be
                    // reclaimed until end of stream — it would hold this
                    // chunk's window bytes and its decoded chunk for the rest
                    // of the partition. Dropping leaves the bytes on the batch
                    // lease, whose residual drop refunds them.
                    re_log::error_once!(
                        "plan/delivery divergence: a chunk arrived after its segment was flushed; dropping"
                    );
                    continue;
                }
                let state = states.entry(segment_idx).or_insert_with(|| {
                    SegmentState::new(
                        origin.clone(),
                        Arc::clone(&plan.segments[segment_idx]),
                        &query_expression,
                        &index_values,
                    )
                });

                let Some(&(key, est_bytes)) = state.planned.chunk_meta.get(&chunk_id) else {
                    // Only planned chunks gate the safe horizon, so an
                    // unplanned one could change already-emitted history; see
                    // `unplanned_chunk_is_dropped`.
                    re_log::error_once!(
                        "plan/delivery divergence: a chunk not in the fetch plan arrived; dropping"
                    );
                    continue;
                };

                if !state.first_delivery(chunk_id) {
                    // The store already holds it and the cursor and ledger
                    // already counted it; a repeated `mark_delivered` would
                    // count some *other* planned chunk sharing its key as
                    // delivered, flushing a partial segment as complete.
                    re_log::error_once!(
                        "plan/delivery divergence: a chunk was delivered more than once; dropping the repeat"
                    );
                    continue;
                }

                let before = state.store.store_bytes();
                state.store.insert_chunk(&Arc::new(chunk))?;
                audit.inserted(state.store.store_bytes().saturating_sub(before));
                audit.est_bytes += est_bytes;
                state.cursor.mark_delivered(key);
                state.ledger.absorb(lease.split(est_bytes));
            }
            // Residual covers undelivered/divergent chunks of this batch.
            drop(lease);

            // ── head-of-line emit ───────────────────────────────────
            if matches!(
                advance_head(
                    &plan,
                    &origin,
                    &executor.pending_analytics,
                    &mut states,
                    &mut head,
                    batches_delivered,
                    &projected_schema,
                    &tx,
                    &mut rows_sent,
                    limit_rows,
                    audit,
                )
                .await?,
                Progress::LimitReached
            ) {
                return Ok(());
            }
        }

        // The last delivery's pass ran with every segment done receiving, so
        // it either failed or walked `head` to the end of the plan; the router
        // never creates a state below `head`.
        re_log::debug_assert!(
            states.is_empty(),
            "segment states survived the end-of-stream walk: {:?}",
            states.keys().collect::<Vec<_>>(),
        );

        Ok(())
    }
}

/// Drive emit progress from the head of the plan's segment order.
///
/// 1. Head complete → final drain via the shared `flush`, drop its state
///    (window refund), advance to the next segment, loop.
/// 2. Head past its last batch and still incomplete → it can never complete;
///    fail the query (see [`under_delivered`]). Failing also un-wedges the
///    partition: the head no longer pins window bytes successors need.
/// 3. Head in flight in `Watermark` mode → incremental emit + GC up to the
///    cursor's safe horizon, release the removed chunks' estimate bytes.
/// 4. Head in flight in `BufferAll` mode, or nothing delivered yet → wait.
///
/// Only the head ever emits — successor segments buffer and insert but must
/// not produce rows out of segment order.
///
/// `batches_delivered` counts deliveries consumed so far, so batch indices
/// `0..batches_delivered` have arrived. It is what makes case 2 decidable.
async fn advance_head(
    plan: &FetchPlan,
    origin: &re_uri::Origin,
    analytics: &crate::PendingQueryAnalytics,
    states: &mut HashMap<usize, SegmentState>,
    head: &mut usize,
    batches_delivered: usize,
    projected_schema: &Arc<Schema>,
    tx: &Sender<RecordBatch>,
    rows_sent: &mut usize,
    limit_rows: Option<usize>,
    audit: &mut MemoryAudit,
) -> ApiResult<Progress> {
    while *head < plan.segments.len() {
        let head_idx = *head;
        // Every batch that could carry this segment has been delivered, so
        // whatever it holds now is all it will ever hold.
        let done_receiving = plan.segments[head_idx].last_batch_idx < batches_delivered;

        let Some(state) = states.get_mut(&head_idx) else {
            if done_receiving {
                // Announced, never delivered at all.
                return Err(under_delivered(
                    origin,
                    analytics,
                    &plan.segments[head_idx],
                    0,
                ));
            }
            // Nothing delivered for the head yet.
            return Ok(Progress::Continue);
        };

        if state.cursor.is_complete() {
            let Some(mut state) = states.remove(&head_idx) else {
                continue;
            };
            state
                .store
                .flush(projected_schema, tx, rows_sent, limit_rows)
                .await?;
            audit.released(state.store.store_bytes());
            drop(state); // ledger Drop refunds the segment's window bytes
            *head += 1;
            if limit_rows.is_some_and(|l| *rows_sent >= l) {
                return Ok(Progress::LimitReached);
            }
            continue; // next segment may already be complete
        }

        if done_receiving {
            // Nothing past the safe horizon is flushed: latest-at output there
            // would carry values whose source chunks never arrived.
            let expected = state.planned.expected_chunks();
            let received = expected.saturating_sub(state.cursor.outstanding_chunks());
            return Err(under_delivered(
                origin,
                analytics,
                &plan.segments[head_idx],
                received,
            ));
        }

        if state.mode() == SegmentEmitMode::Watermark
            && let Some(horizon) = state.cursor.safe_horizon()
        {
            let outcome = state
                .store
                .flush_incremental_to(horizon, projected_schema, tx, rows_sent, limit_rows)
                .await?;
            let est = state.est_bytes_for(outcome.gc.removed_chunks.iter().copied());
            state.ledger.release(est);
            audit.released(outcome.gc.freed_bytes);
            if limit_rows.is_some_and(|l| *rows_sent >= l) {
                return Ok(Progress::LimitReached);
            }
        }
        return Ok(Progress::Continue);
    }
    Ok(Progress::Continue)
}

/// The error for a segment whose announced chunks did not all arrive by the
/// end of its fetches.
///
/// Failing the query is deliberate: the alternative is an `Ok` result with
/// rows silently missing, which a consumer cannot tell from a legitimate
/// empty tail. Nothing retries it automatically; a fresh query replans
/// against the server's current state, which is the remedy when the plan
/// went stale underneath the fetches.
///
/// Records [`QueryErrorKind::Incomplete`] here because the bridge labels any
/// driver error that arrives without a kind as a store-path failure.
fn under_delivered(
    origin: &re_uri::Origin,
    analytics: &crate::PendingQueryAnalytics,
    segment: &PlannedSegment,
    received: usize,
) -> ApiError {
    analytics.record_error(QueryErrorKind::Incomplete);
    ApiError::internal(
        origin,
        format!(
            "segment {} under-delivered: {received}/{} announced chunks arrived by the end of \
             its fetches; failing the query instead of returning a partial result",
            segment.segment_id,
            segment.expected_chunks(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{
        FetchBehavior, chunk_info_batch, emitted_component_values, emitted_segment_ids, run_driver,
        run_driver_projected, scalar_chunk, scalar_chunk_on_component, schema_with_component,
    };

    /// Two segments, watermark mode, everything delivered: every planned row
    /// emits, and — the load-bearing part — rows come out grouped by segment
    /// in plan order. Head-of-line emission is what keeps the `[segment_id ASC,
    /// sort_index ASC]` claim honest, and a violated claim is silently wrong
    /// output rather than a slowdown.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn emits_all_segments_in_plan_order() {
        let chunk_infos = vec![
            chunk_info_batch(
                "seg-a",
                Some("frame"),
                &[Some(10), Some(20)],
                &[false, false],
                &[Some(100), Some(100)],
            ),
            chunk_info_batch("seg-b", Some("frame"), &[Some(30)], &[false], &[Some(100)]),
        ];

        let (result, batches, _audit) = run_driver(
            chunk_infos,
            "frame",
            [FetchBehavior::Chunks(vec![
                (
                    scalar_chunk("seg-a", 0, "frame", Some(10), 1),
                    "seg-a".to_owned(),
                ),
                (
                    scalar_chunk("seg-a", 1, "frame", Some(20), 2),
                    "seg-a".to_owned(),
                ),
                (
                    scalar_chunk("seg-b", 0, "frame", Some(30), 3),
                    "seg-b".to_owned(),
                ),
            ])],
            64 * 1024 * 1024,
            4,
            None,
        )
        .await;

        result.expect("driver completes cleanly");
        let ids = emitted_segment_ids(&batches);
        assert_eq!(
            ids,
            vec!["seg-a", "seg-a", "seg-b"],
            "rows must emit grouped by segment, in plan order"
        );
    }

    /// A row limit stops the driver cleanly mid-plan: exactly `limit` rows
    /// emit and later segments are never flushed.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn limit_stops_the_driver() {
        let chunk_infos = vec![
            chunk_info_batch(
                "seg-a",
                Some("frame"),
                &[Some(10), Some(20)],
                &[false, false],
                &[Some(100), Some(100)],
            ),
            chunk_info_batch("seg-b", Some("frame"), &[Some(30)], &[false], &[Some(100)]),
        ];

        let (result, batches, _audit) = run_driver(
            chunk_infos,
            "frame",
            [FetchBehavior::Chunks(vec![
                (
                    scalar_chunk("seg-a", 0, "frame", Some(10), 1),
                    "seg-a".to_owned(),
                ),
                (
                    scalar_chunk("seg-a", 1, "frame", Some(20), 2),
                    "seg-a".to_owned(),
                ),
                (
                    scalar_chunk("seg-b", 0, "frame", Some(30), 3),
                    "seg-b".to_owned(),
                ),
            ])],
            64 * 1024 * 1024,
            4,
            Some(1),
        )
        .await;

        result.expect("reaching the limit is a clean stop");
        assert_eq!(emitted_segment_ids(&batches), vec!["seg-a"]);
    }

    /// A segment whose planned chunks never all arrive fails the query, and
    /// nothing past its safe horizon ships.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn under_delivered_segment_fails_the_query() {
        // Two chunks planned for seg-a, only one ever delivered.
        let chunk_infos = vec![chunk_info_batch(
            "seg-a",
            Some("frame"),
            &[Some(10), Some(20)],
            &[false, false],
            &[Some(100), Some(100)],
        )];

        let (result, batches, _audit) = run_driver(
            chunk_infos,
            "frame",
            [FetchBehavior::Chunks(vec![(
                scalar_chunk("seg-a", 0, "frame", Some(10), 1),
                "seg-a".to_owned(),
            )])],
            64 * 1024 * 1024,
            4,
            None,
        )
        .await;

        let err = result.expect_err("an under-delivered segment must fail the query");
        assert!(
            format!("{err}").contains("under-delivered"),
            "error should name the divergence, got: {err}",
        );
        // Both chunks share one batch, so the verdict lands on the first
        // emit pass, before any row ships.
        assert_eq!(
            emitted_segment_ids(&batches),
            Vec::<String>::new(),
            "no row may emit once the segment is known to be under-delivered",
        );
    }

    /// An under-delivered head fails the query as soon as its fetches are
    /// exhausted, before successors emit.
    ///
    /// Complete successors must not emit after the verdict — the result is
    /// partial either way, and the consumer must see an error, not rows.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn under_delivered_head_fails_the_query_before_successors_emit() {
        // Both segments are tiny, so the packer puts all three chunks into a
        // single batch and they arrive together.
        let chunk_infos = vec![
            chunk_info_batch(
                "seg-a",
                Some("frame"),
                &[Some(10), Some(20)],
                &[false, false],
                &[Some(100), Some(100)],
            ),
            chunk_info_batch("seg-b", Some("frame"), &[Some(30)], &[false], &[Some(100)]),
        ];

        // seg-a's t=20 chunk never arrives; seg-b arrives complete.
        let (result, batches, _audit) = run_driver(
            chunk_infos,
            "frame",
            [FetchBehavior::Chunks(vec![
                (
                    scalar_chunk("seg-a", 0, "frame", Some(10), 1),
                    "seg-a".to_owned(),
                ),
                (
                    scalar_chunk("seg-b", 0, "frame", Some(30), 3),
                    "seg-b".to_owned(),
                ),
            ])],
            64 * 1024 * 1024,
            1,
            None,
        )
        .await;

        let err = result.expect_err("an under-delivered head must fail the query");
        assert!(
            format!("{err}").contains("1/2 announced chunks"),
            "error should carry received/expected counts, got: {err}",
        );
        assert!(
            !emitted_segment_ids(&batches).contains(&"seg-b".to_owned()),
            "successors must not emit once the head's under-delivery is decided, got {:?}",
            emitted_segment_ids(&batches),
        );
    }

    /// A segment whose estimate exceeds the whole issuance window must still
    /// complete.
    ///
    /// `split_large_segments` spreads such a segment over several batches, and
    /// a `BufferAll` segment releases nothing until it is complete — so once
    /// its delivered batches fill the window, the next `acquire` waits on
    /// bytes that only the segment itself could free. Gating every batch alike
    /// deadlocks here; the batch the driver is blocked on must never wait.
    ///
    /// Wrapped in a timeout because the regression is a hang, which would
    /// otherwise stall CI instead of failing.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn buffer_all_segment_larger_than_window_completes() {
        use super::super::test_support::chunk_info_batch_sized;

        // No `:start` column => BufferAll. 4 x 8 MiB compressed clears the
        // 8 MiB packing target, so the planner splits this one segment across
        // four batches; 4 x 100 bytes of estimate overruns the 250-byte window.
        let chunk_infos = vec![chunk_info_batch_sized(
            "seg-a",
            None,
            &[Some(10), Some(20), Some(30), Some(40)],
            &[false; 4],
            &[Some(100); 4],
            &[8 * 1024 * 1024; 4],
        )];

        let responses = (0..4).map(|i| {
            let n = i64::try_from(i).unwrap();
            FetchBehavior::Chunks(vec![(
                scalar_chunk("seg-a", i, "frame", Some((n + 1) * 10), n),
                "seg-a".to_owned(),
            )])
        });

        let (result, batches, audit) = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            run_driver(chunk_infos, "frame", responses, 250, 4, None),
        )
        .await
        .expect("the driver must not wedge on a segment larger than the window");

        result.expect("driver completes cleanly");
        assert_eq!(
            emitted_segment_ids(&batches).len(),
            4,
            "every row of the oversized segment must emit",
        );
        // Without this the test could pass on a run that never overdrafted —
        // the regression it guards is a deadlock, and "didn't deadlock" is
        // also what a fixture that quietly fit inside `W` would report.
        assert!(
            audit.grants > 0 && audit.peak_live_bytes > 0,
            "a segment larger than `W` must be funded by the priority path, got {audit:?}",
        );
    }

    /// A chunk delivered for a segment the head has already passed must be
    /// dropped at the router, not routed into a fresh `SegmentState`.
    ///
    /// A re-created state sits below `head`, which `advance_head` never
    /// revisits, so it can neither emit nor be reclaimed until end of stream —
    /// it would pin the chunk's window bytes and its decoded chunk for the
    /// rest of the partition. The end-of-stream `debug_assert` is what fails
    /// if the router ever re-creates one.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn delivery_for_a_passed_segment_is_dropped() {
        use super::super::test_support::chunk_info_batch_sized;

        // One chunk each at the 8 MiB packing target, so the planner emits one
        // batch per segment and seg-a is complete, flushed, and passed by the
        // time batch 1 is routed.
        let chunk_infos = vec![
            chunk_info_batch_sized(
                "seg-a",
                Some("frame"),
                &[Some(10)],
                &[false],
                &[Some(100)],
                &[8 * 1024 * 1024],
            ),
            chunk_info_batch_sized(
                "seg-b",
                Some("frame"),
                &[Some(20)],
                &[false],
                &[Some(100)],
                &[8 * 1024 * 1024],
            ),
        ];

        // Batch 1 re-delivers seg-a's chunk (same `ChunkId`, so it matches the
        // plan's `chunk_meta`) alongside seg-b's. Serial, so each scripted
        // response pairs with the batch that requested it.
        let responses = [
            FetchBehavior::Chunks(vec![(
                scalar_chunk("seg-a", 0, "frame", Some(10), 1),
                "seg-a".to_owned(),
            )]),
            FetchBehavior::Chunks(vec![
                (
                    scalar_chunk("seg-b", 0, "frame", Some(20), 2),
                    "seg-b".to_owned(),
                ),
                (
                    scalar_chunk("seg-a", 0, "frame", Some(10), 1),
                    "seg-a".to_owned(),
                ),
            ]),
        ];

        let (result, batches, _audit) =
            run_driver(chunk_infos, "frame", responses, 250, 1, None).await;

        result.expect("a late duplicate delivery is not an error");
        assert_eq!(
            emitted_segment_ids(&batches),
            vec!["seg-a", "seg-b"],
            "the late duplicate must neither add a row nor displace seg-b",
        );
    }

    /// A repeated delivery of one planned chunk must not stand in for a
    /// different planned chunk that never arrives.
    ///
    /// `ChunkStore::insert_chunk` makes the duplicate a no-op, but the cursor
    /// counts per `CursorKey`, not per `ChunkId` — two chunks sharing a
    /// `time_min` share a key, so an unrejected repeat of one completes the
    /// cursor while the other is still outstanding, and the segment flushes
    /// as complete with that chunk's data missing. Rejected, the segment is
    /// under-delivered instead: the query fails, and the horizon stayed below
    /// the shared `time_min` so no row emitted first.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn duplicate_delivery_does_not_complete_the_cursor() {
        let chunk_infos = vec![chunk_info_batch(
            "seg-a",
            Some("frame"),
            &[Some(10), Some(10)],
            &[false, false],
            &[Some(100), Some(100)],
        )];

        // Chunk 0 arrives twice in the one planned batch; chunk 1 never does.
        let (result, batches, _audit) = run_driver(
            chunk_infos,
            "frame",
            [FetchBehavior::Chunks(vec![
                (
                    scalar_chunk("seg-a", 0, "frame", Some(10), 1),
                    "seg-a".to_owned(),
                ),
                (
                    scalar_chunk("seg-a", 0, "frame", Some(10), 1),
                    "seg-a".to_owned(),
                ),
            ])],
            64 * 1024 * 1024,
            4,
            None,
        )
        .await;

        let err = result.expect_err("the repeat must not complete the cursor");
        assert!(
            format!("{err}").contains("under-delivered"),
            "the failure must be the under-delivery, got: {err}",
        );
        assert!(
            emitted_segment_ids(&batches).is_empty(),
            "emitting here means the repeat completed the cursor while a planned \
             chunk was still outstanding; got rows for {:?}",
            emitted_segment_ids(&batches),
        );
    }

    /// A chunk the server delivers without the plan announcing it is dropped
    /// at the router, never inserted.
    ///
    /// Only planned chunks gate the safe horizon, so an unplanned chunk can
    /// land below `processed_through_time` — where it emits no row of its own
    /// but changes latest-at carry-forward for every row after it, making
    /// rows emitted before and after its arrival disagree about the same
    /// history — or inside a yet-unemitted range, where it adds rows the
    /// query never asked for. It also holds residency the window never
    /// charged. The fixture delivers one of each; the observable here is the
    /// added row (this harness queries without sparse fill, so carry-forward
    /// is invisible), and the drop in the router forecloses both.
    /// `run_driver` additionally asserts the window ends fully refunded.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unplanned_chunk_is_dropped() {
        use super::super::test_support::chunk_info_batch_sized;

        // One chunk per batch at the packing target, so serial delivery pairs
        // each scripted response with one batch and the horizon advances
        // between them.
        let chunk_infos = vec![chunk_info_batch_sized(
            "seg-a",
            Some("frame"),
            &[Some(10), Some(20), Some(40)],
            &[false; 3],
            &[Some(8 * 1024 * 1024); 3],
            &[8 * 1024 * 1024; 3],
        )];

        let responses = [
            FetchBehavior::Chunks(vec![(
                scalar_chunk("seg-a", 0, "frame", Some(10), 1),
                "seg-a".to_owned(),
            )]),
            FetchBehavior::Chunks(vec![(
                scalar_chunk("seg-a", 1, "frame", Some(20), 2),
                "seg-a".to_owned(),
            )]),
            // Indices 98 and 99 were never planned. t=15 is below the
            // watermark by the time it arrives (rows t=10 and t=20 already
            // emitted — inserted, it would change carry-forward for later
            // rows but never emit a row of its own); t=50 is inside the
            // final drain's range, where an insert adds a whole row the
            // query never asked for.
            FetchBehavior::Chunks(vec![
                (
                    scalar_chunk("seg-a", 2, "frame", Some(40), 3),
                    "seg-a".to_owned(),
                ),
                (
                    scalar_chunk_on_component(
                        "seg-a",
                        99,
                        "frame",
                        Some(15),
                        99,
                        re_types_core::ComponentDescriptor::partial("late_value"),
                    ),
                    "seg-a".to_owned(),
                ),
                (
                    scalar_chunk_on_component(
                        "seg-a",
                        98,
                        "frame",
                        Some(50),
                        100,
                        re_types_core::ComponentDescriptor::partial("late_value"),
                    ),
                    "seg-a".to_owned(),
                ),
            ]),
        ];

        let (result, batches, _audit) = run_driver_projected(
            chunk_infos,
            "frame",
            responses,
            64 * 1024 * 1024,
            1,
            None,
            schema_with_component("late_value"),
        )
        .await;

        result.expect("an unplanned chunk is not an error");
        assert_eq!(
            emitted_segment_ids(&batches),
            vec!["seg-a", "seg-a", "seg-a"],
            "exactly the planned rows must emit — a fourth row means the \
             unplanned t=50 chunk was inserted",
        );
        assert_eq!(
            emitted_component_values(&batches, "late_value"),
            vec![None, None, None],
            "no emitted row may carry a value from an unplanned chunk",
        );
    }

    /// A failed fetch propagates out of `run` rather than being swallowed or
    /// turned into a short read — a silently truncated result set would look
    /// like a legitimate empty tail to the consumer.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fetch_error_propagates() {
        let chunk_infos = vec![chunk_info_batch(
            "seg-a",
            Some("frame"),
            &[Some(10)],
            &[false],
            &[Some(100)],
        )];

        let (result, _batches, _audit) = run_driver(
            chunk_infos,
            "frame",
            [FetchBehavior::Fail(tonic::Status::unavailable(
                "injected fetch failure",
            ))],
            64 * 1024 * 1024,
            4,
            None,
        )
        .await;

        let err = result.expect_err("a failed fetch must surface as an error");
        assert!(
            format!("{err}").contains("injected fetch failure"),
            "error should carry the transport cause, got: {err}",
        );
    }

    /// A static chunk that lands after its segment's temporal chunks must hold
    /// back every row of that segment until it arrives.
    ///
    /// A static chunk has no position on the timeline, so it can carry values
    /// that belong to *any* row. Emitting a row before it lands ships a null
    /// where that value belongs — silently wrong output rather than a failure,
    /// which is why this is asserted against the real store/emit path and not
    /// only against the cursor arithmetic in `segment.rs`.
    ///
    /// Observed through the flush count: one emitted batch means the rows all
    /// came out of the single final drain, so no incremental emit ran while the
    /// `PreTime` chunk was outstanding. `temporal_chunks_emit_incrementally`
    /// is the companion that rules out the fixture's packing as the cause.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn late_static_chunk_holds_back_every_row() {
        use super::super::test_support::chunk_info_batch_sized;

        // Row 0 is the static chunk (null `:start`, `chunk_is_static`), rows 1
        // and 2 are temporal. One chunk per batch at the packing target, so
        // serial delivery pairs each scripted response with one batch.
        let chunk_infos = vec![chunk_info_batch_sized(
            "seg-a",
            Some("frame"),
            &[None, Some(10), Some(20)],
            &[true, false, false],
            &[Some(100), Some(100), Some(100)],
            &[8 * 1024 * 1024; 3],
        )];

        // The static chunk is delivered last, which is the whole point: the
        // cursor keys off `CursorKey`, not batch position, so the plan's own
        // ordering (`PreTime` first) does not decide arrival order.
        let responses = [
            FetchBehavior::Chunks(vec![(
                scalar_chunk("seg-a", 1, "frame", Some(10), 1),
                "seg-a".to_owned(),
            )]),
            FetchBehavior::Chunks(vec![(
                scalar_chunk("seg-a", 2, "frame", Some(20), 2),
                "seg-a".to_owned(),
            )]),
            FetchBehavior::Chunks(vec![(
                scalar_chunk_on_component(
                    "seg-a",
                    0,
                    "frame",
                    None,
                    7,
                    re_types_core::ComponentDescriptor::partial("static_value"),
                ),
                "seg-a".to_owned(),
            )]),
        ];

        let (result, batches, _audit) = run_driver_projected(
            chunk_infos,
            "frame",
            responses,
            64 * 1024 * 1024,
            1,
            None,
            schema_with_component("static_value"),
        )
        .await;

        result.expect("driver completes cleanly");
        assert_eq!(
            emitted_segment_ids(&batches),
            vec!["seg-a", "seg-a"],
            "both temporal rows must emit once the static chunk lands",
        );
        assert_eq!(
            batches.len(),
            1,
            "an outstanding static chunk must gate every incremental emit, so all \
             rows come out of the final drain; got {} flushes",
            batches.len(),
        );
        assert_eq!(
            emitted_component_values(&batches, "static_value"),
            vec![Some(7), Some(7)],
            "every row must carry the static value; a null is the silently-wrong \
             output the gate exists to prevent",
        );
    }

    /// Companion to `late_static_chunk_holds_back_every_row`, with the same
    /// three-batch split but every chunk temporal.
    ///
    /// Here the watermark *does* release rows as the horizon advances, so the
    /// rows arrive across several flushes. Without this pair, the single-flush
    /// outcome above would also be what a fixture that happened to deliver
    /// everything before the first emit pass reported.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn temporal_chunks_emit_incrementally() {
        use super::super::test_support::chunk_info_batch_sized;

        let chunk_infos = vec![chunk_info_batch_sized(
            "seg-a",
            Some("frame"),
            &[Some(10), Some(20), Some(30)],
            &[false; 3],
            &[Some(100); 3],
            &[8 * 1024 * 1024; 3],
        )];

        let responses = (0..3).map(|i| {
            let n = i64::try_from(i).unwrap();
            FetchBehavior::Chunks(vec![(
                scalar_chunk("seg-a", i, "frame", Some((n + 1) * 10), n),
                "seg-a".to_owned(),
            )])
        });

        let (result, batches, _audit) =
            run_driver(chunk_infos, "frame", responses, 64 * 1024 * 1024, 1, None).await;

        result.expect("driver completes cleanly");
        assert_eq!(
            emitted_segment_ids(&batches),
            vec!["seg-a", "seg-a", "seg-a"],
            "every temporal row must emit",
        );
        assert!(
            batches.len() > 1,
            "with only temporal chunks outstanding the horizon advances and rows \
             emit incrementally; got a single flush",
        );
    }
}
