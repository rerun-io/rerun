use pyo3::{Py, PyAny, PyResult, Python, pyfunction};

/// Get the trace context ContextVar for distributed tracing propagation.
#[pyfunction]
pub fn _rerun_trace_context(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let context_var = re_perf_telemetry::get_trace_context_var(py)?;
    Ok(context_var.unbind())
}
