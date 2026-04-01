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
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use opentelemetry_proto::tonic::{
    collector::trace::v1::{ExportTraceServiceRequest, ExportTraceServiceResponse},
    common::v1::any_value::Value,
    common::v1::{AnyValue, KeyValue},
    resource::v1::Resource,
    trace::v1::{ResourceSpans, ScopeSpans, Span, span::Link, span::SpanKind},
};
use re_uri::Origin;
use web_time::{Duration, SystemTime};

#[cfg(not(target_arch = "wasm32"))]
type Channel = tonic::transport::Channel;

#[cfg(target_arch = "wasm32")]
type Channel = tonic_web_wasm_client::Client;

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
pub struct ConnectionAnalytics {
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
    client: tokio::sync::Mutex<Option<tonic::client::Grpc<Channel>>>,
}

impl ConnectionAnalytics {
    /// Create a new analytics sender for the given origin.
    ///
    /// The actual gRPC connection is established lazily on first use.
    pub fn new(origin: Origin) -> Self {
        Self {
            inner: Arc::new(Inner {
                origin,
                client: tokio::sync::Mutex::new(None),
            }),
        }
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
                fetch_stats: FetchStats::default(),
                scan_start,
                time_to_first_chunk: parking_lot::Mutex::new(None),
            }),
        }
    }

    /// Send an OTLP span in the background. Never blocks the caller.
    fn send_span(&self, span: Span, trace_id: Option<opentelemetry::TraceId>) {
        let this = self.clone();

        let fut = async move {
            if let Err(err) = this.send_span_impl(span, trace_id).await {
                re_log::debug_once!("Failed to send analytics to Rerun Cloud: {err}");
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
        let mut guard = self.inner.client.lock().await;

        let grpc = if let Some(grpc) = guard.as_mut() {
            grpc
        } else {
            match re_redap_client::channel(self.inner.origin.clone()).await {
                Ok(channel) => guard.get_or_insert(tonic::client::Grpc::new(channel)),
                Err(err) => {
                    return Err(tonic::Status::unavailable(format!(
                        "failed to connect for analytics: {err}"
                    )));
                }
            }
        };

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

    /// Number of columns in the query output schema.
    pub query_columns: usize,

    /// Number of entity paths in the query request.
    pub query_entities: usize,

    /// Total size of all queried chunks in bytes (from chunk metadata).
    pub query_bytes: u64,

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
#[derive(Default)]
struct FetchStats {
    grpc_requests: AtomicU64,
    grpc_bytes: AtomicU64,
    direct_requests: AtomicU64,
    direct_bytes: AtomicU64,
}

impl FetchStats {
    fn snapshot(&self) -> FetchStatsSnapshot {
        FetchStatsSnapshot {
            grpc_requests: self.grpc_requests.load(Ordering::Relaxed),
            grpc_bytes: self.grpc_bytes.load(Ordering::Relaxed),
            direct_requests: self.direct_requests.load(Ordering::Relaxed),
            direct_bytes: self.direct_bytes.load(Ordering::Relaxed),
        }
    }
}

struct FetchStatsSnapshot {
    grpc_requests: u64,
    grpc_bytes: u64,
    direct_requests: u64,
    direct_bytes: u64,
}

/// Tracks a query in progress. Accumulates fetch stats from all partitions
/// and sends a single combined analytics event when the last clone is dropped.
///
/// Cheap to clone (wraps an `Arc`).
#[derive(Clone)]
pub struct PendingQueryAnalytics {
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
    fetch_stats: FetchStats,

    /// Monotonic start time of the query, for computing elapsed durations.
    scan_start: web_time::Instant,

    /// Time from scan start until the first chunk is returned to datafusion.
    time_to_first_chunk: parking_lot::Mutex<Option<Duration>>,
}

impl PendingQueryAnalytics {
    /// Record that the first result chunk has been returned to the user.
    /// Only the first call has any effect.
    pub fn record_first_chunk(&self) {
        let mut guard = self.inner.time_to_first_chunk.lock();
        guard.get_or_insert_with(|| self.inner.scan_start.elapsed());
    }

    /// Record a gRPC fetch.
    pub fn record_grpc_fetch(&self, bytes: u64) {
        self.inner
            .fetch_stats
            .grpc_requests
            .fetch_add(1, Ordering::Relaxed);
        self.inner
            .fetch_stats
            .grpc_bytes
            .fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record a direct (HTTP) fetch.
    pub fn record_direct_fetch(&self, bytes: u64) {
        self.inner
            .fetch_stats
            .direct_requests
            .fetch_add(1, Ordering::Relaxed);
        self.inner
            .fetch_stats
            .direct_bytes
            .fetch_add(bytes, Ordering::Relaxed);
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
        } = self;

        let total_duration = scan_start.elapsed();

        let QueryInfo {
            ref dataset_id,
            query_chunks,
            query_segments,
            query_columns,
            query_entities,
            query_bytes,
            ref time_range,
            time_to_first_chunk_info,
            trace_id,
        } = *query_info;

        let fetch = fetch_stats.snapshot();
        let time_to_first_chunk = time_to_first_chunk.lock().take();

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
            kv_int("query_columns", query_columns as i64),
            kv_int("query_entities", query_entities as i64),
            kv_int("query_bytes", query_bytes as i64),
            kv_int("total_duration_us", total_duration.as_micros() as i64),
            // Fetch stats: gRPC
            kv_int("fetch_grpc_requests", fetch.grpc_requests as i64),
            kv_int("fetch_grpc_bytes", fetch.grpc_bytes as i64),
            // Fetch stats: direct (HTTP)
            kv_int("fetch_direct_requests", fetch.direct_requests as i64),
            kv_int("fetch_direct_bytes", fetch.direct_bytes as i64),
        ];

        if let Some(ttfci) = time_to_first_chunk_info {
            attributes.push(kv_int(
                "time_to_first_chunk_info_us",
                ttfci.as_micros() as i64,
            ));
        }

        if let Some(ttfr) = time_to_first_chunk {
            attributes.push(kv_int("time_to_first_chunk_us", ttfr.as_micros() as i64));
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
