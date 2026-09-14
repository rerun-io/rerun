//! Per-connection analytics for dataset queries.
//!
//! Each connection to a Rerun Hub instance gets its own analytics sender
//! that forwards OTLP trace events to that instance's OTEL ingest endpoint.
//! This ensures analytics go to the correct cloud when the viewer is connected
//! to multiple clouds simultaneously.
//!
//! ## One event per query
//!
//! A single user action (dataset query) produces exactly one analytics event,
//! sent when the query completes. The event includes both the scan/planning
//! phase stats and the fetch phase stats (split by gRPC vs direct fetches).
//!
//! ## Trace correlation
//!
//! When the client makes a `query_dataset` call, the server responds with an
//! `x-request-trace-id` header containing the server-side trace ID. The client
//! captures this and, when sending the analytics OTLP export to the server,
//! sets the same `x-request-trace-id` header on the analytics request. This
//! allows the server to correlate the analytics event with the original query
//! trace in Grafana/Tempo.

use std::fmt;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use datafusion::logical_expr::Expr;
use datafusion::physical_plan::metrics::{ExecutionPlanMetricsSet, MetricBuilder, MetricsSet};
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::any_value::Value,
    common::v1::{AnyValue, KeyValue},
    resource::v1::Resource,
    trace::v1::{ResourceSpans, ScopeSpans, Span, span::SpanKind},
};
use re_async::AsyncRuntimeHandle;
use re_dataframe::QueryExpression;
use re_protos::cloud::v1alpha1::SystemTableKind;
use re_protos::cloud::v1alpha1::ext::ProviderDetails;
use re_redap_client::ConnectionAnalyticsExporter;
use re_uri::Origin;
use web_time::{Duration, Instant, SystemTime};

use crate::metrics_capture::{QueryMetrics, QuerySnapshot, build_query_snapshot};

/// A per-connection analytics client that sends OTLP traces to a specific
/// Rerun Hub's OTEL ingest endpoint.
///
/// Cheap to clone (wraps an `Arc`).
///
/// The target of these events are `PostHog`, and are aimed at user analytics.
/// This means a single user action (e.g. a dataset query) should only
/// trigger a single `PostHog` event, sent at the conclusion of the action.
#[derive(Clone)]
pub(crate) struct ConnectionAnalytics {
    inner: Arc<Inner>,
}

impl fmt::Debug for ConnectionAnalytics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionAnalytics")
            .field("origin", &self.inner.origin)
            .finish_non_exhaustive()
    }
}

struct Inner {
    origin: Origin,
    async_runtime: Option<AsyncRuntimeHandle>,

    /// Analytics OTLP exporter sharing the layered tower service of the sibling
    /// [`re_redap_client::ConnectionClient`] (same HTTP/2 transport, same auth /
    /// version / propagate-headers stack).
    exporter: Option<ConnectionAnalyticsExporter>,
}

impl ConnectionAnalytics {
    /// Create a new analytics sender for the given origin.
    ///
    /// The analytics OTLP exports go through the same authenticated and
    /// version-tagged HTTP/2 connection as regular REDAP RPCs.
    pub fn new(exporter: ConnectionAnalyticsExporter, async_runtime: AsyncRuntimeHandle) -> Self {
        let origin = exporter.origin().clone();
        Self {
            inner: Arc::new(Inner {
                origin,
                async_runtime: Some(async_runtime),
                exporter: Some(exporter),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn disabled_for_test(origin: Origin) -> Self {
        Self {
            inner: Arc::new(Inner {
                origin,
                async_runtime: None,
                exporter: None,
            }),
        }
    }

    /// Begin tracking analytics for a table scan.
    ///
    /// Returns a [`PendingTableQueryAnalytics`] that accumulates stats across the
    /// scan. The analytics event is sent when the last clone is dropped.
    pub fn begin_table_query(
        &self,
        info: TableQueryInfo,
        scan_start: Instant,
    ) -> PendingTableQueryAnalytics {
        PendingTableQueryAnalytics {
            inner: Arc::new(PendingTableInner {
                connection: self.clone(),
                info,
                stats: SharedTableScanStats::default(),
                scan_start,
                time_to_first_response: OnceLock::new(),
                time_to_first_batch: OnceLock::new(),
                trace_id: OnceLock::new(),
                error_kind: OnceLock::new(),
            }),
        }
    }

    /// Send an OTLP span in the background. Never blocks the caller.
    fn send_span(&self, span: Span, trace_id: Option<opentelemetry::TraceId>) {
        let this = self.clone();

        let fut = async move {
            if let Err(err) = this.send_span_impl(span, trace_id).await {
                re_log::debug_once!(
                    "Failed to send analytics to Rerun Hub: {} ({})",
                    err.code(),
                    err.message()
                );
            }
        };

        if let Some(async_runtime) = &self.inner.async_runtime {
            async_runtime.spawn_future(fut);
        }
    }

    async fn send_span_impl(
        &self,
        mut span: Span,
        trace_id: Option<opentelemetry::TraceId>,
    ) -> tonic::Result<()> {
        let Some(exporter) = &self.inner.exporter else {
            return Ok(());
        };
        assign_span_identity(&mut span, trace_id)?;

        // `service.name` is the OTel resource attribute that identifies the
        // sending service in the trace store (Grafana/Tempo etc.). We
        // hard-code it to `"rerun-viewer"` here because this piggy-back is
        // viewer-specific by construction — it ships `cloud_query_dataset`
        // / `cloud_scan_table` spans from the viewer's query path,
        // regardless of whatever `OTEL_SERVICE_NAME` the caller's process
        // might have set for its own general tracing.
        //
        // The SDK-trace path in `re_perf_telemetry` (driven by
        // `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=rerun://`) instead follows
        // the OTel convention and uses whatever `OTEL_SERVICE_NAME` was
        // set at init time (e.g. `rerun-py`, application-specific).
        let mut resource_attributes = vec![kv_string("service.name", "rerun-viewer")];
        if let Some(analytics) = re_analytics::Analytics::global_get() {
            resource_attributes.push(kv_string("analytics_id", &analytics.config().analytics_id));
        }

        let export_request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource {
                    attributes: resource_attributes,
                    dropped_attributes_count: 0,
                    entity_refs: Vec::new(),
                }),
                scope_spans: vec![ScopeSpans {
                    scope: None,
                    spans: vec![span],
                    schema_url: String::new(),
                }],
                schema_url: String::new(),
            }],
        };

        exporter.export_trace(export_request, trace_id).await
    }
}

fn assign_span_identity(
    span: &mut Span,
    correlated_trace_id: Option<opentelemetry::TraceId>,
) -> tonic::Result<()> {
    span.trace_id = if let Some(trace_id) =
        correlated_trace_id.filter(|trace_id| *trace_id != opentelemetry::TraceId::INVALID)
    {
        trace_id.to_bytes().to_vec()
    } else {
        random_nonzero_id::<16>()?
    };
    span.span_id = random_nonzero_id::<8>()?;
    Ok(())
}

fn random_nonzero_id<const N: usize>() -> tonic::Result<Vec<u8>> {
    loop {
        let mut id = [0; N];
        getrandom::fill(&mut id).map_err(|err| {
            tonic::Status::internal(format!("failed to generate OTLP span ID: {err}"))
        })?;
        if id.iter().any(|byte| *byte != 0) {
            return Ok(id.to_vec());
        }
    }
}

// ----------------------------------------------------------------------------

/// Begin tracking analytics for a query.
///
/// Always returns a [`PendingQueryAnalytics`] — `connection` is `None` when
/// the per-process telemetry stack is not active, in which case the resulting
/// analytics struct gathers data passively (for the in-process
/// [`crate::metrics_capture`] subscribers and DataFusion's `metrics()`) but
/// skips the `PostHog` OTLP send at drop time.
///
/// Constructs the query's [`QueryMetrics`] from `query_info` and owns it —
/// retrieve the shared handle via [`PendingQueryAnalytics::metrics`]. It's the
/// single source of truth for fetch counters: the `PendingInner::Drop` path
/// reads it via [`build_query_snapshot`] to construct the OTLP span attribute
/// set, and `SegmentStreamExec` reads it for the snapshot path and
/// `EXPLAIN ANALYZE`.
pub fn begin_query(
    connection: Option<ConnectionAnalytics>,
    query_info: QueryInfo,
    scan_start: Instant,
    scan_start_wall: SystemTime,
) -> PendingQueryAnalytics {
    PendingQueryAnalytics {
        inner: Arc::new(PendingInner {
            connection,
            metrics: Arc::new(QueryMetrics::new(query_info)),
            scan_start,
            scan_start_wall,
            time_to_first_chunk: OnceLock::new(),
            direct_terminal_reason: OnceLock::new(),
            error_kind: OnceLock::new(),
        }),
    }
}

/// Query shape
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryType {
    /// Query for static (timeless) data — no temporal selector applies.
    Static,

    /// Point-in-time query: a `latest_at` selector with no range.
    LatestAt,

    /// Time-range query: a range selector with no `latest_at`.
    Range,

    /// Combined dataframe query: both `latest_at` and range selectors are set.
    Dataframe,

    /// Neither `latest_at` nor range is set — an unbounded scan of all timestamps.
    FullScan,
}

impl QueryType {
    /// Classify the query shape into a bounded label for analytics.
    pub(crate) fn classify(query_expression: &QueryExpression) -> Self {
        if query_expression.is_static() {
            Self::Static
        } else {
            let has_latest_at = query_expression.min_latest_at().is_some();
            let has_range = query_expression.max_range().is_some();
            match (has_latest_at, has_range) {
                (true, true) => Self::Dataframe,
                (true, false) => Self::LatestAt,
                (false, true) => Self::Range,
                (false, false) => Self::FullScan,
            }
        }
    }

    /// Stable string label emitted into the analytics span.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::LatestAt => "latest_at",
            Self::Range => "range",
            Self::Dataframe => "dataframe",
            Self::FullScan => "full_scan",
        }
    }
}

/// Information about the query planning phase, collected in `scan()`.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryInfo {
    /// The dataset being queried. Sent to the server so it can enrich the
    /// analytics event with full dataset stats (total chunks, bytes, etc.).
    pub dataset_id: String,

    /// Number of unique chunks returned by `query_dataset` (subset of the dataset).
    pub query_chunks: usize,

    /// Number of distinct segments involved in the query.
    pub query_segments: usize,

    /// Number of distinct layers touched by the query.
    pub query_layers: usize,

    /// Number of columns in the query output schema.
    pub query_columns: usize,

    /// Number of entity paths in the query request.
    pub query_entities: usize,

    /// Total size of all queried chunks in bytes (from chunk metadata).
    pub query_bytes: u64,

    /// DataFusion target partition count for this scan (from
    /// `SessionConfig::target_partitions`) — the degree of parallelism the
    /// plan was built for. Provides the denominator that makes the observed
    /// `peak_inflight_fetches` throughput counter interpretable.
    pub target_partitions: usize,

    /// Min # chunks touched within any single segment in this query.
    pub query_chunks_per_segment_min: u32,

    /// Max number of chunks touched within any single segment in this query.
    pub query_chunks_per_segment_max: u32,

    /// Mean number of chunks touched per segment in this query.
    pub query_chunks_per_segment_mean: f32,

    /// Query shape
    pub query_type: QueryType,

    /// Name of the sort/filter index (timeline) for this query, if any.
    pub primary_index_name: Option<String>,

    /// Time from sending `query_dataset` until the first response message
    /// arrives (the chunk metadata, not actual chunk data).
    pub time_to_first_chunk_info: Option<Duration>,

    /// Server-side trace ID from the `x-request-trace-id` response header.
    pub trace_id: Option<opentelemetry::TraceId>,

    /// Number of filter expressions the table provider was able to push down to the
    /// server (returning `Exact` or `Inexact` from `supports_filters_pushdown`).
    pub filters_pushed_down: usize,

    /// Number of filter expressions the table provider could not push down — they
    /// will be applied by DataFusion via a downstream `FilterExec`.
    pub filters_applied_client_side: usize,

    /// True when projection-based entity-path narrowing actually trimmed the set of
    /// entity paths sent to `query_dataset`.
    pub entity_path_narrowing_applied: bool,

    /// Number of filter expressions DataFusion presented to
    /// `supports_filters_pushdown` at planning time (equals
    /// `filters_pushed_down + filters_applied_client_side`).
    pub filters_total: u32,

    /// Semicolon-delimited SQL representations of every offered filter, with
    /// all literals scrubbed to `?` placeholders so the signature is a template
    /// and carries no customer data (see [`expr_filter_signature`])
    /// (e.g. `"(log_time > ?);rerun_segment_id IN (?, ?)"`).
    pub filters_signatures: String,

    /// Semicolon-delimited SQL signatures of filters classified as
    /// [`datafusion::logical_expr::TableProviderFilterPushDown::Exact`] at planning time.
    pub filters_signatures_exact: String,

    /// Semicolon-delimited SQL signatures of filters classified as
    /// [`datafusion::logical_expr::TableProviderFilterPushDown::Inexact`] at planning time.
    pub filters_signatures_inexact: String,

    /// Semicolon-delimited SQL signatures of filters classified as
    /// [`datafusion::logical_expr::TableProviderFilterPushDown::Unsupported`] at planning time.
    pub filters_signatures_unsupported: String,
}

/// Tracks a query in progress. Accumulates per-query state across phases.
///
/// Fetch counters live in the plan's [`QueryMetrics`] (shared with
/// `SegmentStreamExec` and DataFusion `EXPLAIN ANALYZE`); this struct just
/// holds an `Arc` clone of that handle plus timing and error state. A single
/// combined OTLP analytics event is sent to `PostHog` when the last clone is
/// dropped — but only if the per-process telemetry stack is active
/// (`connection.is_some()`).
///
/// Cheap to clone (wraps an `Arc`).
#[derive(Clone)]
pub(crate) struct PendingQueryAnalytics {
    inner: Arc<PendingInner>,
}

impl fmt::Debug for PendingQueryAnalytics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingQueryAnalytics")
            .finish_non_exhaustive()
    }
}

pub(crate) struct PendingInner {
    /// `None` when the telemetry stack is off — analytics still accumulate
    /// passively (for `metrics_capture` subscribers and DataFusion
    /// `metrics()`) but the drop-time OTLP send is skipped.
    connection: Option<ConnectionAnalytics>,

    /// The query's fetch counters + embedded plan-time `QueryInfo`. IO tasks
    /// `fetch_add` into the atomics during execution; `Drop` reads them via
    /// [`build_query_snapshot`] to build the OTLP span. Single source of
    /// truth — there is no parallel accumulator; `SegmentStreamExec` reaches
    /// it through [`PendingQueryAnalytics::metrics`].
    metrics: Arc<QueryMetrics>,

    /// Monotonic start time of the query, for computing elapsed durations.
    scan_start: Instant,

    /// Wall-clock start time of the query. Combined with `SystemTime::now()` at
    /// drop time to produce the OTLP span's `start`/`end` timestamps, which then
    /// match `total_duration_us`.
    scan_start_wall: SystemTime,

    /// Time from scan start until the first chunk is returned to datafusion.
    time_to_first_chunk: OnceLock<Duration>,

    /// First terminal direct-fetch failure reason encountered, if any.
    /// Only set once. Stored as `&'static str` from the bounded
    /// [`DirectFetchFailureReason`] label set.
    direct_terminal_reason: OnceLock<DirectFetchFailureReason>,

    /// Error classification, if the query failed. `None` ⇒ success.
    /// Stored as `&'static str` from [`QueryErrorKind::as_str`] so emission is zero-copy.
    error_kind: OnceLock<&'static str>,
}

/// Bounded set of query-failure classifications for the analytics span.
///
/// Kept as an enum (rather than free-form strings) so that adding a new call
/// site cannot silently introduce a new `error_kind` value and inflate the
/// analytics cardinality. Add a variant here if you need a new bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", expect(dead_code))]
pub enum QueryErrorKind {
    /// A gRPC fetch (`FetchChunks` or the fast-path gRPC-only fetch) failed.
    GrpcFetch,

    /// A direct (HTTP Range) fetch failed, non-retryable or retries exhausted.
    DirectFetch,

    /// CPU-side decoding or execution error (chunk insertion, row materialization).
    Decode,

    /// Generic / unclassified error (e.g. IO task join failure).
    Other,
}

impl QueryErrorKind {
    /// Stable string label emitted into the analytics span.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GrpcFetch => "grpc_fetch",
            Self::DirectFetch => "direct_fetch",
            Self::Decode => "decode",
            Self::Other => "other",
        }
    }
}

/// Bounded set of terminal failure reasons for direct fetches.
///
/// These labels are emitted both into the per-process OTEL counter
/// (`chunk_fetch.direct.result`) and into the per-query `PostHog` span as
/// `fetch_direct_terminal_reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectFetchFailureReason {
    Timeout,
    Http4xx,
    Http5xx,
    Connection,
    Decode,

    /// The source object on the blob store changed since the dataset was
    /// registered.
    SourceChanged,
    Other,
}

impl DirectFetchFailureReason {
    /// Convert to the stable string label used in telemetry.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Http4xx => "http_4xx",
            Self::Http5xx => "http_5xx",
            Self::Connection => "connection",
            Self::Decode => "decode",
            Self::SourceChanged => "source_changed",
            Self::Other => "other",
        }
    }
}

impl PendingQueryAnalytics {
    /// The shared per-query [`QueryMetrics`] handle — the single source of
    /// truth for fetch counters and the plan-time `QueryInfo`. Also read by
    /// `SegmentStreamExec` for the snapshot path and `EXPLAIN ANALYZE`.
    pub(crate) fn metrics(&self) -> &Arc<QueryMetrics> {
        &self.inner.metrics
    }

    /// Record that the first result chunk has been returned to the user.
    /// Only the first call has any effect.
    #[cfg_attr(target_arch = "wasm32", expect(dead_code))]
    pub fn record_first_chunk(&self) {
        self.inner
            .time_to_first_chunk
            .get_or_init(|| self.inner.scan_start.elapsed());
    }

    /// Record the terminal failure reason for a direct fetch that exhausted retries
    /// or hit a non-retryable error. Only the first call has effect.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn record_direct_terminal_failure(&self, reason: DirectFetchFailureReason) {
        #[expect(clippy::let_underscore_must_use)]
        let _ = self.inner.direct_terminal_reason.set(reason);
    }

    /// Mark the query as failed with the given error kind.
    ///
    /// Only the first call has effect.
    pub fn record_error(&self, kind: QueryErrorKind) {
        #[expect(clippy::let_underscore_must_use)]
        let _ = self.inner.error_kind.set(kind.as_str());
    }

    /// Current error label, if any. Used by the in-process `metrics_capture`
    /// path so cancelled / failed queries still surface an `error_kind` in
    /// the [`crate::QuerySnapshot`].
    pub fn error_kind(&self) -> Option<&'static str> {
        self.inner.error_kind.get().copied()
    }

    /// Time from `scan_start` until the first chunk was returned, if recorded.
    /// Mirrors the value the OTLP `Drop` path emits, so `metrics_capture`
    /// subscribers see the same number.
    pub fn time_to_first_chunk(&self) -> Option<Duration> {
        self.inner.time_to_first_chunk.get().copied()
    }

    /// Terminal direct-fetch failure reason, if one was recorded. Mirrors the
    /// OTLP `Drop`-time value.
    pub fn direct_terminal_reason(&self) -> Option<DirectFetchFailureReason> {
        self.inner.direct_terminal_reason.get().copied()
    }

    /// Elapsed time since this query was begun in [`begin_query`]. The
    /// snapshot path uses this instead of a per-partition start so the
    /// reported duration doesn't depend on which partition is last to
    /// finish (or when DataFusion got around to scheduling it).
    pub fn total_duration(&self) -> Duration {
        self.inner.scan_start.elapsed()
    }
}

/// Per-task accumulator for fetch counters.
///
/// Each outer fetch task owns one of these and mutates it without
/// synchronization. At the end of the task it is folded into the plan's
/// shared [`QueryMetrics`] via [`TaskFetchStats::flush_into`]. Issued-request
/// counters are updated directly at the transport boundary so cancellation
/// cannot discard calls that already reached the network.
///
/// Buffering the remaining counters avoids cross-core cache-line ping-pong
/// during retry-heavy fetch loops.
#[derive(Default)]
#[must_use]
pub(crate) struct TaskFetchStats {
    grpc_bytes: u64,
    direct_bytes: u64,
    direct_retries_total: u64,
    direct_requests_retried: u64,
    direct_retry_sleep: Duration,
    direct_max_attempt: u64,
    direct_original_ranges: u64,
    direct_merged_ranges: u64,

    /// CPU time spent in `Chunk::from_record_batch` (both fetch paths) for the
    /// chunks decoded by this task.
    decode: Duration,
}

#[cfg_attr(target_arch = "wasm32", expect(dead_code))]
impl TaskFetchStats {
    pub fn record_grpc_bytes(&mut self, bytes: u64) {
        self.grpc_bytes += bytes;
    }

    /// Add decode/decompress CPU time observed while turning fetched bytes into
    /// chunks. Accumulated per task, then summed into `decode_time_us`.
    pub fn record_decode(&mut self, elapsed: Duration) {
        self.decode = self.decode.saturating_add(elapsed);
    }

    pub fn record_direct_bytes(&mut self, bytes: u64) {
        self.direct_bytes += bytes;
    }

    /// Record a single direct-fetch retry on one merged request.
    ///
    /// `sleep` is the backoff duration actually slept before the retry attempt.
    /// `attempt` is the attempt number about to be made (starts at 2 for the first retry).
    pub fn record_direct_retry(&mut self, sleep: Duration, attempt: u64) {
        self.direct_retries_total += 1;
        self.direct_retry_sleep = self.direct_retry_sleep.saturating_add(sleep);
        self.direct_max_attempt = self.direct_max_attempt.max(attempt);
    }

    /// Record that a single merged request needed at least one retry (call once per
    /// retried request, regardless of how many attempts it took).
    pub fn record_direct_request_was_retried(&mut self) {
        self.direct_requests_retried += 1;
    }

    /// Record the range-merging efficiency for this batch.
    pub fn record_direct_ranges(&mut self, original: u64, merged: u64) {
        self.direct_original_ranges += original;
        self.direct_merged_ranges += merged;
    }

    /// Merge another task-local accumulator into this one.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Prevent double-counting stats"
    )]
    pub fn merge_from(&mut self, other: Self) {
        let Self {
            grpc_bytes,
            direct_bytes,
            direct_retries_total,
            direct_requests_retried,
            direct_retry_sleep: direct_retry_sleep_us,
            direct_max_attempt,
            direct_original_ranges,
            direct_merged_ranges,
            decode,
        } = other;
        self.grpc_bytes += grpc_bytes;
        self.direct_bytes += direct_bytes;
        self.direct_retries_total += direct_retries_total;
        self.direct_requests_retried += direct_requests_retried;
        self.direct_retry_sleep += direct_retry_sleep_us;
        self.direct_max_attempt = self.direct_max_attempt.max(direct_max_attempt);
        self.direct_original_ranges += direct_original_ranges;
        self.direct_merged_ranges += direct_merged_ranges;
        self.decode = self.decode.saturating_add(decode);
    }

    /// Fold this buffer into the shared [`QueryMetrics`].
    ///
    /// All counters are aggregated across partitions; `direct_max_attempt`
    /// uses `fetch_max` so the resulting value is the true cross-partition
    /// max rather than a sum.
    pub fn flush_into(self, metrics: &QueryMetrics) {
        let Self {
            grpc_bytes,
            direct_bytes,
            direct_retries_total,
            direct_requests_retried,
            direct_retry_sleep,
            direct_max_attempt,
            direct_original_ranges,
            direct_merged_ranges,
            decode,
        } = self;

        // Zero-valued fields are skipped so totally-idle tasks don't touch the
        // shared cache line at all.
        if grpc_bytes != 0 {
            metrics
                .fetch_grpc_bytes
                .fetch_add(grpc_bytes, Ordering::Relaxed);
        }
        if direct_bytes != 0 {
            metrics
                .fetch_direct_bytes
                .fetch_add(direct_bytes, Ordering::Relaxed);
        }
        if direct_retries_total != 0 {
            metrics
                .fetch_direct_retries
                .fetch_add(direct_retries_total, Ordering::Relaxed);
        }
        if direct_requests_retried != 0 {
            metrics
                .fetch_direct_requests_retried
                .fetch_add(direct_requests_retried, Ordering::Relaxed);
        }
        if !direct_retry_sleep.is_zero() {
            metrics
                .fetch_direct_retry_sleep_us
                .fetch_add(direct_retry_sleep.as_micros() as u64, Ordering::Relaxed);
        }
        if direct_max_attempt != 0 {
            metrics
                .fetch_direct_max_attempt
                .fetch_max(direct_max_attempt, Ordering::Relaxed);
        }
        if direct_original_ranges != 0 {
            metrics
                .fetch_direct_original_ranges
                .fetch_add(direct_original_ranges, Ordering::Relaxed);
        }
        if direct_merged_ranges != 0 {
            metrics
                .fetch_direct_merged_ranges
                .fetch_add(direct_merged_ranges, Ordering::Relaxed);
        }
        if !decode.is_zero() {
            metrics
                .decode_time_us
                .fetch_add(decode.as_micros() as u64, Ordering::Relaxed);
        }
    }

    /// Flush this buffer into `analytics`' shared [`QueryMetrics`], also
    /// recording an error (if any) onto `analytics`.
    pub fn try_flush_into(
        self,
        analytics: &PendingQueryAnalytics,
        result: Result<(), QueryErrorKind>,
    ) {
        self.flush_into(analytics.metrics());
        if let Err(err) = result {
            analytics.record_error(err);
        }
    }
}

/// Build an ad-hoc [`MetricsSet`] for DataFusion `EXPLAIN ANALYZE` output.
///
/// Called on demand from `ExecutionPlan::metrics()`. Reads the live atomics
/// in `metrics` plus plan-time scalars in `metrics.query_info`, plus per-call
/// auxiliary inputs (`num_partitions`, `time_to_first_chunk`) that aren't
/// stored on `QueryMetrics`.
///
/// Names match what the old `seed_plan_time_metrics` + per-partition fetch
/// counters used, so downstream dashboards / grep targets keep working.
/// The output is summed across partitions (not per-partition rows) — the
/// `num_partitions` gauge surfaces the partition count separately.
///
/// `query_type` and `primary_index_name` are not surfaced — labels don't fit
/// the `MetricsSet::Count` shape; they show up in `DisplayAs::Verbose` instead.
/// `query_chunks_per_segment_mean` is `f32`, also doesn't fit `Count` — it
/// flows through the `QuerySnapshot` to `PostHog` / `query_metrics()` but not
/// EXPLAIN. Anyone reading EXPLAIN can divide `query_chunks / query_segments`.
pub(crate) fn build_metrics_set_for_explain(
    metrics: &QueryMetrics,
    num_partitions: usize,
    time_to_first_chunk: Option<Duration>,
) -> MetricsSet {
    let set = ExecutionPlanMetricsSet::new();
    let info = &metrics.query_info;
    let load = |a: &AtomicU64| a.load(Ordering::Relaxed) as usize;

    let global = |name: &'static str| MetricBuilder::new(&set).global_counter(name);
    global("query_chunks").add(info.query_chunks);
    global("query_segments").add(info.query_segments);
    global("query_layers").add(info.query_layers);
    global("query_columns").add(info.query_columns);
    global("query_entities").add(info.query_entities);
    global("query_bytes").add(info.query_bytes as usize);
    global("query_chunks_per_segment_min").add(info.query_chunks_per_segment_min as usize);
    global("query_chunks_per_segment_max").add(info.query_chunks_per_segment_max as usize);
    global("filters_pushed_down").add(info.filters_pushed_down);
    global("filters_applied_client_side").add(info.filters_applied_client_side);
    if info.entity_path_narrowing_applied {
        global("entity_path_narrowing_applied").add(1);
    }
    if let Some(ttfci) = info.time_to_first_chunk_info {
        global("time_to_first_chunk_info_us").add(ttfci.as_micros() as usize);
    }
    global("num_partitions").add(num_partitions);

    global("fetch_grpc_requests").add(load(&metrics.fetch_grpc_requests));
    global("fetch_grpc_bytes").add(load(&metrics.fetch_grpc_bytes));
    global("fetch_direct_requests").add(load(&metrics.fetch_direct_requests));
    global("fetch_direct_bytes").add(load(&metrics.fetch_direct_bytes));
    global("fetch_direct_retries").add(load(&metrics.fetch_direct_retries));
    global("fetch_direct_requests_retried").add(load(&metrics.fetch_direct_requests_retried));
    global("fetch_direct_retry_sleep_us").add(load(&metrics.fetch_direct_retry_sleep_us));
    global("fetch_direct_max_attempt").add(load(&metrics.fetch_direct_max_attempt));
    global("fetch_direct_original_ranges").add(load(&metrics.fetch_direct_original_ranges));
    global("fetch_direct_merged_ranges").add(load(&metrics.fetch_direct_merged_ranges));
    global("planned_fetch_batches").add(load(&metrics.planned_fetch_batches));
    global("planned_segment_waves").add(load(&metrics.planned_segment_waves));
    global("segment_admission_limit").add(load(&metrics.segment_admission_limit));
    global("segment_admission_candidate_limit")
        .add(load(&metrics.segment_admission_candidate_limit));
    global("segment_admission_source_code").add(load(&metrics.segment_admission_source));
    global("segment_admission_candidate_reason_code")
        .add(load(&metrics.segment_admission_candidate_reason));
    global("segment_admission_adaptive_enabled")
        .add(load(&metrics.segment_admission_adaptive_enabled));
    global("segment_admission_profile_segment_count")
        .add(load(&metrics.segment_admission_profile_segment_count));
    global("segment_admission_profile_complete")
        .add(load(&metrics.segment_admission_profile_complete));
    global("segment_admission_p95_segment_bytes")
        .add(load(&metrics.segment_admission_p95_segment_bytes));
    global("segment_admission_max_segment_bytes")
        .add(load(&metrics.segment_admission_max_segment_bytes));
    global("segment_admission_largest_window_bytes")
        .add(load(&metrics.segment_admission_largest_window_bytes));
    global("max_segments_per_fetch_batch").add(load(&metrics.max_segments_per_fetch_batch));
    global("max_segments_per_wave").add(load(&metrics.max_segments_per_wave));
    global("peak_active_segments").add(load(&metrics.peak_active_segments));
    global("pipeline_budget_bytes").add(load(&metrics.pipeline_budget_bytes));
    global("pipeline_peak_decoded_bytes").add(load(&metrics.pipeline_peak_decoded_bytes));
    global("pipeline_byte_waits").add(load(&metrics.pipeline_byte_waits));
    global("segment_admission_waits").add(load(&metrics.segment_admission_waits));
    global("pipeline_stall_breaker_activations")
        .add(load(&metrics.pipeline_stall_breaker_activations));
    global("delivered_rows").add(load(&metrics.delivered_rows));
    global("delivered_bytes").add(load(&metrics.delivered_bytes));
    global("decode_duration_us").add(load(&metrics.decode_time_us));
    global("peak_inflight_fetches").add(load(&metrics.peak_inflight_fetches));

    if let Some(ttfr) = time_to_first_chunk {
        MetricBuilder::new(&set)
            .subset_time("time_to_first_chunk", 0)
            .add_duration(ttfr);
    }

    set.clone_inner()
}

impl Drop for PendingInner {
    fn drop(&mut self) {
        // Only build + send the `PostHog` OTLP span when the per-process
        // telemetry stack is active. When it isn't, the analytics struct is
        // still serving its other consumers (`metrics_capture` and
        // DataFusion's `metrics()`) — we just skip the send.
        let Some(connection) = self.connection.as_ref() else {
            return;
        };

        let total_duration = self.scan_start.elapsed();
        let scan_end_wall = SystemTime::now();
        let time_to_first_chunk = self.time_to_first_chunk.get().copied();
        let direct_terminal_reason = self.direct_terminal_reason.get().copied();
        let error_kind = self.error_kind.get().copied();
        let trace_id = self.metrics.query_info.trace_id;

        let snapshot = build_query_snapshot(
            &self.metrics,
            total_duration,
            time_to_first_chunk,
            error_kind,
            direct_terminal_reason,
        );

        let span = build_query_span(&snapshot, self.scan_start_wall..scan_end_wall);

        connection.send_span(span, trace_id);
    }
}

/// Build the OTLP `cloud_query_dataset` span from a [`QuerySnapshot`].
///
/// Pure function — no I/O, no time reads. Takes the same `QuerySnapshot` shape
/// that the in-process `metrics_capture` subscribers see, so `PostHog` and
/// Python readers are guaranteed to observe identical values.
fn build_query_span(snap: &QuerySnapshot, wall_clock_range: Range<SystemTime>) -> Span {
    let start_time_unix_nano = nanos_since_epoch(&wall_clock_range.start);
    let end_time_unix_nano = nanos_since_epoch(&wall_clock_range.end);

    let QuerySnapshot {
        query_info:
            QueryInfo {
                dataset_id,
                query_chunks,
                query_segments,
                query_layers,
                query_columns,
                query_entities,
                query_bytes,
                target_partitions,
                query_chunks_per_segment_min,
                query_chunks_per_segment_max,
                query_chunks_per_segment_mean,
                query_type,
                primary_index_name,
                time_to_first_chunk_info,
                trace_id: _,
                filters_pushed_down,
                filters_applied_client_side,
                entity_path_narrowing_applied,
                filters_total,
                filters_signatures,
                filters_signatures_exact,
                filters_signatures_inexact,
                filters_signatures_unsupported,
            },
        total_duration,
        time_to_first_chunk,
        error_kind,
        direct_terminal_reason,
        fetch_grpc_requests,
        fetch_grpc_bytes,
        fetch_direct_requests,
        fetch_direct_bytes,
        fetch_direct_retries,
        fetch_direct_requests_retried,
        fetch_direct_retry_sleep,
        fetch_direct_max_attempt,
        fetch_direct_original_ranges,
        fetch_direct_merged_ranges,
        planned_fetch_batches,
        planned_segment_waves,
        segment_admission_limit,
        segment_admission_candidate_limit,
        segment_admission_source,
        segment_admission_candidate_reason,
        segment_admission_adaptive_enabled,
        segment_admission_profile_segment_count,
        segment_admission_profile_complete,
        segment_admission_p95_segment_bytes,
        segment_admission_max_segment_bytes,
        segment_admission_largest_window_bytes,
        max_segments_per_fetch_batch,
        max_segments_per_wave,
        peak_active_segments,
        pipeline_budget_bytes,
        pipeline_peak_decoded_bytes,
        pipeline_byte_waits,
        segment_admission_waits,
        pipeline_stall_breaker_activations,
        delivered_rows,
        delivered_bytes,
        decode_duration,
        peak_inflight_fetches,
    } = snap;

    #[expect(
        clippy::cast_possible_wrap,
        reason = "OTLP proto uses i64 for int values"
    )]
    let mut attributes = vec![
        kv_string("dataset_id", dataset_id),
        kv_int("query_chunks", *query_chunks as i64),
        kv_int("query_segments", *query_segments as i64),
        kv_int("query_layers", *query_layers as i64),
        kv_int("query_columns", *query_columns as i64),
        kv_int("query_entities", *query_entities as i64),
        kv_int("query_bytes", *query_bytes as i64),
        kv_int(
            "query_chunks_per_segment_min",
            i64::from(*query_chunks_per_segment_min),
        ),
        kv_int(
            "query_chunks_per_segment_max",
            i64::from(*query_chunks_per_segment_max),
        ),
        kv_double(
            "query_chunks_per_segment_mean",
            f64::from(*query_chunks_per_segment_mean),
        ),
        kv_string("query_type", query_type.as_str()),
        kv_int("total_duration_us", total_duration.as_micros() as i64),
        kv_bool("is_success", error_kind.is_none()),
        // Fetch stats: gRPC
        kv_int("fetch_grpc_requests", *fetch_grpc_requests as i64),
        kv_int("fetch_grpc_bytes", *fetch_grpc_bytes as i64),
        // Fetch stats: direct (HTTP). Note: gRPC retries happen at the transport
        // layer and are not visible here — only direct-URL retries are counted.
        kv_int("fetch_direct_requests", *fetch_direct_requests as i64),
        kv_int("fetch_direct_bytes", *fetch_direct_bytes as i64),
        kv_int("fetch_direct_retries", *fetch_direct_retries as i64),
        kv_int(
            "fetch_direct_requests_retried",
            *fetch_direct_requests_retried as i64,
        ),
        kv_int(
            "fetch_direct_retry_sleep_us",
            fetch_direct_retry_sleep.as_micros() as i64,
        ),
        kv_int("fetch_direct_max_attempt", *fetch_direct_max_attempt as i64),
        kv_int(
            "fetch_direct_original_ranges",
            *fetch_direct_original_ranges as i64,
        ),
        kv_int(
            "fetch_direct_merged_ranges",
            *fetch_direct_merged_ranges as i64,
        ),
        kv_int("planned_fetch_batches", *planned_fetch_batches as i64),
        kv_int("planned_segment_waves", *planned_segment_waves as i64),
        kv_int("segment_admission_limit", *segment_admission_limit as i64),
        kv_int(
            "segment_admission_candidate_limit",
            *segment_admission_candidate_limit as i64,
        ),
        kv_string("segment_admission_source", segment_admission_source),
        kv_string(
            "segment_admission_candidate_reason",
            segment_admission_candidate_reason,
        ),
        kv_bool(
            "segment_admission_adaptive_enabled",
            *segment_admission_adaptive_enabled,
        ),
        kv_int(
            "segment_admission_profile_segment_count",
            *segment_admission_profile_segment_count as i64,
        ),
        kv_bool(
            "segment_admission_profile_complete",
            *segment_admission_profile_complete,
        ),
        kv_int(
            "segment_admission_p95_segment_bytes",
            *segment_admission_p95_segment_bytes as i64,
        ),
        kv_int(
            "segment_admission_max_segment_bytes",
            *segment_admission_max_segment_bytes as i64,
        ),
        kv_int(
            "segment_admission_largest_window_bytes",
            *segment_admission_largest_window_bytes as i64,
        ),
        kv_int(
            "max_segments_per_fetch_batch",
            *max_segments_per_fetch_batch as i64,
        ),
        kv_int("max_segments_per_wave", *max_segments_per_wave as i64),
        kv_int("peak_active_segments", *peak_active_segments as i64),
        kv_int("pipeline_budget_bytes", *pipeline_budget_bytes as i64),
        kv_int(
            "pipeline_peak_decoded_bytes",
            *pipeline_peak_decoded_bytes as i64,
        ),
        kv_int("pipeline_byte_waits", *pipeline_byte_waits as i64),
        kv_int("segment_admission_waits", *segment_admission_waits as i64),
        kv_int(
            "pipeline_stall_breaker_activations",
            *pipeline_stall_breaker_activations as i64,
        ),
        kv_int("filters_pushed_down", *filters_pushed_down as i64),
        kv_int(
            "filters_applied_client_side",
            *filters_applied_client_side as i64,
        ),
        kv_bool(
            "entity_path_narrowing_applied",
            *entity_path_narrowing_applied,
        ),
        // Delivered payload, decode cost, observed parallelism.
        kv_int("query_target_partitions", *target_partitions as i64),
        kv_int("delivered_rows", *delivered_rows as i64),
        kv_int("delivered_bytes", *delivered_bytes as i64),
        kv_int("decode_duration_us", decode_duration.as_micros() as i64),
        kv_int("peak_inflight_fetches", *peak_inflight_fetches as i64),
    ];

    if *filters_total > 0 {
        attributes.push(kv_int("filters_total", i64::from(*filters_total)));
    }
    if !filters_signatures.is_empty() {
        attributes.push(kv_string("filters_signatures", filters_signatures));
    }
    if !filters_signatures_exact.is_empty() {
        attributes.push(kv_string(
            "filters_signatures_exact",
            filters_signatures_exact,
        ));
    }
    if !filters_signatures_inexact.is_empty() {
        attributes.push(kv_string(
            "filters_signatures_inexact",
            filters_signatures_inexact,
        ));
    }
    if !filters_signatures_unsupported.is_empty() {
        attributes.push(kv_string(
            "filters_signatures_unsupported",
            filters_signatures_unsupported,
        ));
    }

    if let Some(name) = primary_index_name.as_deref() {
        attributes.push(kv_string("primary_index_name", name));
    }

    if let Some(ttfci) = time_to_first_chunk_info {
        attributes.push(kv_int(
            "time_to_first_chunk_info_us",
            ttfci.as_micros() as i64,
        ));
    }

    if let Some(ttfr) = time_to_first_chunk {
        attributes.push(kv_int("time_to_first_chunk_us", ttfr.as_micros() as i64));
    }

    if let Some(reason) = direct_terminal_reason {
        attributes.push(kv_string("fetch_direct_terminal_reason", reason.as_str()));
    }

    if let Some(kind) = error_kind {
        attributes.push(kv_string("error_kind", kind));
    }

    Span {
        name: "cloud_query_dataset".to_owned(),
        kind: SpanKind::Client.into(),
        start_time_unix_nano,
        end_time_unix_nano,
        attributes,
        ..Default::default()
    }
}

// ----------------------------------------------------------------------------
// Table-scan analytics
//
// Mirrors the dataset-query analytics above but for `ScanTable` calls, which
// flow through `TableEntryTableProvider` / `GrpcStreamProvider`. Lance and
// other server-side scan stats (rows_scanned, fragments_*, …) are not
// reachable from the client today and will be added via server-side OTLP
// span enrichment in a follow-up.

/// Replace every literal in `expr` with a `?` placeholder.
///
/// This templates the filter signature — two filters differing only in their
/// constants (`frame_nr > 100` vs `frame_nr > 200`) collapse to the same
/// signature — and, just as importantly, keeps customer data out of both the
/// analytics span and the Python `MetricsCollector`, which read the same
/// signatures. The `TreeNode` walk recurses, so `IN (…)`, `BETWEEN … AND …`,
/// `LIKE`, and nested comparisons are all scrubbed in one pass.
fn scrub_expr_literals(expr: &Expr) -> Expr {
    use datafusion::common::tree_node::{Transformed, TreeNode as _};
    use datafusion::logical_expr::expr::Placeholder;

    expr.clone()
        .transform(|e| {
            if matches!(e, Expr::Literal(..)) {
                // Unparses to a bare `?` (see datafusion unparser) — no quotes,
                // no value, no type. A single fixed token means identical
                // templates regardless of how many literals a filter has.
                Ok(Transformed::yes(Expr::Placeholder(
                    Placeholder::new_with_field("?".to_owned(), None),
                )))
            } else {
                Ok(Transformed::no(e))
            }
        })
        .map(|transformed| transformed.data)
        .unwrap_or_else(|_| expr.clone())
}

/// Produce a SQL representation of a DataFusion filter expression for analytics.
///
/// Literals are scrubbed to `?` placeholders first (see [`scrub_expr_literals`])
/// so the signature is a template and carries no customer data.
///
/// Falls back to `variant_name()` for expressions the SQL unparser doesn't support.
/// The result is escaped so that individual signatures can be safely joined with `;`.
pub(crate) fn expr_filter_signature(expr: &Expr) -> String {
    let scrubbed = scrub_expr_literals(expr);
    let sql = datafusion::sql::unparser::expr_to_sql(&scrubbed)
        .map(|e| e.to_string())
        .unwrap_or_else(|_| scrubbed.variant_name().to_owned());
    escape_sig_str(&sql)
}

/// Escape `\` and `;` so individual signatures can be safely joined with `;`.
fn escape_sig_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;")
}

/// Underlying provider variant of a table entry.
///
/// Bounded so the analytics dimension cardinality stays low. Add a variant if
/// the catalog server exposes a new system or storage backend.
#[derive(Clone, Copy, Debug)]
pub enum TableKind {
    /// User-registered Lance-backed table.
    Lance,

    /// `__entries`: the catalog's entry list.
    SystemEntries,

    /// `__namespaces`: not currently used.
    SystemNamespaces,

    /// Caller did not (or could not) determine the kind without an extra RPC.
    Unknown,
}

impl TableKind {
    /// Stable string label emitted into the analytics span.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Lance => "lance",
            Self::SystemEntries => "system_entries",
            Self::SystemNamespaces => "system_namespaces",
            Self::Unknown => "unknown",
        }
    }
}

impl From<&ProviderDetails> for TableKind {
    fn from(details: &ProviderDetails) -> Self {
        match details {
            ProviderDetails::LanceTable(_) => Self::Lance,
            ProviderDetails::SystemTable(t) => match t.kind {
                SystemTableKind::Entries => Self::SystemEntries,
                SystemTableKind::Namespaces => Self::SystemNamespaces,
                SystemTableKind::Unspecified => Self::Unknown,
            },
        }
    }
}

/// Where a table scan was initiated from.
///
/// Bounded enum to keep the analytics dimension cardinality low. Each new
/// caller (e.g. a future programmatic API) should add a variant.
#[derive(Clone, Copy, Debug)]
pub enum TableQueryCaller {
    /// `RedapCatalogProvider` resolving a name through DataFusion's `SessionContext`.
    /// Typically fires from SQL queries issued via the Python SDK.
    CatalogResolver,

    /// `__entries` system-table scan (catalog browsing).
    EntriesTable,

    /// Viewer table-detail UI in `re_redap_browser`.
    BrowserDetailView,
}

impl TableQueryCaller {
    /// Stable string label emitted into the analytics span.
    const fn as_str(self) -> &'static str {
        match self {
            Self::CatalogResolver => "catalog_resolver",
            Self::EntriesTable => "entries_table",
            Self::BrowserDetailView => "browser_detail_view",
        }
    }
}

/// Information about the table-scan planning phase, collected in `scan()`.
#[derive(Clone, Debug)]
pub struct TableQueryInfo {
    /// Server-assigned id of the table being scanned. Stringified so it
    /// matches `dataset_id` formatting on the dataset-query span.
    pub table_id: String,

    /// Underlying table kind. See [`TableKind`].
    pub table_kind: TableKind,

    /// What initiated this scan. See [`TableQueryCaller`].
    pub caller: TableQueryCaller,

    /// Number of fields in the table's full schema.
    pub schema_total_columns: u32,

    /// Number of columns DataFusion asked the provider to produce. Equal to
    /// `schema_total_columns` when no projection is requested.
    pub projected_columns: u32,

    /// `true` iff DataFusion provided a `LIMIT` to the scan.
    pub has_limit: bool,

    /// The `LIMIT` value, when present.
    pub limit_value: Option<u64>,

    /// Wall-clock start..end of the scan. End is set at `Drop` time.
    pub time_range: Range<SystemTime>,

    /// Total number of filter exprs DataFusion offered to `supports_filters_pushdown`.
    /// Zero when the method was never called (no filters in the query).
    pub filters_total: u32,

    /// Semicolon-delimited SQL representations of every offered filter, in
    /// the same order DataFusion supplied them. Empty when none were offered.
    /// See [`expr_filter_signature`].
    pub filters_signatures: String,
}

/// Accumulates per-scan counters from the streaming-provider IO loop.
///
/// Not contended in practice today (one stream per scan) but kept atomic for
/// consistency with the dataset-query side and to leave room for parallelism.
#[derive(Default)]
pub(crate) struct SharedTableScanStats {
    grpc_requests: AtomicU64,
    batches: AtomicU64,
    rows_returned: AtomicU64,
    bytes_returned: AtomicU64,
}

/// Snapshot of [`SharedTableScanStats`] taken at span-build time.
///
/// Pulled out so [`build_table_query_span`] can be a pure, easily-testable
/// function with no atomic loads of its own.
#[derive(Default, Clone, Copy)]
pub(crate) struct TableScanStatsSnapshot {
    pub grpc_requests: u64,
    pub batches: u64,
    pub rows_returned: u64,
    pub bytes_returned: u64,
}

impl SharedTableScanStats {
    /// Take a snapshot of the counters using relaxed atomic loads.
    fn snapshot(&self) -> TableScanStatsSnapshot {
        TableScanStatsSnapshot {
            grpc_requests: self.grpc_requests.load(Ordering::Relaxed),
            batches: self.batches.load(Ordering::Relaxed),
            rows_returned: self.rows_returned.load(Ordering::Relaxed),
            bytes_returned: self.bytes_returned.load(Ordering::Relaxed),
        }
    }
}

/// Tracks a table scan in progress. Cheap to clone (wraps an `Arc`).
///
/// The analytics event is emitted when the last clone is dropped.
#[derive(Clone)]
pub(crate) struct PendingTableQueryAnalytics {
    inner: Arc<PendingTableInner>,
}

impl fmt::Debug for PendingTableQueryAnalytics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingTableQueryAnalytics")
            .finish_non_exhaustive()
    }
}

struct PendingTableInner {
    connection: ConnectionAnalytics,
    info: TableQueryInfo,
    stats: SharedTableScanStats,

    /// Monotonic start time of the scan, for computing elapsed durations.
    scan_start: Instant,

    /// Time from `scan_start` until the first `ScanTableResponse` arrives.
    time_to_first_response: OnceLock<Duration>,

    /// Time from `scan_start` until the first `RecordBatch` is yielded to
    /// DataFusion. In today's streaming-provider path this is essentially the
    /// same point as `time_to_first_response`; both fields are kept so the
    /// analytics schema stays meaningful if batch coalescing changes.
    time_to_first_batch: OnceLock<Duration>,

    /// Server-side trace id from the `x-request-trace-id` response header on
    /// the `ScanTable` response.
    trace_id: OnceLock<opentelemetry::TraceId>,

    /// Error classification, if the scan failed. `None` ⇒ success.
    /// Stored as `&'static str` from [`QueryErrorKind::as_str`] so emission is zero-copy.
    error_kind: OnceLock<&'static str>,
}

impl PendingTableQueryAnalytics {
    /// Record the server-side trace id from the `ScanTable` response. Only the
    /// first call has effect.
    pub fn record_trace_id(&self, trace_id: opentelemetry::TraceId) {
        #[expect(clippy::let_underscore_must_use)]
        let _ = self.inner.trace_id.set(trace_id);
    }

    /// Record that the first `ScanTableResponse` has arrived from gRPC. Only
    /// the first call has effect.
    pub fn record_first_response(&self) {
        self.inner
            .time_to_first_response
            .get_or_init(|| self.inner.scan_start.elapsed());
    }

    /// Record that the first `RecordBatch` has been yielded to DataFusion.
    /// Only the first call has effect.
    pub fn record_first_batch(&self) {
        self.inner
            .time_to_first_batch
            .get_or_init(|| self.inner.scan_start.elapsed());
    }

    /// Record one received gRPC message and its decoded record batch.
    pub fn record_batch(&self, num_rows: u64, num_bytes: u64) {
        self.inner
            .stats
            .grpc_requests
            .fetch_add(1, Ordering::Relaxed);
        self.inner.stats.batches.fetch_add(1, Ordering::Relaxed);
        if num_rows != 0 {
            self.inner
                .stats
                .rows_returned
                .fetch_add(num_rows, Ordering::Relaxed);
        }
        if num_bytes != 0 {
            self.inner
                .stats
                .bytes_returned
                .fetch_add(num_bytes, Ordering::Relaxed);
        }
    }

    /// Mark the scan as failed with the given error kind. Only the first call
    /// has effect.
    pub fn record_error(&self, kind: QueryErrorKind) {
        #[expect(clippy::let_underscore_must_use)]
        let _ = self.inner.error_kind.set(kind.as_str());
    }

    /// Build the OTLP span using the current accumulated state, without
    /// dropping the analytics. Lets end-to-end tests inspect the post-stream
    /// span before the [`Drop`] impl runs.
    #[cfg(test)]
    pub(crate) fn build_span_for_test(&self) -> Span {
        let mut span = self.inner.build_span();
        assign_span_identity(&mut span, self.inner.trace_id.get().copied()).unwrap();
        span
    }
}

impl PendingTableInner {
    /// Snapshot the inner state and produce the OTLP span. Used both by
    /// [`Drop`] and by tests (via `PendingTableQueryAnalytics::build_span_for_test()`).
    ///
    /// Reads `scan_start.elapsed()` for `total_duration_us` and `SystemTime::now()`
    /// for the span end time, so calling this twice produces slightly different
    /// timing values — that's fine, it's how Drop already behaves.
    fn build_span(&self) -> Span {
        let total_duration = self.scan_start.elapsed();
        let scan_end_wall = SystemTime::now();
        let stats = self.stats.snapshot();
        build_table_query_span(
            &self.info,
            stats,
            self.info.time_range.start..scan_end_wall,
            total_duration,
            self.time_to_first_response.get().copied(),
            self.time_to_first_batch.get().copied(),
            self.trace_id.get().copied(),
            self.error_kind.get().copied(),
        )
    }
}

impl Drop for PendingTableInner {
    fn drop(&mut self) {
        let span = self.build_span();
        let trace_id = self.trace_id.get().copied();
        self.connection.send_span(span, trace_id);
    }
}

/// Build the OTLP `cloud_scan_table` span from collected per-scan data.
///
/// Pure function — no I/O, no time reads. Extracted from `Drop for
/// PendingTableInner` so the exact attribute set the analytics pipeline relies
/// on can be locked down by unit tests; if a future change accidentally drops
/// or renames a field, the tests fail.
pub(crate) fn build_table_query_span(
    info: &TableQueryInfo,
    stats: TableScanStatsSnapshot,
    wall_clock_range: Range<SystemTime>,
    total_duration: Duration,
    time_to_first_response: Option<Duration>,
    time_to_first_batch: Option<Duration>,
    _trace_id: Option<opentelemetry::TraceId>,
    error_kind: Option<&'static str>,
) -> Span {
    let TableQueryInfo {
        ref table_id,
        table_kind,
        caller,
        schema_total_columns,
        projected_columns,
        has_limit,
        limit_value,
        time_range: _,
        filters_total,
        ref filters_signatures,
    } = *info;

    let start_time_unix_nano = nanos_since_epoch(&wall_clock_range.start);
    let end_time_unix_nano = nanos_since_epoch(&wall_clock_range.end);

    #[expect(
        clippy::cast_possible_wrap,
        reason = "OTLP proto uses i64 for int values"
    )]
    let mut attributes = vec![
        // Identification
        kv_string("table_id", table_id),
        kv_string("table_kind", table_kind.as_str()),
        kv_string("caller", caller.as_str()),
        // Schema / projection
        kv_int("schema_total_columns", i64::from(schema_total_columns)),
        kv_int("projected_columns", i64::from(projected_columns)),
        // Limit
        kv_bool("has_limit", has_limit),
        // Outcome
        kv_bool("is_success", error_kind.is_none()),
        // Timing
        kv_int("total_duration_us", total_duration.as_micros() as i64),
        // gRPC
        kv_int("fetch_grpc_requests", stats.grpc_requests as i64),
        // Result size
        kv_int("num_record_batches", stats.batches as i64),
        kv_int("rows_returned", stats.rows_returned as i64),
        kv_int("bytes_returned", stats.bytes_returned as i64),
    ];

    if let Some(value) = limit_value {
        #[expect(
            clippy::cast_possible_wrap,
            reason = "OTLP proto uses i64 for int values"
        )]
        attributes.push(kv_int("limit_value", value as i64));
    }

    if let Some(ttfr) = time_to_first_response {
        attributes.push(kv_int("time_to_first_response_us", ttfr.as_micros() as i64));
    }

    if let Some(ttfb) = time_to_first_batch {
        attributes.push(kv_int("time_to_first_batch_us", ttfb.as_micros() as i64));
    }

    if let Some(kind) = error_kind {
        attributes.push(kv_string("error_kind", kind));
    }

    if filters_total > 0 {
        attributes.push(kv_int("filters_total", i64::from(filters_total)));
    }
    if !filters_signatures.is_empty() {
        attributes.push(kv_string("filters_signatures", filters_signatures));
    }

    Span {
        name: "cloud_scan_table".to_owned(),
        kind: SpanKind::Client.into(),
        start_time_unix_nano,
        end_time_unix_nano,
        attributes,
        ..Default::default()
    }
}

// ----------------------------------------------------------------------------

fn nanos_since_epoch(time: &SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn kv_string(key: &str, value: &str) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        value: Some(AnyValue {
            value: Some(Value::StringValue(value.to_owned())),
        }),
        key_strindex: 0,
    }
}

fn kv_int(key: &str, value: impl Into<i64>) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        value: Some(AnyValue {
            value: Some(Value::IntValue(value.into())),
        }),
        key_strindex: 0,
    }
}

fn kv_bool(key: &str, value: bool) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        value: Some(AnyValue {
            value: Some(Value::BoolValue(value)),
        }),
        key_strindex: 0,
    }
}

fn kv_double(key: &str, value: f64) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        value: Some(AnyValue {
            value: Some(Value::DoubleValue(value)),
        }),
        key_strindex: 0,
    }
}

#[cfg(test)]
mod test_explain_metrics_set;

#[cfg(test)]
mod test_expr_filter_signature;

#[cfg(test)]
mod test_filter_capture_span;

#[cfg(test)]
mod test_table_query;

#[cfg(test)]
mod tests;
