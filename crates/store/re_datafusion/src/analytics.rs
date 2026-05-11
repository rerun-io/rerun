//! Per-connection analytics for dataset queries.
//!
//! Each connection to a Rerun Cloud instance gets its own analytics sender
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

use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use opentelemetry_proto::tonic::{
    collector::trace::v1::{ExportTraceServiceRequest, ExportTraceServiceResponse},
    common::v1::any_value::Value,
    common::v1::{AnyValue, KeyValue},
    resource::v1::Resource,
    trace::v1::{ResourceSpans, ScopeSpans, Span, span::Link, span::SpanKind},
};
use re_dataframe::QueryExpression;
use re_protos::cloud::v1alpha1::SystemTableKind;
use re_protos::cloud::v1alpha1::ext::ProviderDetails;
use re_uri::Origin;
use tokio::sync::OnceCell;
use web_time::{Duration, SystemTime};

const EXPORT_PATH: &str = "/opentelemetry.proto.collector.trace.v1.TraceService/Export";

/// A per-connection analytics client that sends OTLP traces to a specific
/// Rerun Cloud's OTEL ingest endpoint.
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

impl std::fmt::Debug for ConnectionAnalytics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionAnalytics")
            .field("origin", &self.inner.origin)
            .finish_non_exhaustive()
    }
}

struct Inner {
    origin: Origin,

    /// gRPC client sharing the layered tower service of the sibling
    /// [`re_redap_client::ConnectionClient`] (i.e. same HTTP/2 transport, same auth /
    /// version / propagate-headers stack).
    ///
    /// Cloned per send so concurrent OTLP exports don't serialize on a single client.
    grpc: tonic::client::Grpc<re_redap_client::RedapClientInner>,

    /// Lazily populated once per connection via [`ConnectionAnalytics::set_server_version`].
    /// `None` if the version RPC failed or has not completed yet.
    server_version: OnceCell<Option<String>>,
}

impl ConnectionAnalytics {
    /// Create a new analytics sender for the given origin.
    ///
    /// The analytics OTLP channel reuses the layered tower service of the supplied
    /// [`re_redap_client::ConnectionClient`], so exports go through the same
    /// authenticated and version-tagged HTTP/2 connection as regular RPCs.
    pub fn new(origin: Origin, client: &re_redap_client::ConnectionClient) -> Self {
        Self {
            inner: Arc::new(Inner {
                origin,
                grpc: tonic::client::Grpc::new(client.service()),
                server_version: OnceCell::new(),
            }),
        }
    }

    /// Record the server/stack version string (e.g. semver) for this connection.
    ///
    /// Only the first call has effect. The value is then attached to every
    /// subsequent query span on this connection as `server_version`.
    pub fn set_server_version(&self, version: Option<String>) {
        // `OnceCell::set` returns Err if the cell was already set; silently ignore —
        // we only want the first value.
        #[expect(clippy::let_underscore_must_use)]
        let _ = self.inner.server_version.set(version);
    }

    /// Returns the cached server version, if available.
    fn server_version(&self) -> Option<String> {
        self.inner.server_version.get().and_then(Clone::clone)
    }

    /// Begin tracking analytics for a query.
    ///
    /// Returns a [`PendingQueryAnalytics`] that accumulates stats across phases.
    /// The analytics event is sent when the last clone is dropped.
    pub fn begin_query(
        &self,
        query_info: QueryInfo,
        scan_start: web_time::Instant,
    ) -> PendingQueryAnalytics {
        PendingQueryAnalytics {
            inner: Arc::new(PendingInner {
                connection: self.clone(),
                query_info,
                fetch_stats: SharedFetchStats::default(),
                scan_start,
                time_to_first_chunk: OnceLock::new(),
                direct_terminal_reason: OnceLock::new(),
                error_kind: OnceLock::new(),
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
        scan_start: web_time::Instant,
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
                    "Failed to send analytics to Rerun Cloud: {} ({})",
                    err.code(),
                    err.message()
                );
            }
        };

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(fut);

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(fut);
            } else {
                // Prefer spawning on the current tokio runtime if available.
                // When called from Python via FFI, the polling thread may not have a
                // tokio runtime entered, so fall back to a detached thread.
                std::thread::Builder::new()
                    .name("query-analytics-sender".to_owned())
                    .spawn(move || {
                        let rt = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build();
                        match rt {
                            Ok(rt) => rt.block_on(fut),
                            Err(err) => {
                                re_log::debug_once!("Failed to create analytics runtime: {err}");
                            }
                        }
                    })
                    .ok();
            }
        }
    }

    async fn send_span_impl(
        &self,
        span: Span,
        trace_id: Option<opentelemetry::TraceId>,
    ) -> tonic::Result<()> {
        // Clone per send: avoids serializing
        // concurrent sends behind a single client's `&mut self` borrow.
        let mut grpc = self.inner.grpc.clone();

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

        let mut request = tonic::Request::new(export_request);
        if let Some(trace_id) = trace_id
            && let Ok(value) = trace_id.to_string().parse()
        {
            request.metadata_mut().insert("x-request-trace-id", value);
        }

        grpc.ready().await.map_err(|err| {
            tonic::Status::unavailable(format!("analytics channel not ready: {err}"))
        })?;

        let path = http::uri::PathAndQuery::from_static(EXPORT_PATH);
        let codec = tonic_prost::ProstCodec::default();

        let _response: tonic::Response<ExportTraceServiceResponse> =
            grpc.unary(request.map(|m| m), path, codec).await?;

        Ok(())
    }
}

// ----------------------------------------------------------------------------

/// Query shape
#[derive(Clone, Copy, Debug)]
pub(crate) enum QueryType {
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
    const fn as_str(self) -> &'static str {
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
#[derive(Clone, Debug)]
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

    /// Max number of chunks touched within any single segment in this query.
    pub query_chunks_per_segment_max: u32,

    /// Mean number of chunks touched per segment in this query.
    pub query_chunks_per_segment_mean: f32,

    /// Query shape
    pub query_type: QueryType,

    /// Name of the sort/filter index (timeline) for this query, if any.
    pub primary_index_name: Option<String>,

    /// Wall-clock start..end of the scan planning phase.
    pub time_range: Range<SystemTime>,

    /// Time from sending `query_dataset` until the first response message
    /// arrives (the chunk metadata, not actual chunk data).
    pub time_to_first_chunk_info: Option<Duration>,

    /// Server-side trace ID from the `x-request-trace-id` response header.
    pub trace_id: Option<opentelemetry::TraceId>,
}

/// Accumulates fetch statistics from multiple partitions.
///
/// Thread-safe — multiple IO loops can record stats concurrently.
///
/// This is the final sink for per-query fetch counters. To avoid cross-core
/// cache-line contention during the hot fetch/retry loops, writers accumulate
/// into a task-local [`TaskFetchStats`] and flush into this shared struct
/// exactly once per outer fetch task via [`TaskFetchStats::flush_into`].
#[derive(Default)]
pub(crate) struct SharedFetchStats {
    grpc_requests: AtomicU64,
    grpc_bytes: AtomicU64,
    direct_requests: AtomicU64,
    direct_bytes: AtomicU64,

    /// Total extra direct-fetch attempts across all merged requests (attempts beyond the first).
    /// Note: gRPC retries happen at the transport layer and are not visible here — only direct
    /// (HTTP Range) retries are counted.
    direct_retries_total: AtomicU64,

    /// Number of distinct merged requests that ended up needing more than one attempt.
    direct_requests_retried: AtomicU64,

    /// Total time spent in backoff sleeps across direct-fetch retries (microseconds).
    direct_retry_sleep_us: AtomicU64,

    /// Worst-case attempt number reached for any single merged request.
    direct_max_attempt: AtomicU64,

    /// Number of byte ranges generated by the batch splitter before gap-merging.
    direct_original_ranges: AtomicU64,

    /// Number of merged HTTP Range requests actually issued after gap-merging.
    direct_merged_ranges: AtomicU64,
}

impl SharedFetchStats {
    /// Take a snapshot of the counters after every outer fetch task has flushed.
    fn snapshot(&mut self) -> TaskFetchStats {
        // &mut self means we can skip the atomic-load barriers
        TaskFetchStats {
            grpc_requests: *self.grpc_requests.get_mut(),
            grpc_bytes: *self.grpc_bytes.get_mut(),
            direct_requests: *self.direct_requests.get_mut(),
            direct_bytes: *self.direct_bytes.get_mut(),
            direct_retries_total: *self.direct_retries_total.get_mut(),
            direct_requests_retried: *self.direct_requests_retried.get_mut(),
            direct_retry_sleep_us: *self.direct_retry_sleep_us.get_mut(),
            direct_max_attempt: *self.direct_max_attempt.get_mut(),
            direct_original_ranges: *self.direct_original_ranges.get_mut(),
            direct_merged_ranges: *self.direct_merged_ranges.get_mut(),
        }
    }
}

/// Tracks a query in progress. Accumulates fetch stats from all partitions
/// and sends a single combined analytics event when the last clone is dropped.
///
/// Cheap to clone (wraps an `Arc`).
#[derive(Clone)]
pub(crate) struct PendingQueryAnalytics {
    inner: Arc<PendingInner>,
}

impl std::fmt::Debug for PendingQueryAnalytics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingQueryAnalytics")
            .finish_non_exhaustive()
    }
}

struct PendingInner {
    connection: ConnectionAnalytics,
    query_info: QueryInfo,
    fetch_stats: SharedFetchStats,

    /// Monotonic start time of the query, for computing elapsed durations.
    scan_start: web_time::Instant,

    /// Time from scan start until the first chunk is returned to datafusion.
    time_to_first_chunk: OnceLock<Duration>,

    /// First terminal direct-fetch failure reason encountered, if any.
    /// Only set once. Stored as `&'static str` from the bounded
    /// [`DirectFetchFailureReason`] label set.
    direct_terminal_reason: std::sync::OnceLock<DirectFetchFailureReason>,

    /// Error classification, if the query failed. `None` ⇒ success.
    /// Stored as `&'static str` from [`QueryErrorKind::as_str`] so emission is zero-copy.
    error_kind: std::sync::OnceLock<&'static str>,
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
#[cfg_attr(target_arch = "wasm32", expect(dead_code))]
pub(crate) enum DirectFetchFailureReason {
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
    pub(crate) fn as_str(self) -> &'static str {
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
    /// Record that the first result chunk has been returned to the user.
    /// Only the first call has any effect.
    #[cfg_attr(target_arch = "wasm32", expect(dead_code))]
    pub fn record_first_chunk(&self) {
        self.inner
            .time_to_first_chunk
            .get_or_init(|| self.inner.scan_start.elapsed());
    }

    /// Access the shared [`SharedFetchStats`] sink. Used by [`TaskFetchStats::flush_into`].
    pub(crate) fn fetch_stats(&self) -> &SharedFetchStats {
        &self.inner.fetch_stats
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
}

/// Per-task accumulator for fetch counters.
///
/// Each outer fetch task owns one of these and mutates it without synchronization.
/// At the end of the task it is folded into the shared [`SharedFetchStats`] via
/// [`TaskFetchStats::flush_into`], which is the only place the shared cache line
/// is touched.
///
/// This avoids cross-core cache-line ping-pong on the shared atomics during the
/// hot fetch/retry loops, where worst-case contention would otherwise be
/// `inner_concurrency × outer_concurrency × num_partitions`.
#[derive(Default)]
#[must_use]
pub(crate) struct TaskFetchStats {
    grpc_requests: u64,
    grpc_bytes: u64,
    direct_requests: u64,
    direct_bytes: u64,
    direct_retries_total: u64,
    direct_requests_retried: u64,
    direct_retry_sleep_us: u64,
    direct_max_attempt: u64,
    direct_original_ranges: u64,
    direct_merged_ranges: u64,
}

#[cfg_attr(target_arch = "wasm32", expect(dead_code))]
impl TaskFetchStats {
    /// Record a gRPC fetch.
    pub fn record_grpc_fetch(&mut self, bytes: u64) {
        self.grpc_requests += 1;
        self.grpc_bytes += bytes;
    }

    /// Record a direct (HTTP) fetch.
    pub fn record_direct_fetch(&mut self, bytes: u64) {
        self.direct_requests += 1;
        self.direct_bytes += bytes;
    }

    /// Record a single direct-fetch retry on one merged request.
    ///
    /// `sleep` is the backoff duration actually slept before the retry attempt.
    /// `attempt` is the attempt number about to be made (starts at 2 for the first retry).
    pub fn record_direct_retry(&mut self, sleep: Duration, attempt: u64) {
        self.direct_retries_total += 1;
        self.direct_retry_sleep_us += sleep.as_micros() as u64;
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
            grpc_requests,
            grpc_bytes,
            direct_requests,
            direct_bytes,
            direct_retries_total,
            direct_requests_retried,
            direct_retry_sleep_us,
            direct_max_attempt,
            direct_original_ranges,
            direct_merged_ranges,
        } = other;
        self.grpc_requests += grpc_requests;
        self.grpc_bytes += grpc_bytes;
        self.direct_requests += direct_requests;
        self.direct_bytes += direct_bytes;
        self.direct_retries_total += direct_retries_total;
        self.direct_requests_retried += direct_requests_retried;
        self.direct_retry_sleep_us += direct_retry_sleep_us;
        self.direct_max_attempt = self.direct_max_attempt.max(direct_max_attempt);
        self.direct_original_ranges += direct_original_ranges;
        self.direct_merged_ranges += direct_merged_ranges;
    }

    /// Fold this buffer into the shared atomic sink.
    pub fn flush_into(self, shared: &SharedFetchStats) {
        macro_rules! flush_stats {
            {sum $($sum_id:ident),*; max $($max_id:ident),*;} => {
                let Self {
                    $($sum_id,)*
                    $($max_id,)*
                } = self;
                $(
                    // Zero-valued fields are skipped so totally-idle tasks don't touch the
                    // shared cache line at all.
                    if $sum_id != 0 {
                        shared.$sum_id
                            .fetch_add($sum_id, Ordering::Relaxed);
                    }
                )+
                $(
                    if $max_id != 0 {
                        shared.$max_id
                            .fetch_max($max_id, Ordering::Relaxed);
                    }
                )*
            };
        }
        flush_stats! {
            sum
                grpc_requests,
                grpc_bytes,
                direct_requests,
                direct_bytes,
                direct_retries_total,
                direct_requests_retried,
                direct_retry_sleep_us,
                direct_original_ranges,
                direct_merged_ranges;
            max
                direct_max_attempt;
        };
    }

    /// Flush this buffer into `analytics` if present, also recording an error (if any).
    pub fn try_flush_into(
        self,
        analytics: Option<&PendingQueryAnalytics>,
        result: Result<(), QueryErrorKind>,
    ) {
        if let Some(analytics) = analytics {
            self.flush_into(analytics.fetch_stats());
            if let Err(err) = result {
                analytics.record_error(err);
            }
        }
    }
}

impl Drop for PendingInner {
    fn drop(&mut self) {
        let Self {
            connection,
            query_info,
            fetch_stats,
            scan_start,
            time_to_first_chunk,
            direct_terminal_reason,
            error_kind,
        } = self;

        let total_duration = scan_start.elapsed();

        let QueryInfo {
            ref dataset_id,
            query_chunks,
            query_segments,
            query_layers,
            query_columns,
            query_entities,
            query_bytes,
            query_chunks_per_segment_max,
            query_chunks_per_segment_mean,
            query_type,
            ref primary_index_name,
            ref time_range,
            time_to_first_chunk_info,
            trace_id,
        } = *query_info;

        let fetch = fetch_stats.snapshot();
        let time_to_first_chunk = time_to_first_chunk.get().copied();
        let direct_terminal_reason = direct_terminal_reason.get().copied();
        let error_kind = error_kind.get().copied();

        let [start_ns, end_ns] = [
            nanos_since_epoch(&time_range.start),
            nanos_since_epoch(&time_range.end),
        ];

        #[expect(
            clippy::cast_possible_wrap,
            reason = "OTLP proto uses i64 for int values"
        )]
        let mut attributes = vec![
            kv_string("dataset_id", dataset_id),
            kv_int("query_chunks", query_chunks as i64),
            kv_int("query_segments", query_segments as i64),
            kv_int("query_layers", query_layers as i64),
            kv_int("query_columns", query_columns as i64),
            kv_int("query_entities", query_entities as i64),
            kv_int("query_bytes", query_bytes as i64),
            kv_int(
                "query_chunks_per_segment_max",
                i64::from(query_chunks_per_segment_max),
            ),
            kv_double(
                "query_chunks_per_segment_mean",
                f64::from(query_chunks_per_segment_mean),
            ),
            kv_string("query_type", query_type.as_str()),
            kv_int("total_duration_us", total_duration.as_micros() as i64),
            kv_bool("is_success", error_kind.is_none()),
            // Fetch stats: gRPC
            kv_int("fetch_grpc_requests", fetch.grpc_requests as i64),
            kv_int("fetch_grpc_bytes", fetch.grpc_bytes as i64),
            // Fetch stats: direct (HTTP). Note: gRPC retries happen at the transport
            // layer and are not visible here — only direct-URL retries are counted.
            kv_int("fetch_direct_requests", fetch.direct_requests as i64),
            kv_int("fetch_direct_bytes", fetch.direct_bytes as i64),
            kv_int("fetch_direct_retries", fetch.direct_retries_total as i64),
            kv_int(
                "fetch_direct_requests_retried",
                fetch.direct_requests_retried as i64,
            ),
            kv_int(
                "fetch_direct_retry_sleep_us",
                fetch.direct_retry_sleep_us as i64,
            ),
            kv_int("fetch_direct_max_attempt", fetch.direct_max_attempt as i64),
            kv_int(
                "fetch_direct_original_ranges",
                fetch.direct_original_ranges as i64,
            ),
            kv_int(
                "fetch_direct_merged_ranges",
                fetch.direct_merged_ranges as i64,
            ),
        ];

        if let Some(name) = primary_index_name {
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

        if let Some(version) = connection.server_version() {
            attributes.push(kv_string("server_version", &version));
        }

        let links = trace_id
            .map(|id| {
                vec![Link {
                    trace_id: id.to_bytes().to_vec(),
                    ..Default::default()
                }]
            })
            .unwrap_or_default();

        let span = Span {
            name: "cloud_query_dataset".to_owned(),
            kind: SpanKind::Client.into(),
            start_time_unix_nano: start_ns,
            end_time_unix_nano: end_ns,
            attributes,
            links,
            ..Default::default()
        };

        connection.send_span(span, trace_id);
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

/// Underlying provider variant of a table entry.
///
/// Bounded so the analytics dimension cardinality stays low. Add a variant if
/// the Data Platform exposes a new system or storage backend.
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
}

/// Accumulates per-scan counters from the streaming-provider IO loop.
///
/// Not contended in practice today (one stream per scan) but kept atomic for
/// consistency with [`SharedFetchStats`] and to leave room for parallelism.
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

impl std::fmt::Debug for PendingTableQueryAnalytics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingTableQueryAnalytics")
            .finish_non_exhaustive()
    }
}

struct PendingTableInner {
    connection: ConnectionAnalytics,
    info: TableQueryInfo,
    stats: SharedTableScanStats,

    /// Monotonic start time of the scan, for computing elapsed durations.
    scan_start: web_time::Instant,

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
        self.inner.build_span()
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
        let scan_end_wall = web_time::SystemTime::now();
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
            self.connection.server_version().as_deref(),
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
#[expect(
    clippy::too_many_arguments,
    reason = "pure builder fn; grouping these would be churn without clarity"
)]
pub(crate) fn build_table_query_span(
    info: &TableQueryInfo,
    stats: TableScanStatsSnapshot,
    wall_clock_range: Range<SystemTime>,
    total_duration: Duration,
    time_to_first_response: Option<Duration>,
    time_to_first_batch: Option<Duration>,
    trace_id: Option<opentelemetry::TraceId>,
    error_kind: Option<&'static str>,
    server_version: Option<&str>,
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

    if let Some(version) = server_version {
        attributes.push(kv_string("server_version", version));
    }

    let links = trace_id
        .map(|id| {
            vec![Link {
                trace_id: id.to_bytes().to_vec(),
                ..Default::default()
            }]
        })
        .unwrap_or_default();

    Span {
        name: "cloud_scan_table".to_owned(),
        kind: SpanKind::Client.into(),
        start_time_unix_nano,
        end_time_unix_nano,
        attributes,
        links,
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
    }
}

fn kv_int(key: &str, value: i64) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        value: Some(AnyValue {
            value: Some(Value::IntValue(value)),
        }),
    }
}

fn kv_bool(key: &str, value: bool) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        value: Some(AnyValue {
            value: Some(Value::BoolValue(value)),
        }),
    }
}

fn kv_double(key: &str, value: f64) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        value: Some(AnyValue {
            value: Some(Value::DoubleValue(value)),
        }),
    }
}

#[cfg(test)]
mod table_query_tests {
    use std::collections::HashSet;

    use re_protos::cloud::v1alpha1::ext::{LanceTable, ProviderDetails, SystemTable};

    use super::*;

    fn lance_provider_details() -> ProviderDetails {
        // Construct via the protobuf type so we don't need a direct `url`
        // dependency in this crate just for tests.
        let proto = re_protos::cloud::v1alpha1::LanceTable {
            table_url: "s3://bucket/path".to_owned(),
        };
        ProviderDetails::LanceTable(LanceTable::try_from(proto).unwrap())
    }

    // ---- helpers ----

    fn dummy_table_query_info() -> TableQueryInfo {
        TableQueryInfo {
            table_id: "tbl-42".to_owned(),
            table_kind: TableKind::Lance,
            caller: TableQueryCaller::BrowserDetailView,
            schema_total_columns: 12,
            projected_columns: 5,
            has_limit: false,
            limit_value: None,
            time_range: SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        }
    }

    fn empty_stats() -> TableScanStatsSnapshot {
        TableScanStatsSnapshot::default()
    }

    fn attribute_keys(span: &Span) -> HashSet<&str> {
        let keys: HashSet<_> = span.attributes.iter().map(|kv| kv.key.as_str()).collect();
        assert_eq!(
            keys.len(),
            span.attributes.len(),
            "span contains duplicate attribute keys"
        );
        keys
    }

    fn find_int(span: &Span, key: &str) -> Option<i64> {
        span.attributes
            .iter()
            .find(|kv| kv.key == key)
            .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
                Value::IntValue(i) => Some(*i),
                _ => None,
            })
    }

    fn find_string<'a>(span: &'a Span, key: &str) -> Option<&'a str> {
        span.attributes
            .iter()
            .find(|kv| kv.key == key)
            .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
                Value::StringValue(s) => Some(s.as_str()),
                _ => None,
            })
    }

    fn find_bool(span: &Span, key: &str) -> Option<bool> {
        span.attributes
            .iter()
            .find(|kv| kv.key == key)
            .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
                Value::BoolValue(b) => Some(*b),
                _ => None,
            })
    }

    /// Required attributes that must always be emitted, regardless of scan
    /// outcome. Adding/removing one of these is a breaking change for the
    /// analytics pipeline (`PostHog` dashboards, server-side enrichment, etc.).
    const REQUIRED_KEYS: &[&str] = &[
        "table_id",
        "table_kind",
        "caller",
        "schema_total_columns",
        "projected_columns",
        "has_limit",
        "is_success",
        "total_duration_us",
        "fetch_grpc_requests",
        "num_record_batches",
        "rows_returned",
        "bytes_returned",
    ];

    // ---- builder shape ----

    #[test]
    fn build_table_query_span_minimal_emits_only_required_attributes() {
        let info = dummy_table_query_info();

        let span = build_table_query_span(
            &info,
            empty_stats(),
            SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            Duration::from_micros(500),
            None,
            None,
            None,
            None,
            None,
        );

        // Span shape
        assert_eq!(span.name, "cloud_scan_table");
        assert_eq!(span.kind, i32::from(SpanKind::Client));
        assert!(span.links.is_empty());

        // Attribute key set is exactly the required keys — no optional keys leaked.
        let expected: HashSet<&str> = REQUIRED_KEYS.iter().copied().collect();
        let actual = attribute_keys(&span);
        assert_eq!(
            actual,
            expected,
            "extra/missing attribute keys: {:?}",
            actual.symmetric_difference(&expected).collect::<Vec<_>>()
        );

        // Spot-check key values from the dummy info.
        assert_eq!(find_string(&span, "table_id"), Some("tbl-42"));
        assert_eq!(find_string(&span, "table_kind"), Some("lance"));
        assert_eq!(find_string(&span, "caller"), Some("browser_detail_view"));
        assert_eq!(find_int(&span, "schema_total_columns"), Some(12));
        assert_eq!(find_int(&span, "projected_columns"), Some(5));
        assert_eq!(find_bool(&span, "has_limit"), Some(false));
        assert_eq!(find_bool(&span, "is_success"), Some(true));
        assert_eq!(find_int(&span, "total_duration_us"), Some(500));
    }

    #[test]
    fn build_table_query_span_records_scan_stats() {
        let info = dummy_table_query_info();
        let stats = TableScanStatsSnapshot {
            grpc_requests: 7,
            batches: 7,
            rows_returned: 12_345,
            bytes_returned: 4_567_890,
        };

        let span = build_table_query_span(
            &info,
            stats,
            SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            Duration::from_micros(2_000),
            None,
            None,
            None,
            None,
            None,
        );

        assert_eq!(find_int(&span, "fetch_grpc_requests"), Some(7));
        assert_eq!(find_int(&span, "num_record_batches"), Some(7));
        assert_eq!(find_int(&span, "rows_returned"), Some(12_345));
        assert_eq!(find_int(&span, "bytes_returned"), Some(4_567_890));
    }

    #[test]
    fn build_table_query_span_emits_optional_attributes_when_present() {
        let trace_id = opentelemetry::TraceId::from_bytes([3u8; 16]);
        let mut info = dummy_table_query_info();
        info.has_limit = true;
        info.limit_value = Some(500);

        let span = build_table_query_span(
            &info,
            empty_stats(),
            SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            Duration::from_micros(1_000),
            Some(Duration::from_micros(50)),
            Some(Duration::from_micros(75)),
            Some(trace_id),
            Some(QueryErrorKind::Decode.as_str()),
            Some("redap-9.9.9"),
        );

        // All optional keys are present.
        let optional = [
            "limit_value",
            "time_to_first_response_us",
            "time_to_first_batch_us",
            "error_kind",
            "server_version",
        ];
        let keys = attribute_keys(&span);
        for k in optional {
            assert!(keys.contains(k), "missing optional attribute: {k}");
        }

        // is_success flips to false when error_kind is set.
        assert_eq!(find_bool(&span, "is_success"), Some(false));

        assert_eq!(find_int(&span, "limit_value"), Some(500));
        assert_eq!(find_int(&span, "time_to_first_response_us"), Some(50));
        assert_eq!(find_int(&span, "time_to_first_batch_us"), Some(75));
        assert_eq!(find_string(&span, "error_kind"), Some("decode"));
        assert_eq!(find_string(&span, "server_version"), Some("redap-9.9.9"));

        // Trace id flows into span.links.
        assert_eq!(span.links.len(), 1);
        assert_eq!(span.links[0].trace_id, trace_id.to_bytes().to_vec());
    }

    #[test]
    fn build_table_query_span_uses_wall_clock_range() {
        let info = dummy_table_query_info();
        let start = SystemTime::UNIX_EPOCH + Duration::from_millis(2_000);
        let end = SystemTime::UNIX_EPOCH + Duration::from_millis(2_500);

        let span = build_table_query_span(
            &info,
            empty_stats(),
            start..end,
            Duration::from_micros(0),
            None,
            None,
            None,
            None,
            None,
        );

        assert_eq!(span.start_time_unix_nano, 2_000_000_000);
        assert_eq!(span.end_time_unix_nano, 2_500_000_000);
    }

    #[test]
    fn build_table_query_span_records_table_kind_and_caller_strings() {
        // Walk every variant — protects the bounded-enum string mapping from
        // accidental changes that would silently rename PostHog dimensions.
        let cases = [
            (TableKind::Lance, "lance"),
            (TableKind::SystemEntries, "system_entries"),
            (TableKind::SystemNamespaces, "system_namespaces"),
            (TableKind::Unknown, "unknown"),
        ];
        for (kind, expected) in cases {
            let mut info = dummy_table_query_info();
            info.table_kind = kind;
            let span = build_table_query_span(
                &info,
                empty_stats(),
                SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH,
                Duration::ZERO,
                None,
                None,
                None,
                None,
                None,
            );
            assert_eq!(find_string(&span, "table_kind"), Some(expected));
        }

        let cases = [
            (TableQueryCaller::CatalogResolver, "catalog_resolver"),
            (TableQueryCaller::EntriesTable, "entries_table"),
            (TableQueryCaller::BrowserDetailView, "browser_detail_view"),
        ];
        for (caller, expected) in cases {
            let mut info = dummy_table_query_info();
            info.caller = caller;
            let span = build_table_query_span(
                &info,
                empty_stats(),
                SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH,
                Duration::ZERO,
                None,
                None,
                None,
                None,
                None,
            );
            assert_eq!(find_string(&span, "caller"), Some(expected));
        }
    }

    #[test]
    fn build_table_query_span_no_limit_value_when_no_limit() {
        // has_limit defaults to false, limit_value to None — the optional
        // `limit_value` attribute must NOT be emitted.
        let info = dummy_table_query_info();
        let span = build_table_query_span(
            &info,
            empty_stats(),
            SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH,
            Duration::ZERO,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(!attribute_keys(&span).contains("limit_value"));
    }

    // ---- TableKind::from(&ProviderDetails) ----

    #[test]
    fn table_kind_from_lance_provider() {
        assert!(matches!(
            TableKind::from(&lance_provider_details()),
            TableKind::Lance
        ));
    }

    #[test]
    fn table_kind_from_system_entries_provider() {
        let pd = ProviderDetails::SystemTable(SystemTable {
            kind: SystemTableKind::Entries,
        });
        assert!(matches!(TableKind::from(&pd), TableKind::SystemEntries));
    }

    #[test]
    fn table_kind_from_system_namespaces_provider() {
        let pd = ProviderDetails::SystemTable(SystemTable {
            kind: SystemTableKind::Namespaces,
        });
        assert!(matches!(TableKind::from(&pd), TableKind::SystemNamespaces));
    }

    #[test]
    fn table_kind_from_system_unspecified_falls_back_to_unknown() {
        let pd = ProviderDetails::SystemTable(SystemTable {
            kind: SystemTableKind::Unspecified,
        });
        assert!(matches!(TableKind::from(&pd), TableKind::Unknown));
    }

    // ---- record_* idempotence ----
    //
    // All `record_*` setters are advertised as "only the first call has effect".
    // These tests pin that contract; subsequent calls must not overwrite.

    fn make_pending() -> PendingTableQueryAnalytics {
        let origin: Origin = "rerun+http://localhost:51234".parse().unwrap();
        let client = re_redap_client::ConnectionClient::new_disconnected();
        let analytics = ConnectionAnalytics::new(origin, &client);
        analytics.begin_table_query(dummy_table_query_info(), web_time::Instant::now())
    }

    #[tokio::test]
    async fn record_first_response_is_once_only() {
        let pending = make_pending();
        pending.record_first_response();
        let first = pending.inner.time_to_first_response.get().copied().unwrap();
        std::thread::sleep(Duration::from_millis(2));
        pending.record_first_response();
        let second = pending.inner.time_to_first_response.get().copied().unwrap();
        assert_eq!(first, second, "second call must not overwrite");
    }

    #[tokio::test]
    async fn record_first_batch_is_once_only() {
        let pending = make_pending();
        pending.record_first_batch();
        let first = pending.inner.time_to_first_batch.get().copied().unwrap();
        std::thread::sleep(Duration::from_millis(2));
        pending.record_first_batch();
        let second = pending.inner.time_to_first_batch.get().copied().unwrap();
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn record_error_is_once_only() {
        let pending = make_pending();
        pending.record_error(QueryErrorKind::GrpcFetch);
        pending.record_error(QueryErrorKind::Decode);
        assert_eq!(
            pending.inner.error_kind.get().copied(),
            Some(QueryErrorKind::GrpcFetch.as_str())
        );
    }

    #[tokio::test]
    async fn record_trace_id_is_once_only() {
        let pending = make_pending();
        let first = opentelemetry::TraceId::from_bytes([1u8; 16]);
        let second = opentelemetry::TraceId::from_bytes([2u8; 16]);
        pending.record_trace_id(first);
        pending.record_trace_id(second);
        assert_eq!(pending.inner.trace_id.get().copied(), Some(first));
    }

    #[tokio::test]
    async fn record_batch_accumulates_across_calls() {
        let pending = make_pending();
        pending.record_batch(100, 1_000);
        pending.record_batch(50, 500);
        pending.record_batch(0, 0); // empty batch — still counts a request/batch
        let stats = pending.inner.stats.snapshot();
        assert_eq!(stats.grpc_requests, 3);
        assert_eq!(stats.batches, 3);
        assert_eq!(stats.rows_returned, 150);
        assert_eq!(stats.bytes_returned, 1_500);
    }
}
