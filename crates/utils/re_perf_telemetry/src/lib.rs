//! Everything needed to set up telemetry (logs, traces, metrics) for both clients and servers.
//!
//! Despite the name `re_perf_telemetry`, this actually handles _all_ forms of telemetry,
//! including all log output.
//!
//! This sort of telemetry is always disabled on our OSS binaries, and is only used for
//! * The Rerun Cloud infrastructure
//! * Profiling by Rerun developer
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
mod memory_telemetry;
mod metrics_server;
mod prometheus;
mod shared_reader;
mod telemetry;
mod tracestate;
mod utils;

use std::collections::HashMap;

use opentelemetry_sdk::propagation::TraceContextPropagator;

pub use self::args::{LogFormat, TelemetryArgs};
pub use self::grpc::{
    ClientTelemetryLayer, GrpcMakeSpan, GrpcOnEos, GrpcOnFirstBodyChunk, GrpcOnRequest,
    GrpcOnResponse, GrpcOnResponseOptions, ServerTelemetryLayer, TelemetryLayerOptions,
    TraceIdLayer, TracingInjectorInterceptor, new_client_telemetry_layer,
    new_server_telemetry_layer,
};
pub use self::telemetry::{Telemetry, TelemetryDropBehavior};
pub use self::utils::to_short_str;

pub mod external {
    #[cfg(feature = "tracy")]
    pub use tracing_tracy;
    pub use {clap, opentelemetry, tower, tower_http, tracing, tracing_opentelemetry};
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

    pub fn tracestate(&self) -> HashMap<String, String> {
        self.tracestate
            .as_ref()
            .map(|s| crate::tracestate::parse_pairs(s))
            .unwrap_or_default()
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

// ---

/// The name of the `ContextVar` used for trace context propagation
pub const TRACE_CONTEXT_VAR_NAME: &str = "TRACE_CONTEXT";

#[cfg(feature = "pyo3")]
/// Get the trace context `ContextVar` object.
///
/// This returns the same Python `ContextVar` instance every time, ensuring that
/// values set on it can be read back later. It is up to the caller to ensure trace context
/// is reset and cleared as needed.
pub fn get_trace_context_var(py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::Bound<'_, pyo3::PyAny>> {
    use pyo3::prelude::*;

    static CONTEXT_VAR: parking_lot::Mutex<Option<pyo3::Py<pyo3::PyAny>>> =
        parking_lot::Mutex::new(None);

    let mut guard = CONTEXT_VAR.lock();

    if let Some(var) = guard.as_ref() {
        return Ok(var.bind(py).clone());
    }

    // Create the trace context ContextVar
    let module = py.import("contextvars")?;
    let contextvar_class = module.getattr("ContextVar")?;
    let trace_ctx_var = contextvar_class.call1((TRACE_CONTEXT_VAR_NAME,))?;
    let trace_ctx_unbound = trace_ctx_var.clone().unbind();

    *guard = Some(trace_ctx_unbound);

    Ok(trace_ctx_var)
}

#[cfg(feature = "pyo3")]
/// Extract trace context from Python `ContextVar` for cross-boundary propagation.
pub fn extract_trace_context_from_contextvar(py: pyo3::Python<'_>) -> TraceHeaders {
    use pyo3::prelude::*;
    use pyo3::types::PyDict;

    fn try_extract(py: pyo3::Python<'_>) -> PyResult<TraceHeaders> {
        let context_var = get_trace_context_var(py)?;

        match context_var.call_method0("get") {
            Ok(trace_data) => {
                if let Ok(dict) = trace_data.downcast::<PyDict>() {
                    let traceparent = dict
                        .get_item(TraceHeaders::TRACEPARENT_KEY)?
                        .and_then(|v| v.extract::<String>().ok())
                        .unwrap_or_default();

                    let tracestate = dict
                        .get_item(TraceHeaders::TRACESTATE_KEY)?
                        .and_then(|v| v.extract::<String>().ok());

                    let headers = TraceHeaders {
                        traceparent,
                        tracestate,
                    };

                    tracing::debug!("Trace headers: {:?}", headers);
                    Ok(headers)
                } else {
                    Ok(TraceHeaders::empty())
                }
            }
            Err(_) => Ok(TraceHeaders::empty()),
        }
    }

    try_extract(py).unwrap_or_else(|err| {
        tracing::debug!("Failed to extract trace context: {err}");
        TraceHeaders::empty()
    })
}

// ---

// Extension to [`tracing_subscriber:EnvFilter`] that allows to
// add a directive only if not already present in the base filter
pub trait EnvFilterExt
where
    Self: Sized,
{
    fn add_directive_if_absent(
        self,
        base: &str,
        target: &str,
        default: &str,
    ) -> anyhow::Result<Self>;
}

impl EnvFilterExt for tracing_subscriber::EnvFilter {
    fn add_directive_if_absent(
        self,
        base: &str,
        target: &str,
        default: &str,
    ) -> anyhow::Result<Self> {
        if !base.contains(&format!("{target}=")) {
            let filter = self.add_directive(format!("{target}={default}").parse()?);
            Ok(filter)
        } else {
            Ok(self)
        }
    }
}
