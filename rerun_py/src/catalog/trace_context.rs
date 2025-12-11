use pyo3::{Py, PyAny, PyResult, Python, pyfunction};

#[cfg(feature = "perf_telemetry")]
/// Create a tracing span with optional distributed tracing context propagation
macro_rules! with_trace_span {
    ($py:expr, $span_name:expr, $body:block) => {{
        let trace_headers = extract_trace_context_from_contextvar($py);
        if !trace_headers.traceparent.is_empty() {
            let parent_ctx =
                re_perf_telemetry::external::opentelemetry::global::get_text_map_propagator(
                    |prop| prop.extract(&trace_headers),
                );
            let _guard = parent_ctx.attach();
            let _span = tracing::span!(tracing::Level::INFO, $span_name).entered();
            $body
        } else {
            let _span = tracing::span!(tracing::Level::INFO, $span_name).entered();
            $body
        }
    }};
}

#[cfg(feature = "perf_telemetry")]
pub(crate) use with_trace_span;

/// Get the trace context ContextVar for distributed tracing propagation.
#[pyfunction]
pub fn rerun_trace_context(py: Python<'_>) -> PyResult<Py<PyAny>> {
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
