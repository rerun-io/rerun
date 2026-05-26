//! Bridges Python OpenTelemetry trace context into Rust tracing spans.
//!
//! The Python SDK and the Rust gRPC client use two separate tracing systems:
//! - **Python**: OpenTelemetry SDK (creates spans like `register`, `query`, etc.)
//! - **Rust**: the `tracing` crate, with `tracing-opentelemetry` bridging to OTel
//!
//! These don't automatically share context across the Python→Rust FFI boundary.
//! Without an explicit bridge, the Rust [`TracingInjectorInterceptor`] sees no
//! active span and won't inject `traceparent` headers into outgoing gRPC requests
//! — so the server starts a new, unlinked trace.
//!
//! The bridge works through a Python [`ContextVar`] as a shared mailbox:
//!
//! 1. Python calls [`get_trace_context_var`] to obtain the `ContextVar`.
//! 2. The `with_tracing` decorator serializes the current OTel span into
//!    W3C trace headers and writes them into the `ContextVar`.
//! 3. When Python calls an SDK method (e.g. `dataset.register()`), Rust calls
//!    [`read_trace_context_from_python`] which reads those headers back,
//!    attaches the OTel context, and creates a Rust `tracing::Span` parented to it.
//! 4. The [`TracingInjectorInterceptor`] picks up this span and injects
//!    `traceparent` into the outgoing gRPC request.
//!
//! [`TracingInjectorInterceptor`]: re_perf_telemetry::TracingInjectorInterceptor
//! [`ContextVar`]: https://docs.python.org/3/library/contextvars.html#contextvars.ContextVar

use pyo3::{Py, PyAny, PyResult, Python, pyfunction};

/// Read the trace context from the Python `ContextVar` and create a parented
/// Rust [`tracing::Span`].
///
/// This is the **read side** of the bridge — it consumes trace headers that
/// Python wrote via [`get_trace_context_var`]. Any gRPC calls made within this
/// span will automatically carry the correct `traceparent` header.
///
/// Must be called while the Python GIL is held. The returned span captures its
/// parent at creation time, so it can safely be passed to `.instrument(span)` or
/// `.entered()` after the GIL is released.
///
/// Returns [`tracing::Span::none`] when `perf_telemetry` is disabled.
#[must_use]
#[track_caller]
pub(crate) fn read_trace_context_from_python(
    #[allow(unused)] py: Python<'_>,
    #[allow(unused)] name: &'static str,
) -> tracing::Span {
    #[cfg(feature = "perf_telemetry")]
    {
        let trace_headers = extract_trace_context_from_contextvar(py);
        let _guard = trace_headers.attach();
        tracing::span!(tracing::Level::INFO, "sdk", otel.name = name)
    }

    #[cfg(not(feature = "perf_telemetry"))]
    tracing::Span::none()
}

/// Return the `ContextVar` that Python uses to pass trace headers to Rust.
///
/// This is the **write side** of the bridge — Python's `with_tracing` decorator
/// calls this to get the `ContextVar`, then writes W3C trace headers into it.
/// Rust later reads them back via [`read_trace_context_from_python`].
///
/// Returns `None` when `perf_telemetry` is disabled.
#[pyfunction]
#[pyo3(name = "_get_trace_context_var")]
pub fn get_trace_context_var(py: Python<'_>) -> PyResult<Py<PyAny>> {
    #[cfg(feature = "perf_telemetry")]
    {
        let context_var = trace_context_var(py)?;
        Ok(context_var.unbind())
    }
    #[cfg(not(feature = "perf_telemetry"))]
    {
        Ok(py.None())
    }
}

// ---
// Python `ContextVar` plumbing for trace-context propagation.
//
// All pyo3 use lives in this crate so `re_perf_telemetry` stays
// language-agnostic. The boundary between the two is a plain Rust
// `TraceHeaders` value: this side reads the `ContextVar` and hands the
// struct over; `re_perf_telemetry` consumes it without ever touching the
// Python runtime.

/// Name of the Python `ContextVar` used for trace-context propagation. The
/// Python decorator side (`rerun._tracing.with_tracing`) uses the same name
/// to set headers; Rust reads them back through this ContextVar.
#[cfg(feature = "perf_telemetry")]
const TRACE_CONTEXT_VAR_NAME: &str = "TRACE_CONTEXT";

/// Get the trace context `ContextVar` object.
///
/// This returns the same Python `ContextVar` instance every time, ensuring that
/// values set on it can be read back later. It is up to the caller to ensure trace context
/// is reset and cleared as needed.
#[cfg(feature = "perf_telemetry")]
fn trace_context_var(py: Python<'_>) -> PyResult<pyo3::Bound<'_, PyAny>> {
    use pyo3::prelude::*;

    static CONTEXT_VAR: parking_lot::Mutex<Option<Py<PyAny>>> = parking_lot::Mutex::new(None);

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

/// Extract trace context from the Python `ContextVar` for cross-boundary propagation.
///
/// Returns empty [`re_perf_telemetry::TraceHeaders`] if the `ContextVar` is unset or extraction fails.
#[cfg(feature = "perf_telemetry")]
pub(crate) fn extract_trace_context_from_contextvar(
    py: Python<'_>,
) -> re_perf_telemetry::TraceHeaders {
    use pyo3::prelude::*;
    use pyo3::types::PyDict;
    use re_perf_telemetry::TraceHeaders;

    fn try_extract(py: Python<'_>) -> PyResult<TraceHeaders> {
        let context_var = trace_context_var(py)?;

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
