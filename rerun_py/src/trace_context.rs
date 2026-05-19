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
        let trace_headers = re_perf_telemetry::extract_trace_context_from_contextvar(py);
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
        let context_var = re_perf_telemetry::get_trace_context_var(py)?;
        Ok(context_var.unbind())
    }
    #[cfg(not(feature = "perf_telemetry"))]
    {
        Ok(py.None())
    }
}
