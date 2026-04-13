//! Python↔Rust trace context bridge via a shared [`ContextVar`].
//!
//! See `rerun_py/src/catalog/trace_context.rs` for the full bridge documentation
//! and the Rust-side entry points that call into these helpers.
//!
//! [`ContextVar`]: https://docs.python.org/3/library/contextvars.html#contextvars.ContextVar

use crate::TraceHeaders;

/// The name of the Python `ContextVar` used for trace context propagation.
pub const TRACE_CONTEXT_VAR_NAME: &str = "TRACE_CONTEXT";

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
