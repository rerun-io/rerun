//! Python↔Rust trace context bridge via a shared [`ContextVar`].
//!
//! See `rerun_py/src/catalog/trace_context.rs` for the full bridge documentation
//! and the Rust-side entry points that call into these helpers.
//!
//! [`ContextVar`]: https://docs.python.org/3/library/contextvars.html#contextvars.ContextVar

use crate::TraceHeaders;

/// The name of the Python `ContextVar` used for trace context propagation.
pub const TRACE_CONTEXT_VAR_NAME: &str = "TRACE_CONTEXT";

/// The name of the Python `ContextVar` carrying the active [`tracing_session`] id.
///
/// [`tracing_session`]: https://ref.rerun.io/docs/python
pub const RERUN_SESSION_VAR_NAME: &str = "RERUN_SESSION_ID";

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

/// Extract trace context from the Python `ContextVar` for cross-boundary propagation.
///
/// Returns empty [`TraceHeaders`] if the `ContextVar` is unset or extraction fails.
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

/// Get the rerun session id `ContextVar` object.
///
/// Set by the Python `tracing_session()` context manager. The Rust-side
/// [`crate::current_rerun_session_id`] helper reads it on every outbound RPC
/// to enrich the W3C `tracestate` with `rerun_session_id=<id>`.
pub fn get_rerun_session_var(py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::Bound<'_, pyo3::PyAny>> {
    use pyo3::prelude::*;

    static CONTEXT_VAR: parking_lot::Mutex<Option<pyo3::Py<pyo3::PyAny>>> =
        parking_lot::Mutex::new(None);

    let mut guard = CONTEXT_VAR.lock();

    if let Some(var) = guard.as_ref() {
        return Ok(var.bind(py).clone());
    }

    let module = py.import("contextvars")?;
    let contextvar_class = module.getattr("ContextVar")?;
    // Default to an explicit `None` so `.get()` never raises `LookupError`.
    let kwargs = pyo3::types::PyDict::new(py);
    kwargs.set_item("default", py.None())?;
    let var = contextvar_class.call((RERUN_SESSION_VAR_NAME,), Some(&kwargs))?;
    *guard = Some(var.clone().unbind());

    Ok(var)
}

/// Read the current rerun session id from the Python `ContextVar`.
///
/// Returns `None` when no `tracing_session()` is active, the value is unset, or
/// the value fails [`crate::RerunTracingSessionId::parse`].
pub fn current_rerun_session_id_from_contextvar(
    py: pyo3::Python<'_>,
) -> Option<crate::RerunTracingSessionId> {
    use pyo3::prelude::*;

    let var = get_rerun_session_var(py).ok()?;
    let value = var.call_method0("get").ok()?;
    if value.is_none() {
        return None;
    }
    let raw = value.extract::<String>().ok()?;
    crate::RerunTracingSessionId::parse(&raw)
}
