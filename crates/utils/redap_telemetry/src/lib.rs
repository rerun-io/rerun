//! Everything needed to set up telemetry (logs, traces, metrics) for both clients and servers.
//!
//! Logging strategy
//! ================
//!
//! * All our logs go through the structured `tracing` macros.
//!
//! * We always log from `tracing` directly into stdio: we never involve the `OpenTelemetry`
//!   logging API. Production is expected to read the logs from the pod's output.
//!   There is never any internal buffering going on, besides the buffering of stdio itself.
//!
//! * All logs that happen as part of the larger trace/span will automatically be uploaded
//!   with that trace/span.
//!   This makes our traces a very powerful debugging tool, in addition to a profiler.
//!
//! Tracing strategy
//! ================
//!
//! * All our traces go through the structured `tracing` macros. We *never* use the
//!   `OpenTelemetry` macros.
//!
//! * The traces go through a first layer of filtering based on the value of `RUST_TRACE`, which
//!   functions similarly to a `RUST_LOG` filter.
//!
//! * The traces are then sent to the `OpenTelemetry` SDK, where they will go through a pass of
//!   sampling before being sent to the OTLP endpoint.
//!   The sampling mechanism is controlled by the official OTEL environment variables.
//!   span sampling decision.
//!
//! * Spans that contains error logs will properly be marked as failed, and easily findable.
//!
//! Metric strategy
//! ===============
//!
//! * Our metric strategy is basically the opposite of our logging strategy: everything goes
//!   through `OpenTelemetry` directly, `tracing` is never involved.
//!
//! * Metrics are uploaded (as opposed to scrapped!) using the OTLP protocol, on a fixed interval
//!   defined by the `OTEL_METRIC_EXPORT_INTERVAL` environment variable.

mod args;
mod grpc;
mod telemetry;

use opentelemetry_sdk::propagation::TraceContextPropagator;

pub use self::{
    args::{LogFormat, TelemetryArgs},
    grpc::{
        GrpcSpanMaker, TracingExtractorInterceptor, TracingInjectorInterceptor,
        new_grpc_tracing_layer,
    },
    telemetry::{Telemetry, TelemetryDropBehavior},
};

pub mod external {
    pub use opentelemetry;
    pub use tower;
    pub use tower_http;
    pub use tracing;
    pub use tracing_opentelemetry;
}

// ---

/// Returns the active [`TraceId`] in the current context, if any.
///
/// The returned trace ID can be search for in the distributed tracing backend, e.g. in jaeger:
/// ```text
/// http://localhost:16686/trace/{trace_id}
/// ```
///
/// Returns `None` if there is no trace *actively being sampled* in the current context.
///
/// [`TraceId`]: [opentelemetry::TraceId]
pub fn current_trace_id() -> Option<opentelemetry::TraceId> {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    let cx = tracing::Span::current().context();
    let span = cx.span();
    let span_cx = span.span_context();

    (span_cx.is_valid() && span_cx.is_sampled()).then(|| span_cx.trace_id())
}

/// Export the active trace in the current context as the W3C trace headers, if any.
///
/// Returns `None` if there is no trace *actively being sampled* in the current context.
pub fn current_trace_headers() -> Option<TraceHeaders> {
    use opentelemetry::propagation::text_map_propagator::TextMapPropagator as _;
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    let cx = tracing::Span::current().context();
    let span = cx.span();
    let span_cx = span.span_context();

    if !span_cx.is_valid() || !span_cx.is_sampled() {
        return None;
    }

    let propagator = TraceContextPropagator::new();
    let mut carrier = TraceHeaders::empty();

    propagator.inject_context(&cx, &mut carrier);

    Some(carrier)
}

#[derive(Debug, Clone)]
pub struct TraceHeaders {
    pub traceparent: String,
    pub tracestate: Option<String>,
}

impl TraceHeaders {
    pub const TRACEPARENT_KEY: &'static str = "traceparent";
    pub const TRACESTATE_KEY: &'static str = "tracestate";

    fn empty() -> Self {
        Self {
            traceparent: String::new(),
            tracestate: None,
        }
    }
}

impl opentelemetry::propagation::Injector for TraceHeaders {
    fn set(&mut self, key: &str, value: String) {
        match key {
            Self::TRACEPARENT_KEY => self.traceparent = value,
            Self::TRACESTATE_KEY => {
                if !value.is_empty() {
                    self.tracestate = Some(value);
                }
            }
            _ => {}
        }
    }
}

impl opentelemetry::propagation::Extractor for TraceHeaders {
    fn get(&self, key: &str) -> Option<&str> {
        match key {
            Self::TRACEPARENT_KEY => Some(self.traceparent.as_str()),
            Self::TRACESTATE_KEY => self.tracestate.as_deref(),
            _ => None,
        }
    }

    fn keys(&self) -> Vec<&str> {
        vec![Self::TRACEPARENT_KEY, Self::TRACESTATE_KEY]
    }
}

impl From<&TraceHeaders> for opentelemetry::Context {
    fn from(value: &TraceHeaders) -> Self {
        use opentelemetry::propagation::text_map_propagator::TextMapPropagator as _;
        let propagator = TraceContextPropagator::new();
        propagator.extract(value)
    }
}
