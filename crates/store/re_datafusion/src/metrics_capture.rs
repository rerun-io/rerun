//! In-process capture of per-query metrics for programmatic readers.
//!
//! Provides a frozen [`QuerySnapshot`] of one completed query (or a snapshot
//! taken at the moment the last per-partition stream finished) and a
//! [`MetricsCollector`] sink that subscribers attach to a query at plan
//! construction time. Each `SegmentStreamExec` carries the collectors it was
//! built with and pushes a snapshot to each one when its streams complete.
//!
//! The same [`QuerySnapshot`] shape feeds both the OTLP / `PostHog` analytics
//! span built in the crate-private `analytics` module and the Python
//! `query_metrics()` context manager — there is exactly one source of truth
//! (the plan's [`QueryMetrics`]) and one canonical aggregation.

use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use web_time::Duration;

use crate::analytics::{DirectFetchFailureReason, QueryInfo};

/// Canonical source of truth for one query's runtime counters.
///
/// Each `SegmentStreamExec` owns one of these (wrapped in [`Arc`](std::sync::Arc)) and shares
/// it with:
/// - the IO fetch tasks, which `fetch_add` into the counters once per task
///   at flush time (via `TaskFetchStats::flush_into`),
/// - the snapshot path, which reads the atomics via [`build_query_snapshot`]
///   when the last per-partition stream completes,
/// - DataFusion's `ExecutionPlan::metrics()`, which builds an ad-hoc
///   `MetricsSet` on demand from these same atomics for `EXPLAIN ANALYZE`.
///
/// All counters are summed across partitions; `fetch_direct_max_attempt` uses
/// `fetch_max` so it surfaces the true cross-partition max rather than a sum.
#[derive(Debug)]
pub(crate) struct QueryMetrics {
    /// Plan-time facts about the query, recorded once in `scan()` before any
    /// execution. Embedded so `build_query_snapshot` and
    /// `build_metrics_set_for_explain` can both read them without a separate
    /// seeding step into a `MetricsSet`.
    pub query_info: QueryInfo,

    pub fetch_grpc_requests: AtomicU64,
    pub fetch_grpc_bytes: AtomicU64,
    pub fetch_direct_requests: AtomicU64,
    pub fetch_direct_bytes: AtomicU64,
    pub fetch_direct_retries: AtomicU64,
    pub fetch_direct_requests_retried: AtomicU64,
    pub fetch_direct_retry_sleep_us: AtomicU64,
    pub fetch_direct_max_attempt: AtomicU64,
    pub fetch_direct_original_ranges: AtomicU64,
    pub fetch_direct_merged_ranges: AtomicU64,
}

impl QueryMetrics {
    pub(crate) fn new(query_info: QueryInfo) -> Self {
        Self {
            query_info,
            fetch_grpc_requests: AtomicU64::new(0),
            fetch_grpc_bytes: AtomicU64::new(0),
            fetch_direct_requests: AtomicU64::new(0),
            fetch_direct_bytes: AtomicU64::new(0),
            fetch_direct_retries: AtomicU64::new(0),
            fetch_direct_requests_retried: AtomicU64::new(0),
            fetch_direct_retry_sleep_us: AtomicU64::new(0),
            fetch_direct_max_attempt: AtomicU64::new(0),
            fetch_direct_original_ranges: AtomicU64::new(0),
            fetch_direct_merged_ranges: AtomicU64::new(0),
        }
    }
}

/// One completed query's metrics, as seen from the client side.
///
/// Built once via `build_query_snapshot` and consumed by:
/// - the crate-private `analytics::PendingInner::drop` when sending the
///   `PostHog` OTLP span (so the span attributes are guaranteed to match what
///   other readers see),
/// - each subscribed [`MetricsCollector`] at end-of-stream, so Python code can
///   read the metrics back without going through the `datafusion_ffi`
///   `EXPLAIN ANALYZE` path that currently strips them.
#[derive(Clone, Debug, PartialEq)]
pub struct QuerySnapshot {
    /// Plan-time facts about the query, recorded once in `scan()` before any
    /// execution. Embedded as-is so a new `QueryInfo` field automatically
    /// flows through to every snapshot consumer.
    pub query_info: QueryInfo,

    // ---- Execution-time ----------------------------------------------------
    /// Wall-clock time from the start of `scan()` until the query finished
    /// (cleanly or via error). Always populated.
    pub total_duration: Duration,

    /// Time from `scan_start` until the first chunk reached the consumer.
    /// `None` when no chunk was ever delivered (e.g. early error, empty
    /// result).
    pub time_to_first_chunk: Option<Duration>,

    /// `None` on success. On failure, one of the stable
    /// `QueryErrorKind` string labels
    /// (`"grpc_fetch"`, `"direct_fetch"`, `"decode"`, `"other"`).
    pub error_kind: Option<&'static str>,

    /// Reason a direct (HTTP Range) fetch hit a terminal failure — i.e. a
    /// non-retryable error or retries exhausted. `None` when no direct fetch
    /// terminally failed (this can be `None` even when `error_kind` is set,
    /// if the failure was on the gRPC or decode path).
    pub direct_terminal_reason: Option<DirectFetchFailureReason>,

    // ---- Fetch counters (summed across partitions) -------------------------
    //
    // "gRPC" counters cover `FetchChunks` / fast-path gRPC fetches; "direct"
    // counters cover HTTP Range fetches against the underlying object store.
    //
    // The `_bytes` counters here are **not** measured at the wire — they are
    // sums of the catalog-reported `chunk_byte_length` over the chunks that
    // went down each path. That makes them a clean proxy for "useful payload
    // transferred," but they exclude HTTP/gRPC framing overhead, bytes pulled
    // by range-merging filler, and bytes consumed by failed retry attempts.
    // Treat them as a lower bound on actual network traffic, not an exact
    // measurement.
    /// Number of gRPC fetch calls the scanner issued.
    pub fetch_grpc_requests: u64,

    /// Sum of `chunk_byte_length` (catalog metadata, compressed on-disk size)
    /// over chunks fetched via gRPC. See the section comment above for what
    /// this is and isn't.
    pub fetch_grpc_bytes: u64,

    /// Number of direct (HTTP Range) fetches the scanner issued. Counts each
    /// merged request once, regardless of how many byte ranges it carried or
    /// how many attempts it took.
    pub fetch_direct_requests: u64,

    /// Sum of `chunk_byte_length` (catalog metadata, compressed on-disk size)
    /// over chunks fetched via direct HTTP. Notably this does **not** count
    /// the filler bytes that range-merging pulls between adjacent chunks in
    /// the same object, so actual wire traffic can exceed this value.
    pub fetch_direct_bytes: u64,

    /// Total number of direct-fetch retry *attempts* across all requests.
    /// Each retry of a single request adds 1; a request retried 3 times
    /// contributes 3 here.
    pub fetch_direct_retries: u64,

    /// Number of distinct direct-fetch requests that needed at least one
    /// retry. Always `≤ fetch_direct_retries`; the ratio between them is the
    /// average retries per retried request.
    pub fetch_direct_requests_retried: u64,

    /// Total backoff time slept across all direct-fetch retries
    pub fetch_direct_retry_sleep: Duration,

    /// True cross-partition max attempt number (via `AtomicU64::fetch_max`).
    pub fetch_direct_max_attempt: u64,

    /// Number of byte ranges the planner *wanted* to fetch directly, before
    /// adjacent ranges were coalesced. With `fetch_direct_merged_ranges`,
    /// gives the range-merging ratio.
    pub fetch_direct_original_ranges: u64,

    /// Number of byte ranges actually issued after merging adjacent ranges
    /// into combined HTTP Range requests. Equals `fetch_direct_requests` for
    /// a single-range-per-request scanner.
    pub fetch_direct_merged_ranges: u64,
}

/// Build a [`QuerySnapshot`] from the canonical [`QueryMetrics`] source.
///
/// Reading is a small set of relaxed atomic loads + a clone of `query_info`
/// — no Mutex, no aggregation walk.
pub(crate) fn build_query_snapshot(
    metrics: &QueryMetrics,
    total_duration: Duration,
    time_to_first_chunk: Option<Duration>,
    error_kind: Option<&'static str>,
    direct_terminal_reason: Option<DirectFetchFailureReason>,
) -> QuerySnapshot {
    let load = |a: &AtomicU64| a.load(Ordering::Relaxed);
    QuerySnapshot {
        query_info: metrics.query_info.clone(),

        total_duration,
        time_to_first_chunk,
        error_kind,
        direct_terminal_reason,

        fetch_grpc_requests: load(&metrics.fetch_grpc_requests),
        fetch_grpc_bytes: load(&metrics.fetch_grpc_bytes),
        fetch_direct_requests: load(&metrics.fetch_direct_requests),
        fetch_direct_bytes: load(&metrics.fetch_direct_bytes),
        fetch_direct_retries: load(&metrics.fetch_direct_retries),
        fetch_direct_requests_retried: load(&metrics.fetch_direct_requests_retried),
        fetch_direct_retry_sleep: Duration::from_micros(load(&metrics.fetch_direct_retry_sleep_us)),
        fetch_direct_max_attempt: load(&metrics.fetch_direct_max_attempt),
        fetch_direct_original_ranges: load(&metrics.fetch_direct_original_ranges),
        fetch_direct_merged_ranges: load(&metrics.fetch_direct_merged_ranges),
    }
}

// ----------------------------------------------------------------------------
// MetricsCollector

/// Sink for [`QuerySnapshot`]s collected during a `query_metrics()` scope.
///
/// `Clone` is cheap — the receiver buffer is held behind an `Arc<Mutex<...>>`
/// — and clones share state. The expected ownership pattern is: one clone
/// is attached to each `DataframeQueryTableProvider` that observes this scope
/// (via the Python `ContextVar` read in `dataset_view.rs::reader()`), and the
/// same buffer is exposed to Python through `_MetricsCollectorHandle`.
#[derive(Clone, Debug)]
pub struct MetricsCollector {
    inner: std::sync::Arc<Mutex<Vec<QuerySnapshot>>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Allocate a fresh collector with an empty buffer.
    pub fn new() -> Self {
        Self {
            inner: std::sync::Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Take and clear all queued snapshots.
    pub fn drain(&self) -> Vec<QuerySnapshot> {
        let mut guard = self.inner.lock();
        std::mem::take(&mut *guard)
    }

    /// Non-destructive copy of the current buffer.
    pub fn snapshot(&self) -> Vec<QuerySnapshot> {
        let guard = self.inner.lock();
        guard.clone()
    }

    fn push(&self, snapshot: QuerySnapshot) {
        let mut guard = self.inner.lock();
        guard.push(snapshot);
    }
}

/// Push a snapshot to each collector in the list. Used by
/// `SegmentStreamExec`'s stream-completion hook.
pub fn push_snapshot(collectors: &[MetricsCollector], snapshot: &QuerySnapshot) {
    for c in collectors {
        c.push(snapshot.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::QueryType;

    fn dummy_query_info() -> QueryInfo {
        QueryInfo {
            dataset_id: "ds-test".to_owned(),
            query_chunks: 10,
            query_segments: 2,
            query_layers: 1,
            query_columns: 4,
            query_entities: 1,
            query_bytes: 1024,
            query_chunks_per_segment_min: 5,
            query_chunks_per_segment_max: 5,
            query_chunks_per_segment_mean: 5.0,
            query_type: QueryType::LatestAt,
            primary_index_name: Some("time".to_owned()),
            time_to_first_chunk_info: None,
            trace_id: None,
            filters_pushed_down: 1,
            filters_applied_client_side: 0,
            entity_path_narrowing_applied: true,
            filters_total: 0,
            filters_signatures: String::new(),
            filters_signatures_exact: String::new(),
            filters_signatures_inexact: String::new(),
            filters_signatures_unsupported: String::new(),
        }
    }

    #[test]
    fn build_query_snapshot_reads_atomic_counters() {
        let metrics = QueryMetrics::new(dummy_query_info());
        // Simulate two partitions each flushing into the shared atomics.
        metrics.fetch_grpc_bytes.fetch_add(1_000, Ordering::Relaxed);
        metrics.fetch_grpc_bytes.fetch_add(2_500, Ordering::Relaxed);
        metrics
            .fetch_direct_requests
            .fetch_add(3, Ordering::Relaxed);

        let snap = build_query_snapshot(&metrics, Duration::from_millis(42), None, None, None);

        assert_eq!(snap.fetch_grpc_bytes, 3_500);
        assert_eq!(snap.fetch_direct_requests, 3);
        assert_eq!(snap.fetch_grpc_requests, 0);
        assert_eq!(snap.query_info.dataset_id, "ds-test");
        assert_eq!(snap.total_duration, Duration::from_millis(42));
        assert!(snap.query_info.entity_path_narrowing_applied);
    }

    /// Regression: the in-process snapshot path used to pass `None` for
    /// `time_to_first_chunk` and `direct_terminal_reason`, so Python's
    /// `QueryMetrics` always saw `None`. Confirm both flow through now.
    #[test]
    fn build_query_snapshot_forwards_optional_exec_fields() {
        let metrics = QueryMetrics::new(dummy_query_info());
        let snap = build_query_snapshot(
            &metrics,
            Duration::from_micros(999),
            Some(Duration::from_micros(123)),
            Some("direct_fetch"),
            Some(DirectFetchFailureReason::Http5xx),
        );

        assert_eq!(snap.time_to_first_chunk, Some(Duration::from_micros(123)));
        assert_eq!(snap.error_kind, Some("direct_fetch"));
        assert_eq!(
            snap.direct_terminal_reason,
            Some(DirectFetchFailureReason::Http5xx)
        );
    }

    /// Round-trip a snapshot through `push_snapshot` and confirm collectors
    /// receive independent copies.
    #[test]
    fn push_snapshot_fans_out_to_each_collector() {
        let a = MetricsCollector::new();
        let b = MetricsCollector::new();
        let metrics = QueryMetrics::new(dummy_query_info());
        let snap = build_query_snapshot(&metrics, Duration::from_micros(100), None, None, None);

        push_snapshot(&[a.clone(), b.clone()], &snap);

        assert_eq!(a.snapshot().len(), 1);
        assert_eq!(b.snapshot().len(), 1);

        // Draining one is independent of the other.
        assert_eq!(a.drain().len(), 1);
        assert_eq!(a.snapshot().len(), 0);
        assert_eq!(b.snapshot().len(), 1);
    }
}
