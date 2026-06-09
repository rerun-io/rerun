//! In-memory HTTP sink that mimics the `PostHog` capture endpoint.
//!
//! Spawns an `axum` server on a random `127.0.0.1` port. Every received POST
//! is parsed and its `/batch` array is flattened into one [`ReceivedEvent`]
//! per entry, recorded with the request headers. Mirrors
//! [`super::otlp::MockOtlpCollector`] in shape (same `spawn` / `received` /
//! `wait_for` / `is_empty` / `clear` / `shutdown` surface) so tests read
//! consistently.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use parking_lot::Mutex;
use tokio::sync::{Notify, oneshot};

/// A single `PostHog` capture event observed by the sink, flattened out
/// of the wire-format `{ "batch": […] }` envelope.
///
/// One incoming `POST` carrying N events in its `/batch` array produces N
/// [`ReceivedEvent`]s, each with the same `headers` clone but its own
/// `event` JSON. This is the granularity tests reason about: a `wait_for`
/// predicate looks at one event at a time, and pops exactly that one when
/// it matches — leaving any siblings from the same batch in the buffer
/// for follow-up matches.
///
/// If the request body is unparsable JSON, or parses but doesn't carry
/// a `/batch` array, the request lands as a single [`ReceivedEvent`] with
/// [`EventBody::BadRequest`] and the handler returns `400`.
#[derive(Clone, Debug)]
pub struct ReceivedEvent {
    pub headers: HeaderMap,
    pub event: EventBody,
}

/// One element from a parsed `/batch` array, or the diagnostic for a
/// request the handler rejected.
#[derive(Clone, Debug)]
pub enum EventBody {
    Ok(serde_json::Value),
    BadRequest { raw: Vec<u8>, error: String },
}

impl EventBody {
    /// `Some(&value)` if the event was a parsed `/batch` entry; `None`
    /// otherwise.
    ///
    /// Use this in `wait_for` predicates and other Option-shaped contexts.
    /// For direct test assertions, prefer [`Self::expect_parsed`].
    pub fn as_ok(&self) -> Option<&serde_json::Value> {
        if let Self::Ok(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Returns the parsed event JSON, panicking with the raw body and
    /// error if the request had been rejected. Use this in direct test
    /// assertions where a bad request is unambiguously a test failure.
    pub fn expect_parsed(&self) -> &serde_json::Value {
        match self {
            Self::Ok(v) => v,
            Self::BadRequest { raw, error } => panic!(
                "expected a parsed PostHog batch entry, got BadRequest:\n  error: {error}\n  raw ({} bytes): {}",
                raw.len(),
                String::from_utf8_lossy(raw),
            ),
        }
    }
}

#[derive(Default)]
struct Inner {
    received: Mutex<Vec<ReceivedEvent>>,
    notify: Notify,
}

// IMPORTANT: this handler records every event flattened out of the request
// *before* returning its response, so by the time a client's `.send().await`
// returns Ok, every [`ReceivedEvent`] for that request is already in the
// buffer. Tests can assert on `received()` immediately after the client
// `.await` — no `wait_for` needed.
async fn handler(
    State(inner): State<Arc<Inner>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> StatusCode {
    let parsed = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(parsed) => parsed,
        Err(err) => {
            inner.received.lock().push(ReceivedEvent {
                headers,
                event: EventBody::BadRequest {
                    raw: body.to_vec(),
                    error: err.to_string(),
                },
            });
            inner.notify.notify_waiters();
            return StatusCode::BAD_REQUEST;
        }
    };

    let Some(batch) = parsed.pointer("/batch").and_then(|v| v.as_array()) else {
        // Parses as JSON but isn't the PostHog `{ "batch": […] }`
        // envelope — record once as BadRequest so tests can diagnose, and
        // surface the protocol violation through the HTTP status.
        inner.received.lock().push(ReceivedEvent {
            headers,
            event: EventBody::BadRequest {
                raw: body.to_vec(),
                error: "missing or non-array `/batch` field".to_owned(),
            },
        });
        inner.notify.notify_waiters();
        return StatusCode::BAD_REQUEST;
    };

    {
        // Flatten under a single lock so observers never see a partially-
        // applied request; notify once at the end.
        let mut buffer = inner.received.lock();
        for entry in batch {
            buffer.push(ReceivedEvent {
                headers: headers.clone(),
                event: EventBody::Ok(entry.clone()),
            });
        }
    }
    inner.notify.notify_waiters();
    StatusCode::OK
}

/// In-process HTTP server that flattens each POST's `/batch` array into
/// one [`ReceivedEvent`] per entry.
///
/// Only the root path (`/`) and POST requests are routed to the handler;
/// other methods or paths get the axum default (405 / 404) and are not
/// recorded. Drop is fire-and-forget; use [`Self::shutdown`] for graceful
/// teardown that awaits the server task.
pub struct MockPostHog {
    addr: SocketAddr,
    inner: Arc<Inner>,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
}

/// Returned by [`MockPostHog::wait_for`] when no buffered event matched
/// within the timeout.
///
/// `snapshot` is a clone of whatever events were in the buffer at the
/// timeout instant — the buffer itself is left untouched, so a follow-up
/// `wait_for` call can resume waiting against the same buffer.
#[derive(Debug)]
pub struct PosthogWaitTimeout {
    pub snapshot: Vec<ReceivedEvent>,
}

impl std::fmt::Display for PosthogWaitTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "timed out waiting for matching event; buffer holds {} event(s)",
            self.snapshot.len()
        )
    }
}

impl std::error::Error for PosthogWaitTimeout {}

impl MockPostHog {
    /// Bind to an OS-assigned port on `127.0.0.1` and start serving.
    ///
    /// Returns only after the server task has begun executing, so subsequent
    /// requests are not racing the spawned task's first poll.
    pub async fn spawn() -> Self {
        let inner = Arc::new(Inner::default());
        let app = Router::new()
            .route("/", post(handler))
            .with_state(inner.clone());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind 127.0.0.1:0");
        let addr = listener.local_addr().expect("local_addr");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let join = tokio::spawn(async move {
            // Signal that the task has begun executing; the very next thing
            // we do is await the serve future, which begins polling the
            // listener. By the time `spawn()` returns to the caller, axum is
            // about to (or already is) accepting from the kernel queue.
            _ = ready_tx.send(());
            drop(
                axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        drop(shutdown_rx.await);
                    })
                    .await,
            );
        });
        _ = ready_rx.await;

        Self {
            addr,
            inner,
            shutdown: Some(shutdown_tx),
            join: Some(join),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Convenience: `http://127.0.0.1:PORT`, suitable for `PostHogClient::with_url`.
    pub fn endpoint(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Snapshot of all buffered events (cloned out, buffer untouched).
    pub fn received(&self) -> Vec<ReceivedEvent> {
        self.inner.received.lock().clone()
    }

    /// `true` if no events are currently buffered (initial state, fully
    /// drained, or [`Self::clear`]'d).
    pub fn is_empty(&self) -> bool {
        self.inner.received.lock().is_empty()
    }

    pub fn clear(&self) {
        self.inner.received.lock().clear();
    }

    /// Wait for, and consume, the next buffered event that satisfies
    /// `predicate`. On success the matched event is `remove`d from the
    /// buffer in arrival order and returned; siblings from the same
    /// `/batch` stay in place. On timeout the buffer is left untouched
    /// and the failure surfaces the current buffer contents through
    /// [`PosthogWaitTimeout::snapshot`].
    ///
    /// Notification-driven: returns ~immediately once a matching event
    /// is in the buffer. If multiple buffered events match, the earliest
    /// one wins; subsequent calls can pop the next match.
    ///
    /// The pop is the consumption point: anything left after a sequence
    /// of `wait_for` calls is genuinely surplus, so closing a test with
    /// [`crate::assert_sink_empty!`] is a meaningful check.
    pub async fn wait_for<F>(
        &self,
        predicate: F,
        timeout: Duration,
    ) -> Result<ReceivedEvent, PosthogWaitTimeout>
    where
        F: Fn(&ReceivedEvent) -> bool,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            // Arm the waiter *before* the buffer scan so a push racing
            // between scan and await cannot be missed.
            let notified = self.inner.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            {
                let mut buffer = self.inner.received.lock();
                if let Some(pos) = buffer.iter().position(&predicate) {
                    return Ok(buffer.remove(pos));
                }
            }

            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return Err(PosthogWaitTimeout {
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

impl Drop for MockPostHog {
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

    async fn post_json(url: &str, json: serde_json::Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&json).unwrap())
            .send()
            .await
            .unwrap()
    }

    /// Helper: wrap one or more event objects in the `PostHog`
    /// `{ "batch": […] }` envelope that the handler expects.
    fn batch(events: &[serde_json::Value]) -> serde_json::Value {
        serde_json::json!({"api_key": "k", "batch": events})
    }

    #[tokio::test]
    async fn records_one_event_per_batch_entry() {
        let collector = MockPostHog::spawn().await;
        let resp = post_json(
            &collector.endpoint(),
            batch(&[
                serde_json::json!({"event": "a"}),
                serde_json::json!({"event": "b"}),
                serde_json::json!({"event": "c"}),
            ]),
        )
        .await;
        assert!(resp.status().is_success());

        let got = collector.received();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].event.expect_parsed()["event"], "a");
        assert_eq!(got[1].event.expect_parsed()["event"], "b");
        assert_eq!(got[2].event.expect_parsed()["event"], "c");
    }

    #[tokio::test]
    async fn records_request_headers_on_every_flattened_event() {
        let collector = MockPostHog::spawn().await;
        let resp = reqwest::Client::new()
            .post(collector.endpoint())
            .header("Content-Type", "application/json")
            .header("X-Custom-Header", "value-123")
            .body(
                serde_json::to_string(&batch(&[
                    serde_json::json!({"event": "a"}),
                    serde_json::json!({"event": "b"}),
                ]))
                .unwrap(),
            )
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());

        let got = collector.received();
        assert_eq!(got.len(), 2);
        for received in &got {
            assert_eq!(
                received
                    .headers
                    .get("x-custom-header")
                    .and_then(|v| v.to_str().ok()),
                Some("value-123"),
            );
        }
    }

    #[tokio::test]
    async fn records_malformed_json_as_bad_request() {
        let collector = MockPostHog::spawn().await;
        let resp = reqwest::Client::new()
            .post(collector.endpoint())
            .header("Content-Type", "application/json")
            .body("not-json")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);

        let got = collector.received();
        assert_eq!(got.len(), 1);
        match &got[0].event {
            EventBody::Ok(v) => panic!("expected BadRequest, got Ok({v:?})"),
            EventBody::BadRequest { raw, error } => {
                assert_eq!(raw, b"not-json");
                assert!(!error.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn records_missing_batch_as_bad_request() {
        let collector = MockPostHog::spawn().await;
        // Parseable JSON but no `/batch` envelope — must surface as a
        // protocol violation, not silently accepted.
        let resp = post_json(&collector.endpoint(), serde_json::json!({"x": 1})).await;
        assert_eq!(resp.status(), 400);

        let got = collector.received();
        assert_eq!(got.len(), 1);
        match &got[0].event {
            EventBody::Ok(v) => panic!("expected BadRequest, got Ok({v:?})"),
            EventBody::BadRequest { error, .. } => {
                assert!(
                    error.contains("/batch"),
                    "error should mention /batch: {error}"
                );
            }
        }
    }

    #[tokio::test]
    async fn rejects_non_post_methods() {
        let collector = MockPostHog::spawn().await;
        let resp = reqwest::Client::new()
            .get(collector.endpoint())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 405);
        assert!(collector.is_empty());
    }

    #[tokio::test]
    async fn wait_for_pops_only_the_matched_event() {
        let collector = MockPostHog::spawn().await;
        post_json(
            &collector.endpoint(),
            batch(&[
                serde_json::json!({"event": "a"}),
                serde_json::json!({"event": "b"}),
                serde_json::json!({"event": "c"}),
            ]),
        )
        .await;

        let got = collector
            .wait_for(
                |e| e.event.as_ok().map(|v| v["event"] == "b").unwrap_or(false),
                Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!(got.event.expect_parsed()["event"], "b");

        let remaining = collector.received();
        let names: Vec<&str> = remaining
            .iter()
            .filter_map(|e| e.event.as_ok())
            .filter_map(|v| v["event"].as_str())
            .collect();
        assert_eq!(names, vec!["a", "c"]);
    }

    #[tokio::test]
    async fn wait_for_returns_when_matching_event_arrives() {
        let collector = MockPostHog::spawn().await;
        let endpoint = collector.endpoint();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            post_json(&endpoint, batch(&[serde_json::json!({"event": "delayed"})])).await;
        });

        let start = std::time::Instant::now();
        let got = collector
            .wait_for(
                |e| {
                    e.event
                        .as_ok()
                        .map(|v| v["event"] == "delayed")
                        .unwrap_or(false)
                },
                Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!(got.event.expect_parsed()["event"], "delayed");
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn wait_for_times_out_when_predicate_never_matches() {
        let collector = MockPostHog::spawn().await;
        let err = collector
            .wait_for(|_| false, Duration::from_millis(150))
            .await
            .unwrap_err();
        assert!(err.snapshot.is_empty());
    }

    #[tokio::test]
    async fn clear_resets_buffer() {
        let collector = MockPostHog::spawn().await;
        post_json(
            &collector.endpoint(),
            batch(&[serde_json::json!({"event": "1"})]),
        )
        .await;
        assert_eq!(collector.received().len(), 1);
        collector.clear();
        assert!(collector.received().is_empty());
        post_json(
            &collector.endpoint(),
            batch(&[serde_json::json!({"event": "2"})]),
        )
        .await;
        assert_eq!(collector.received().len(), 1);
    }

    #[tokio::test]
    async fn is_empty_reflects_state() {
        let collector = MockPostHog::spawn().await;
        assert!(collector.is_empty());

        post_json(
            &collector.endpoint(),
            batch(&[serde_json::json!({"event": "x"})]),
        )
        .await;
        assert!(!collector.is_empty());

        collector.clear();
        assert!(collector.is_empty());
    }

    #[tokio::test]
    async fn assert_sink_empty_passes_when_empty() {
        let collector = MockPostHog::spawn().await;
        crate::assert_sink_empty!(&collector);
    }

    #[tokio::test]
    #[should_panic(expected = "expected empty, got 1 request(s)")]
    async fn assert_sink_empty_panics_with_diagnostic() {
        let collector = MockPostHog::spawn().await;
        // Handler records before responding, so the recording is visible by
        // the time `post_json` returns. No `wait_for` needed.
        post_json(
            &collector.endpoint(),
            batch(&[serde_json::json!({"event": "unexpected"})]),
        )
        .await;
        crate::assert_sink_empty!(&collector);
    }

    #[tokio::test]
    #[should_panic(expected = "expected a parsed PostHog batch entry, got BadRequest")]
    async fn expect_parsed_panics_on_bad_request() {
        let collector = MockPostHog::spawn().await;
        reqwest::Client::new()
            .post(collector.endpoint())
            .body("not-json")
            .send()
            .await
            .unwrap();
        let got = collector.received();
        assert_eq!(got.len(), 1);
        let _ = got[0].event.expect_parsed();
    }

    #[tokio::test]
    async fn shutdown_completes_cleanly() {
        let collector = MockPostHog::spawn().await;
        post_json(&collector.endpoint(), serde_json::json!({"x": 1})).await;
        collector.shutdown().await;
    }
}
