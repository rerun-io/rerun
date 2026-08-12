//! In-memory OTLP `TraceService::Export` sink for tests.
//!
//! Spawns a real tonic server bound to an OS-assigned `127.0.0.1` port,
//! records every incoming `Export` request (with its gRPC metadata) into a
//! shared buffer, and exposes a notification-driven `wait_for` helper so
//! tests don't need sleep-based polling.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
    trace_service_server::{TraceService, TraceServiceServer},
};
use opentelemetry_proto::tonic::common::v1::InstrumentationScope;
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
use parking_lot::Mutex;
use tokio::sync::{Notify, oneshot};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

/// A single span observed by the sink, flattened out of the
/// `ResourceSpans`/`ScopeSpans` nesting that the OTLP wire format imposes.
///
/// One incoming `Export` RPC carrying N spans produces N
/// [`ReceivedSpan`]s, each with the same `metadata` clone but its own
/// `resource`/`scope`/`span`. This is the granularity tests reason about:
/// a `wait_for` predicate looks at one span at a time, and pops exactly
/// that one when it matches — leaving any siblings from the same batch in
/// the buffer for follow-up matches.
///
/// The bad-request case present in [`super::posthog::ReceivedEvent`] does
/// not exist here: tonic decodes proto bodies upstream of our handler, so
/// any malformed `Export` is rejected with `InvalidArgument` before we
/// ever see it.
#[derive(Clone, Debug)]
pub struct ReceivedSpan {
    pub metadata: tonic::metadata::MetadataMap,
    pub resource: Option<Resource>,
    pub scope: Option<InstrumentationScope>,
    pub span: Span,
}

#[derive(Default)]
struct State {
    received: Mutex<Vec<ReceivedSpan>>,
    notify: Notify,
}

struct CollectorService {
    state: Arc<State>,
}

// IMPORTANT: this handler records the request *before* returning its response,
// so by the time a client's `c.export(…).await` returns Ok, every
// [`ReceivedSpan`] flattened out of that request is already in the buffer.
// Tests can assert on `received()` immediately after the client `.await` —
// no `wait_for` needed.
#[tonic::async_trait]
impl TraceService for CollectorService {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        let (metadata, _ext, payload) = request.into_parts();
        {
            // Flatten the `ResourceSpans`→`ScopeSpans`→`spans` nesting into
            // one [`ReceivedSpan`] per individual span. Push the whole batch
            // under a single lock so observers never see a partially-applied
            // export; notify once at the end.
            let mut buffer = self.state.received.lock();
            for ResourceSpans {
                resource,
                scope_spans,
                ..
            } in payload.resource_spans
            {
                for ScopeSpans { scope, spans, .. } in scope_spans {
                    for span in spans {
                        buffer.push(ReceivedSpan {
                            metadata: metadata.clone(),
                            resource: resource.clone(),
                            scope: scope.clone(),
                            span,
                        });
                    }
                }
            }
        }
        self.state.notify.notify_waiters();
        Ok(Response::new(ExportTraceServiceResponse::default()))
    }
}

/// In-process OTLP `TraceService` server that records every received `Export`
/// request for test assertions.
///
/// Drop is fire-and-forget; use [`Self::shutdown`] for graceful teardown
/// that awaits the server task.
pub struct MockOtlpCollector {
    addr: SocketAddr,
    state: Arc<State>,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
}

/// Returned by [`MockOtlpCollector::wait_for`] when no buffered span
/// matched within the timeout.
///
/// `snapshot` is a clone of whatever spans were in the buffer at the
/// timeout instant — the buffer itself is left untouched, so a follow-up
/// `wait_for` call can resume waiting against the same buffer.
#[derive(Debug)]
pub struct OtlpWaitTimeout {
    pub snapshot: Vec<ReceivedSpan>,
}

impl std::fmt::Display for OtlpWaitTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "timed out waiting for matching span; buffer holds {} span(s)",
            self.snapshot.len()
        )
    }
}

impl std::error::Error for OtlpWaitTimeout {}

impl MockOtlpCollector {
    /// Bind to an OS-assigned port on `127.0.0.1` and start serving.
    ///
    /// Returns only after the server task has begun executing, so subsequent
    /// requests are not racing the spawned task's first poll.
    pub async fn spawn() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind 127.0.0.1:0");
        let addr = listener.local_addr().expect("local_addr");
        let state = Arc::new(State::default());
        let service = CollectorService {
            state: state.clone(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
        let join = tokio::spawn(async move {
            // Signal that the task has begun executing; the very next thing
            // we do is await the serve future, which begins polling the
            // incoming stream. By the time `spawn()` returns, tonic is about
            // to (or already is) accepting from the kernel queue.
            _ = ready_tx.send(());
            drop(
                Server::builder()
                    .add_service(
                        TraceServiceServer::new(service)
                            .accept_compressed(tonic::codec::CompressionEncoding::Gzip)
                            .send_compressed(tonic::codec::CompressionEncoding::Gzip),
                    )
                    .serve_with_incoming_shutdown(incoming, async {
                        drop(shutdown_rx.await);
                    })
                    .await,
            );
        });
        _ = ready_rx.await;

        Self {
            addr,
            state,
            shutdown: Some(shutdown_tx),
            join: Some(join),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// `http://127.0.0.1:PORT`, suitable for tonic / OTLP exporter config.
    ///
    /// Plaintext gRPC only — no TLS support. If a caller wraps an HTTPS client
    /// around this, connection will fail; that's a test setup bug, not a
    /// mock limitation.
    pub fn endpoint(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Snapshot of all buffered spans (cloned out, buffer untouched).
    pub fn received(&self) -> Vec<ReceivedSpan> {
        self.state.received.lock().clone()
    }

    /// `true` if no spans are currently buffered (initial state, fully
    /// drained, or [`Self::clear`]'d).
    pub fn is_empty(&self) -> bool {
        self.state.received.lock().is_empty()
    }

    pub fn clear(&self) {
        self.state.received.lock().clear();
    }

    /// Wait for, and consume, the next buffered span that satisfies
    /// `predicate`. On success the matched span is `remove`d from the
    /// buffer in arrival order and returned; siblings in the same batch
    /// stay in place. On timeout the buffer is left untouched and the
    /// failure surfaces the current buffer contents through
    /// [`OtlpWaitTimeout::snapshot`].
    ///
    /// Notification-driven: returns ~immediately once a matching span is
    /// in the buffer. If multiple buffered spans match, the earliest one
    /// wins; subsequent calls can pop the next match.
    ///
    /// The pop is the consumption point: anything left after a sequence
    /// of `wait_for` calls is genuinely surplus — a stray retransmit, an
    /// unexpected span the test forgot to assert on, etc. — so closing a
    /// test with [`crate::assert_sink_empty!`] is a meaningful check.
    pub async fn wait_for<F>(
        &self,
        predicate: F,
        timeout: Duration,
    ) -> Result<ReceivedSpan, OtlpWaitTimeout>
    where
        F: Fn(&ReceivedSpan) -> bool,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            // Arm the waiter *before* the buffer scan so a push racing
            // between scan and await cannot be missed.
            let notified = self.state.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            {
                let mut buffer = self.state.received.lock();
                if let Some(pos) = buffer.iter().position(&predicate) {
                    return Ok(buffer.remove(pos));
                }
            }

            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return Err(OtlpWaitTimeout {
                    snapshot: self.received(),
                });
            }
        }
    }

    /// Graceful teardown: signal shutdown and await the server task to
    /// complete. Use this at the end of tests that send fire-and-forget
    /// requests to guarantee in-flight responses are fully processed before
    /// the tokio runtime tears down.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            _ = join.await;
        }
    }
}

impl Drop for MockOtlpCollector {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            _ = tx.send(());
        }
        // `join` is dropped; tokio detaches the task to run to completion.
        // Tests that need to verify graceful shutdown should call
        // `shutdown().await` explicitly instead of relying on drop.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry_proto::tonic::collector::trace::v1::trace_service_client::TraceServiceClient;
    use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue, any_value::Value};
    use opentelemetry_proto::tonic::resource::v1::Resource;
    use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};

    /// Build a request carrying `names.len()` spans in a single
    /// `ScopeSpans`, so a single `Export` produces N `ReceivedSpan`s in
    /// arrival order — exercises the flatten path.
    fn export_with_span_names(names: &[&str]) -> ExportTraceServiceRequest {
        ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource {
                    attributes: vec![KeyValue {
                        key: "service.name".into(),
                        value: Some(AnyValue {
                            value: Some(Value::StringValue("test".into())),
                        }),
                        key_strindex: 0,
                    }],
                    dropped_attributes_count: 0,
                    entity_refs: vec![],
                }),
                scope_spans: vec![ScopeSpans {
                    scope: None,
                    spans: names
                        .iter()
                        .map(|n| Span {
                            name: (*n).into(),
                            ..Default::default()
                        })
                        .collect(),
                    schema_url: String::new(),
                }],
                schema_url: String::new(),
            }],
        }
    }

    fn span_with_name(name: &str) -> ExportTraceServiceRequest {
        export_with_span_names(&[name])
    }

    async fn client(endpoint: String) -> TraceServiceClient<tonic::transport::Channel> {
        TraceServiceClient::connect(endpoint).await.unwrap()
    }

    #[tokio::test]
    async fn records_one_received_span_per_proto_span() {
        let collector = MockOtlpCollector::spawn().await;
        let mut c = client(collector.endpoint()).await;
        // One Export with three spans must flatten into three buffered items.
        c.export(export_with_span_names(&["a", "b", "c"]))
            .await
            .unwrap();

        let got = collector.received();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].span.name, "a");
        assert_eq!(got[1].span.name, "b");
        assert_eq!(got[2].span.name, "c");
        // Resource and scope propagate to every flattened span.
        for received in &got {
            assert!(received.resource.is_some());
        }
    }

    #[tokio::test]
    async fn records_request_metadata_on_every_flattened_span() {
        let collector = MockOtlpCollector::spawn().await;
        let mut c = client(collector.endpoint()).await;

        let mut req = Request::new(export_with_span_names(&["x", "y"]));
        req.metadata_mut()
            .insert("x-test-tag", "abc-123".parse().unwrap());
        c.export(req).await.unwrap();

        let got = collector.received();
        assert_eq!(got.len(), 2);
        for received in &got {
            assert_eq!(
                received
                    .metadata
                    .get("x-test-tag")
                    .map(|v| v.to_str().unwrap()),
                Some("abc-123"),
            );
        }
    }

    #[tokio::test]
    async fn wait_for_pops_only_the_matched_span() {
        let collector = MockOtlpCollector::spawn().await;
        let mut c = client(collector.endpoint()).await;
        // Three spans, one batch — predicate-targeted pop must leave the
        // other two behind.
        c.export(export_with_span_names(&["a", "b", "c"]))
            .await
            .unwrap();

        let got = collector
            .wait_for(|s| s.span.name == "b", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(got.span.name, "b");

        let remaining = collector.received();
        let names: Vec<&str> = remaining.iter().map(|s| s.span.name.as_str()).collect();
        assert_eq!(names, vec!["a", "c"]);
    }

    #[tokio::test]
    async fn wait_for_returns_when_matching_span_arrives() {
        let collector = MockOtlpCollector::spawn().await;
        let endpoint = collector.endpoint();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let mut c = client(endpoint).await;
            c.export(span_with_name("delayed")).await.unwrap();
        });

        let start = std::time::Instant::now();
        let got = collector
            .wait_for(|s| s.span.name == "delayed", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(got.span.name, "delayed");
        // Sanity: notification-driven, so this should be well under the 5s budget.
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn wait_for_times_out_when_predicate_never_matches() {
        let collector = MockOtlpCollector::spawn().await;
        let err = collector
            .wait_for(|_| false, Duration::from_millis(150))
            .await
            .unwrap_err();
        assert!(err.snapshot.is_empty());
    }

    #[tokio::test]
    async fn wait_for_timeout_leaves_buffer_untouched() {
        let collector = MockOtlpCollector::spawn().await;
        let mut c = client(collector.endpoint()).await;
        c.export(span_with_name("kept")).await.unwrap();

        // No span named "missing" → predicate never matches → timeout.
        let err = collector
            .wait_for(|s| s.span.name == "missing", Duration::from_millis(150))
            .await
            .unwrap_err();
        // The diagnostic snapshot must surface what's in the buffer, and
        // the buffer itself must still hold the span — a follow-up call
        // can recover.
        assert_eq!(err.snapshot.len(), 1);
        assert_eq!(err.snapshot[0].span.name, "kept");
        assert_eq!(collector.received().len(), 1);
    }

    #[tokio::test]
    async fn assert_sink_empty_passes_when_empty() {
        let collector = MockOtlpCollector::spawn().await;
        crate::assert_sink_empty!(&collector);
    }

    #[tokio::test]
    #[should_panic(expected = "expected empty, got 1 request(s)")]
    async fn assert_sink_empty_panics_with_diagnostic() {
        let collector = MockOtlpCollector::spawn().await;
        let mut c = client(collector.endpoint()).await;
        c.export(span_with_name("unexpected")).await.unwrap();
        crate::assert_sink_empty!(&collector);
    }

    #[tokio::test]
    async fn is_empty_reflects_state() {
        let collector = MockOtlpCollector::spawn().await;
        assert!(collector.is_empty());

        let mut c = client(collector.endpoint()).await;
        c.export(span_with_name("first")).await.unwrap();
        assert!(!collector.is_empty());

        collector.clear();
        assert!(collector.is_empty());
    }

    #[tokio::test]
    async fn clear_resets_buffer() {
        let collector = MockOtlpCollector::spawn().await;
        let mut c = client(collector.endpoint()).await;
        c.export(span_with_name("first")).await.unwrap();
        assert_eq!(collector.received().len(), 1);
        collector.clear();
        assert!(collector.received().is_empty());
        c.export(span_with_name("second")).await.unwrap();
        assert_eq!(collector.received().len(), 1);
    }

    #[tokio::test]
    async fn shutdown_completes_cleanly() {
        let collector = MockOtlpCollector::spawn().await;
        let mut c = client(collector.endpoint()).await;
        c.export(span_with_name("first")).await.unwrap();
        collector.shutdown().await;
    }
}
