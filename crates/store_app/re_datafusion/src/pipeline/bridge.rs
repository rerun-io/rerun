//! The single bridge between the v2 tree and DataFusion: spawn the driver
//! onto the process-global CPU runtime, hand DataFusion a
//! `SendableRecordBatchStream` fed by one channel.
//!
//! Errors travel in-band (an error slot the driver fills before exiting),
//! so `poll_next` has no per-poll join-handle harvesting — the one
//! `JoinHandle` await runs at end-of-stream, where it is what makes the
//! error slot and any panic observable at all.
//! Dropping the stream aborts the driver task (`AbortOnDropHandle`), which
//! unwinds the whole tree: in-flight fetches abort, segment ledgers refund.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll};

use arrow::array::RecordBatch;
use arrow::datatypes::{Schema, SchemaRef};
use datafusion::error::DataFusionError;
use datafusion::execution::{RecordBatchStream, SendableRecordBatchStream};
use futures::FutureExt as _;
use futures_util::Stream;
use re_dataframe::QueryExpression;
use re_redap_client::ApiError;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::task::AbortOnDropHandle;
use tracing::Instrument as _;

use super::FetchConfig;
use super::driver::PartitionDriver;
use super::executor::FetchExecutor;
use super::plan::build_fetch_plan;
use super::window::IssuanceWindow;
use crate::IntoDfError as _;
use crate::analytics::QueryErrorKind;
use crate::chunk_fetcher::{DIRECT_FETCH_HTTP_CLIENT, ObjectStoreAuthenticator};
use crate::dataframe_query_common::{DataframeClientAPI, IndexValuesMap};
use crate::dataframe_query_provider::fetch_plan::TARGET_BATCH_SIZE_BYTES;
use crate::dataframe_query_provider::{
    CpuRuntime, attach_trace_context, emit_snapshot_drop_fallback, emit_snapshot_if_last_partition,
    fail_partition_setup,
};

/// Everything `execute_partition` needs from `SegmentStreamExec::execute`.
pub struct V2PartitionCtx<T> {
    /// One batch per segment, `SegmentId` ASC, already filtered to this
    /// DataFusion partition. Non-empty (the empty-partition early-return
    /// happens in shared `execute` code).
    pub chunk_infos: Vec<RecordBatch>,
    pub query_expression: QueryExpression,
    pub index_values: IndexValuesMap,
    pub projected_schema: Arc<Schema>,
    pub limit_rows: Option<usize>,
    pub client: T,
    pub object_store_auth: Arc<dyn ObjectStoreAuthenticator>,

    /// The IO runtime for network spawns; the CPU runtime this tree runs on
    /// has no reactor. See `SegmentStreamExec::io_handle`.
    pub io_handle: tokio::runtime::Handle,
    pub config: FetchConfig,

    pub trace_headers: Option<crate::TraceHeaders>,
    pub server_trace_id: Option<re_redap_client::TraceId>,
    pub pending_analytics: crate::PendingQueryAnalytics,
    pub captured_collectors: Vec<crate::MetricsCollector>,
    pub partitions_remaining: Arc<AtomicUsize>,
    pub snapshot_sent: Arc<AtomicBool>,
}

pub fn execute_partition<T: DataframeClientAPI>(
    ctx: V2PartitionCtx<T>,
) -> datafusion::common::Result<SendableRecordBatchStream> {
    re_tracing::profile_function!();

    let V2PartitionCtx {
        chunk_infos,
        query_expression,
        index_values,
        projected_schema,
        limit_rows,
        client,
        object_store_auth,
        io_handle,
        config,
        trace_headers,
        server_trace_id,
        pending_analytics,
        captured_collectors,
        partitions_remaining,
        snapshot_sent,
    } = ctx;

    let origin = client.origin().clone();

    let setup = CpuRuntime::try_get().and_then(|cpu_runtime| {
        let plan = build_fetch_plan(
            &origin,
            chunk_infos,
            query_expression.filtered_index,
            &index_values,
            TARGET_BATCH_SIZE_BYTES as u64,
        )
        .map_err(|err| err.with_trace_id(server_trace_id).into_df_error())?;
        Ok((cpu_runtime, plan))
    });
    let (cpu_runtime, plan) = setup.map_err(|err| {
        fail_partition_setup(
            err,
            &captured_collectors,
            &partitions_remaining,
            &snapshot_sent,
            &pending_analytics,
        )
    })?;

    let metrics = Arc::clone(pending_analytics.metrics());
    metrics
        .planned_fetch_batches
        .fetch_add(plan.batches.len() as u64, Ordering::Relaxed);

    if plan.batches.is_empty() {
        // Everything filtered out (e.g. by index_values): same contract as
        // the shared empty-partition branch — count this partition down.
        emit_snapshot_if_last_partition(
            &captured_collectors,
            &partitions_remaining,
            &snapshot_sent,
            &pending_analytics,
        );
        return Ok(Box::pin(V2PartitionStream {
            projected_schema,
            inner: None,
        }));
    }

    let executor = FetchExecutor {
        client,
        http_client: DIRECT_FETCH_HTTP_CLIENT.clone(),
        object_store_auth,
        io_handle,
        window: Arc::new(IssuanceWindow::new(config.window_bytes)),
        front: Arc::new(super::executor::DeliveryFront::default()),
        metrics,
        pending_analytics: pending_analytics.clone(),
    };
    let driver = PartitionDriver {
        plan: Arc::new(plan),
        executor,
        fetch_concurrency: config.fetch_concurrency,
        origin,
        query_expression,
        index_values,
        projected_schema: Arc::clone(&projected_schema),
        limit_rows,
    };

    let (tx, rx) = mpsc::channel::<RecordBatch>(OUTPUT_CHANNEL_SIZE);
    let (err_tx, err_rx) = oneshot::channel::<ApiError>();

    let cpu_task = cpu_runtime.handle().spawn(
        async move {
            if let Err(err) = driver.run(tx).await {
                // In-band error delivery; the receiver may already be
                // gone (stream dropped), which is fine.
                err_tx.send(err).ok();
            }
        }
        .instrument(tracing::info_span!("pipeline_v2_partition")),
    );

    Ok(Box::pin(V2PartitionStream {
        projected_schema,
        inner: Some(V2Inner {
            rx,
            err_rx,
            cpu_task: AbortOnDropHandle::new(cpu_task),
            trace_headers,
            server_trace_id,
            pending_analytics,
            captured_collectors,
            partitions_remaining,
            snapshot_sent,
            completed: false,
        }),
    }))
}

/// Pipelining depth between the driver's emit loop and the DataFusion
/// consumer. Emitted row batches are not window-accounted; this capacity
/// bounds them. 32 matches the v1 pipeline's row-output channel, so both
/// pipelines buffer the same number of emitted batches.
const OUTPUT_CHANNEL_SIZE: usize = 32;

struct V2Inner {
    rx: mpsc::Receiver<RecordBatch>,
    err_rx: oneshot::Receiver<ApiError>,

    /// Dropping this aborts the driver task, which unwinds the whole tree.
    /// Awaited once at end-of-stream: see [`V2PartitionStream::poll_next`].
    cpu_task: AbortOnDropHandle<()>,

    trace_headers: Option<crate::TraceHeaders>,
    server_trace_id: Option<re_redap_client::TraceId>,
    pending_analytics: crate::PendingQueryAnalytics,
    captured_collectors: Vec<crate::MetricsCollector>,
    partitions_remaining: Arc<AtomicUsize>,
    snapshot_sent: Arc<AtomicBool>,

    /// See `DataframeSegmentStreamInner::completed` — distinguishes clean
    /// end-of-stream from an early drop for the snapshot fallback.
    completed: bool,
}

/// Taking `inner` at end-of-stream releases the whole tree even when the FFI
/// consumer never drops the stream (RR-4607).
pub struct V2PartitionStream {
    projected_schema: SchemaRef,
    inner: Option<V2Inner>,
}

impl Stream for V2PartitionStream {
    type Item = Result<RecordBatch, DataFusionError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this_outer = self.get_mut();
        let Some(this) = this_outer.inner.as_mut() else {
            return Poll::Ready(None);
        };

        let _trace_guard = attach_trace_context(this.trace_headers.as_ref());

        match this.rx.poll_recv(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(batch)) => {
                // As close to user-perceived first-result latency as we get.
                this.pending_analytics.record_first_chunk();

                // Payload actually handed to the consumer — post-decode,
                // post-query, post-client-filter. Counted here rather than in
                // the driver because only this side sees what survives to the
                // consumer. `waste_ratio` divides fetched bytes by these, so
                // leaving them at zero reports "fetched everything, delivered
                // nothing" rather than reporting nothing.
                let metrics = this.pending_analytics.metrics();
                metrics
                    .delivered_rows
                    .fetch_add(batch.num_rows() as u64, Ordering::Relaxed);
                metrics
                    .delivered_bytes
                    .fetch_add(batch.get_array_memory_size() as u64, Ordering::Relaxed);

                Poll::Ready(Some(Ok(batch)))
            }
            Poll::Ready(None) => {
                // A closed row channel says nothing about *how* the driver
                // ended: `run` owns `tx` and drops it when its future
                // completes, which is before the task fills the error slot
                // and before the runtime marks the task finished. The task is
                // already finishing, so this await is bounded.
                let join_result = match this.cpu_task.poll_unpin(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(join_result) => join_result,
                };

                // Terminal from here on. Take `inner` so a repeat poll
                // short-circuits on the guard above rather than polling an
                // already-completed `JoinHandle`, which panics.
                let Some(mut inner) = this_outer.inner.take() else {
                    return Poll::Ready(None);
                };

                // The driver has exited, so the slot is settled: `Ok` iff it
                // failed. On this path `inner`'s `Drop` emits the snapshot
                // fallback, matching any other mid-stream termination.
                if let Ok(err) = inner.err_rx.try_recv() {
                    // Only a stream still being polled reaches this, so a
                    // consumer hangup, which also ends the driver with an
                    // error, is never labeled. The first recorded kind wins,
                    // and fetch failures and under-delivery record their own,
                    // so this labels only failures in the store insert or
                    // emit path.
                    inner.pending_analytics.record_error(QueryErrorKind::Decode);
                    return Poll::Ready(Some(Err(err
                        .with_trace_id(inner.server_trace_id)
                        .into_df_error())));
                }

                // A panic closes both channels without ever filling the slot.
                // Cancellation is unreachable while we hold the handle (we
                // only abort from our own `Drop`), but reporting it beats
                // silently truncating if that ever stops being true.
                if let Err(join_err) = join_result {
                    inner.pending_analytics.record_error(QueryErrorKind::Other);
                    return Poll::Ready(Some(datafusion::common::exec_err!(
                        "pipeline v2 driver task failed: {join_err}"
                    )));
                }

                inner.completed = true;
                emit_snapshot_if_last_partition(
                    &inner.captured_collectors,
                    &inner.partitions_remaining,
                    &inner.snapshot_sent,
                    &inner.pending_analytics,
                );
                Poll::Ready(None)
            }
        }
    }
}

impl RecordBatchStream for V2PartitionStream {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.projected_schema)
    }
}

impl Drop for V2Inner {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        // Dropped mid-stream (LIMIT / cancellation): emit the metrics
        // snapshot eagerly with whatever has been recorded so far.
        emit_snapshot_drop_fallback(
            &self.captured_collectors,
            &self.snapshot_sent,
            &self.pending_analytics,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use arrow::array::RecordBatch;

    use super::super::test_support::{
        FetchBehavior, chunk_info_batch, chunk_info_batch_sized, run_bridge, scalar_chunk,
    };

    fn one_chunk_segment() -> Vec<RecordBatch> {
        vec![chunk_info_batch(
            "seg-a",
            Some("frame"),
            &[Some(10)],
            &[false],
            &[Some(100)],
        )]
    }

    /// A partition whose plan cannot be built still has to count itself down.
    ///
    /// The failure happens before any `V2PartitionStream` exists, so neither
    /// the end-of-stream path nor `V2Inner::drop` runs for it. If it also
    /// skipped the countdown, `partitions_remaining` could never reach zero
    /// and the clean snapshot path would be dead for the whole query.
    ///
    /// Two chunk-info batches for one segment is the planner's own hard
    /// error — it refuses to split a segment across fetch batches.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unbuildable_plan_still_counts_the_partition_down() {
        let duplicated_segment = vec![
            chunk_info_batch("seg-a", Some("frame"), &[Some(10)], &[false], &[Some(100)]),
            chunk_info_batch("seg-a", Some("frame"), &[Some(20)], &[false], &[Some(100)]),
        ];

        let (result, remaining, error_kind) = run_bridge(duplicated_segment, "frame", [], 1).await;

        let err = result.expect_err("an unbuildable plan must surface as an error");
        assert!(
            format!("{err}").contains("spans multiple chunk_info batches"),
            "error should carry the planner's cause, got: {err}",
        );
        assert_eq!(
            remaining.load(Ordering::Relaxed),
            0,
            "a partition that fails during setup must still count itself down",
        );
        assert_eq!(error_kind, Some("other"));
    }

    /// A driver panic must reach the consumer as an error, not as a clean
    /// end-of-stream.
    ///
    /// Unwinding the driver drops the whole tree — in-flight fetches, every
    /// segment store, every ledger — so the task stays unfinished for far
    /// longer than the consumer takes to wake on the closed row channel, and
    /// the rows emitted before the panic would be reported as a complete
    /// result set. The task here closes its row channel, then stays alive
    /// before panicking, to hold that window open.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn driver_panic_after_the_row_channel_closes_reaches_the_consumer() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, AtomicUsize};

        use arrow::array::{RecordBatchOptions, StringArray};
        use futures::StreamExt as _;
        use tokio::sync::{mpsc, oneshot};
        use tokio_util::task::AbortOnDropHandle;

        use super::super::test_support::{segment_id_only_schema, test_pending_analytics};
        use super::{V2Inner, V2PartitionStream};

        let schema = segment_id_only_schema();
        let batch = RecordBatch::try_new_with_options(
            Arc::clone(&schema),
            vec![Arc::new(StringArray::from(vec!["seg-a"]))],
            &RecordBatchOptions::new().with_row_count(Some(1)),
        )
        .unwrap();

        let pending_analytics = test_pending_analytics();
        let (tx, rx) = mpsc::channel(1);
        let (err_tx, err_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            // Held, never filled: a panic closes the error slot empty.
            let _err_tx = err_tx;
            tx.send(batch).await.ok();
            drop(tx);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            panic!("injected driver panic");
        });

        let mut stream = V2PartitionStream {
            projected_schema: schema,
            inner: Some(V2Inner {
                rx,
                err_rx,
                cpu_task: AbortOnDropHandle::new(task),
                trace_headers: None,
                server_trace_id: None,
                pending_analytics: pending_analytics.clone(),
                captured_collectors: vec![crate::MetricsCollector::new()],
                partitions_remaining: Arc::new(AtomicUsize::new(1)),
                snapshot_sent: Arc::new(AtomicBool::new(false)),
                completed: false,
            }),
        };

        let first = stream.next().await.expect("the emitted row arrives");
        assert_eq!(
            first.expect("the emitted row is not an error").num_rows(),
            1
        );

        let err = stream
            .next()
            .await
            .expect("a panic must not read as end-of-stream")
            .expect_err("a driver panic must surface as an error");
        assert!(
            format!("{err}").contains("injected driver panic"),
            "error should carry the panic, got: {err}",
        );
        assert_eq!(pending_analytics.error_kind(), Some("other"));
        assert!(
            stream.next().await.is_none(),
            "polling past termination must keep yielding end-of-stream",
        );
    }

    /// Six chunks, each at the 8 MiB packing target so the planner emits one
    /// fetch batch per chunk. Lets a fixture deliver rows for a while and
    /// *then* fail, which is the ordering that puts the consumer mid-drain —
    /// actively polling — when the driver exits.
    fn six_batch_segment() -> Vec<RecordBatch> {
        let starts: Vec<Option<i64>> = (1..=6).map(|i| Some(i * 10)).collect();
        vec![chunk_info_batch_sized(
            "seg-a",
            Some("frame"),
            &starts,
            &[false; 6],
            &[Some(8 * 1024 * 1024); 6],
            &[8 * 1024 * 1024; 6],
        )]
    }

    /// A fetch failure must reach the DataFusion consumer.
    ///
    /// The driver drops the row channel when its future completes, which is
    /// before it fills the error slot and before the runtime marks the task
    /// finished — so the stream observes a closed channel first. Treating
    /// that as end-of-stream would return a short result set indistinguishable
    /// from a legitimate empty tail.
    ///
    /// Looped because the failure mode is a race between the driver task and
    /// the polling thread.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fetch_error_reaches_the_consumer() {
        for attempt in 0..32 {
            // Five batches land before the sixth fails, so the consumer is
            // mid-drain rather than parked when the driver exits — the
            // ordering that actually exercises the window between the row
            // channel closing and the outcome becoming knowable.
            let responses = std::iter::chain(
                (0..5).map(|i| {
                    let n = i64::try_from(i).unwrap();
                    FetchBehavior::Chunks(vec![(
                        scalar_chunk("seg-a", i, "frame", Some((n + 1) * 10), n + 1),
                        "seg-a".to_owned(),
                    )])
                }),
                [FetchBehavior::Fail(tonic::Status::unavailable(
                    "injected fetch failure",
                ))],
            );

            let (result, _remaining, error_kind) =
                run_bridge(six_batch_segment(), "frame", responses, 4).await;

            let err = result.err().unwrap_or_else(|| {
                panic!("a failed fetch must surface as an error (attempt {attempt})")
            });
            assert!(
                format!("{err}").contains("injected fetch failure"),
                "error should carry the transport cause, got: {err}",
            );
            assert_eq!(
                error_kind,
                Some("grpc_fetch"),
                "the transport kind must survive the bridge's store-path fallback",
            );
        }
    }

    /// Under-delivery is a data-integrity failure, not a store-path one, so
    /// its label must survive the bridge's `Decode` fallback.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn under_delivery_is_labeled_incomplete() {
        let (result, _remaining, error_kind) = run_bridge(
            one_chunk_segment(),
            "frame",
            [FetchBehavior::Chunks(vec![])],
            1,
        )
        .await;

        let err = result.expect_err("an under-delivered segment must fail the query");
        assert!(
            format!("{err}").contains("under-delivered"),
            "error should carry the under-delivery, got: {err}",
        );
        assert_eq!(error_kind, Some("incomplete"));
    }

    /// The clean path still terminates, delivers its rows, and counts the
    /// partition down so the analytics snapshot can fire.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn clean_end_of_stream_counts_the_partition_down() {
        let (result, remaining, _error_kind) = run_bridge(
            one_chunk_segment(),
            "frame",
            [FetchBehavior::Chunks(vec![(
                scalar_chunk("seg-a", 0, "frame", Some(10), 1),
                "seg-a".to_owned(),
            )])],
            1,
        )
        .await;

        let batches = result.expect("driver completes cleanly");
        assert_eq!(
            batches.iter().map(RecordBatch::num_rows).sum::<usize>(),
            1,
            "the single planned row must be delivered",
        );
        assert_eq!(
            remaining.load(Ordering::Relaxed),
            0,
            "end-of-stream must count the partition down",
        );
    }
}
