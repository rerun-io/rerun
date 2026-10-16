//! The fetch executor: turns [`PlannedBatch`]es into delivered chunks.
//!
//! One future per planned batch: acquire the batch's estimate from the
//! issuance window (FIFO ⇒ plan order under contention), spawn the
//! fetch+decode onto the IO runtime, await it. The driver runs these through
//! `buffered(N)` so deliveries arrive in plan order while N requests are in
//! flight.
//!
//! Transport is the unchanged `chunk_fetcher` layer: gRPC `FetchChunks` and
//! direct-URL range reads, including the process-global direct-fetch
//! semaphore. There is no per-fetch byte reservation: the window lease
//! acquired here *is* the accounting, and it travels inside the
//! [`Delivered`] value until the driver splits it into per-segment ledgers.

use std::sync::Arc;

use re_dataframe::external::re_chunk::Chunk;
use re_redap_client::{ApiError, ApiResult};
use re_types_core::SegmentId;
use tokio_util::task::AbortOnDropHandle;
use tracing::Instrument as _;

use super::plan::{FetchTransport, PlannedBatch};
use super::window::{IssuanceWindow, WindowLease};
use crate::analytics::{QueryErrorKind, TaskFetchStats};
use crate::chunk_fetcher::{
    ChunksWithSegment, ObjectStoreAuthenticator, fetch_batch_direct, fetch_batch_group_via_grpc,
};
use crate::dataframe_query_common::DataframeClientAPI;
use crate::metrics_capture::{InflightFetchGuard, QueryMetrics};

/// One completed fetch: the planned batch, the window bytes it holds, and
/// the decoded chunks.
pub struct Delivered {
    /// The issuance-window bytes acquired for this batch. The driver splits
    /// per-chunk shares into segment ledgers and drops the residual.
    pub lease: WindowLease,
    pub chunks: Vec<(Chunk, Option<SegmentId>)>,
}

/// Which batch the driver is currently blocked on.
///
/// Deliveries are consumed in plan order, so exactly one batch is ever the
/// one holding everything else up. That batch takes its window bytes without
/// waiting ([`IssuanceWindow::acquire_priority`]); every other batch is gated
/// normally. Since the front always completes, the front always advances —
/// which is the pipeline's forward-progress guarantee.
#[derive(Default)]
pub struct DeliveryFront {
    idx: std::sync::atomic::AtomicUsize,
    advanced: tokio::sync::Notify,
}

impl DeliveryFront {
    /// Called by the driver once it has consumed a delivery; `idx` is the
    /// batch it will block on next.
    pub fn advance_to(&self, idx: usize) {
        self.idx.store(idx, std::sync::atomic::Ordering::Release);
        self.advanced.notify_waiters();
    }

    fn is_front(&self, task_idx: usize) -> bool {
        self.idx.load(std::sync::atomic::Ordering::Acquire) >= task_idx
    }
}

/// Everything a fetch future needs, cloneable per batch.
pub struct FetchExecutor<T> {
    pub client: T,
    pub http_client: reqwest::Client,
    pub object_store_auth: Arc<dyn ObjectStoreAuthenticator>,

    /// The IO runtime to spawn network calls onto. The whole v2 tree runs
    /// on the CPU runtime (which has no reactor), so every network call is
    /// spawned here and awaited.
    pub io_handle: tokio::runtime::Handle,
    pub window: Arc<IssuanceWindow>,
    pub front: Arc<DeliveryFront>,
    pub metrics: Arc<QueryMetrics>,
    pub pending_analytics: crate::PendingQueryAnalytics,
}

impl<T: DataframeClientAPI> FetchExecutor<T> {
    /// The future for one planned batch: window acquire → spawned
    /// fetch+decode on the IO runtime → delivery.
    ///
    /// The spawned task is wrapped in [`AbortOnDropHandle`]: when the driver
    /// (or the whole tree) is dropped, in-flight fetches are aborted instead
    /// of detaching and downloading to completion.
    pub fn issue(
        &self,
        task_idx: usize,
        batch: Arc<PlannedBatch>,
    ) -> impl Future<Output = ApiResult<Delivered>> + Send + use<T> {
        let client = self.client.clone();
        let http_client = self.http_client.clone();
        let object_store_auth = Arc::clone(&self.object_store_auth);
        let io_handle = self.io_handle.clone();
        let window = Arc::clone(&self.window);
        let front = Arc::clone(&self.front);
        let metrics = Arc::clone(&self.metrics);
        let pending_analytics = self.pending_analytics.clone();
        let origin = client.origin().clone();

        async move {
            // Wait for window bytes until this batch becomes the front, which must not wait.
            // The gated acquire stays pinned across front advances so it keeps its FIFO
            // position and any partially assigned permits; it is dropped before the priority
            // acquire so unclaimed permits return to the window rather than adding to the
            // overdraft.
            let gated_lease = {
                let gated = window.acquire(batch.est_bytes);
                tokio::pin!(gated);
                loop {
                    let advanced = front.advanced.notified();
                    if front.is_front(task_idx) {
                        break None;
                    }
                    tokio::select! {
                        lease = &mut gated => break Some(lease),
                        () = advanced => {}
                    }
                }
            };
            let lease = gated_lease.unwrap_or_else(|| window.acquire_priority(batch.est_bytes));

            let fetch_batch = Arc::clone(&batch);
            let task_origin = origin.clone();
            let join_analytics = pending_analytics.clone();
            let handle = AbortOnDropHandle::new(
                io_handle.spawn(
                    async move {
                        // One unit per planned batch, which may fetch over
                        // both transports in turn.
                        let _inflight = InflightFetchGuard::enter(&metrics);

                        // Task-local stats buffer — flushed once to the shared
                        // atomics at the end of this task to avoid cross-core
                        // cache-line contention on the hot counters.
                        let mut stats = TaskFetchStats::default();
                        let mut chunks: Vec<ChunksWithSegment> = Vec::new();

                        // A batch carries one request per transport its rows
                        // use; fetch each in turn onto the same task.
                        for request in &fetch_batch.requests {
                            match request.transport {
                                FetchTransport::Direct => {
                                    match fetch_batch_direct(
                                        &task_origin,
                                        &request.request,
                                        &http_client,
                                        &metrics.fetch_direct_requests,
                                        &mut stats,
                                        &pending_analytics,
                                        object_store_auth.as_ref(),
                                    )
                                    .await
                                    {
                                        Ok(direct_chunks) => chunks.extend(direct_chunks),
                                        Err(err) => {
                                            stats.try_flush_into(
                                                &pending_analytics,
                                                Err(QueryErrorKind::DirectFetch),
                                            );
                                            return Err(err);
                                        }
                                    }
                                }
                                FetchTransport::Grpc => {
                                    match fetch_batch_group_via_grpc(
                                        std::slice::from_ref(&request.request),
                                        &client,
                                        &metrics.fetch_grpc_requests,
                                        &mut stats,
                                    )
                                    .await
                                    {
                                        Ok(grpc_chunks) => chunks.extend(grpc_chunks),
                                        Err(err) => {
                                            stats.try_flush_into(
                                                &pending_analytics,
                                                Err(QueryErrorKind::GrpcFetch),
                                            );
                                            return Err(err);
                                        }
                                    }
                                }
                            }
                        }

                        stats.try_flush_into(&pending_analytics, Ok(()));
                        Ok::<_, ApiError>(chunks)
                    }
                    .instrument(tracing::info_span!("fetch_task", task_idx)),
                ),
            );

            let chunks = match handle.await {
                Ok(result) => result?,
                Err(join_err) => {
                    join_analytics.record_error(QueryErrorKind::Other);
                    return Err(if join_err.is_panic() {
                        ApiError::internal(
                            &origin,
                            format!("chunk fetch task panicked: {join_err}"),
                        )
                    } else {
                        // Unreachable while we own the handle (abort only
                        // happens via our own Drop), but map it anyway.
                        ApiError::internal(
                            &origin,
                            format!("chunk fetch task cancelled: {join_err}"),
                        )
                    });
                }
            };

            Ok(Delivered {
                lease,
                chunks: chunks.into_iter().flatten().collect(),
            })
        }
    }
}
