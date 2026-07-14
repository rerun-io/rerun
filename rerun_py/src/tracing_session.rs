//! Python-side bridge for the `tracing_session()` context manager.
//!
//! See `rerun_py/rerun_sdk/rerun/_tracing_session.py` for the user-facing API.
//!
//! The bridge has two pyo3 entry points:
//!
//! - [`get_tracing_session_var`] — exposes the Python `ContextVar` whose current value is
//!   read by the Rust-side [`re_perf_telemetry::current_rerun_session_id`] on every
//!   outbound gRPC request. The `TraceStateEnricher` uses it to merge
//!   `rerun_session_id=<id>` into the `tracestate` header.
//!
//! - [`is_telemetry_active`] — lets the Python context manager fail fast with an
//!   actionable error when `TELEMETRY_ENABLED` is not truthy. Without an active
//!   telemetry stack, the `TracingInjectorInterceptor` has no valid OTel context to
//!   inject from, so a session id would never reach the wire.
//!
//! [`re_perf_telemetry::current_rerun_session_id`]: https://docs.rs/re_perf_telemetry

use pyo3::{Py, PyAny, PyResult, Python, pyfunction};

/// Return `True` if the rerun telemetry stack initialized successfully.
///
/// `tracing_session()` requires this to be true; otherwise the W3C propagator
/// is not registered and the session id has no transport.
#[pyfunction]
#[pyo3(name = "_is_telemetry_active")]
pub fn is_telemetry_active() -> bool {
    #[cfg(feature = "perf_telemetry")]
    {
        re_perf_telemetry::is_telemetry_active()
    }
    #[cfg(not(feature = "perf_telemetry"))]
    {
        false
    }
}

/// Return the `ContextVar` carrying the active rerun session id.
///
/// Set by the `tracing_session()` context manager and read on every outbound
/// gRPC call to merge `rerun_session_id=<id>` into the W3C `tracestate` header.
///
/// Returns `None` when `perf_telemetry` is disabled.
#[pyfunction]
#[pyo3(name = "_get_tracing_session_var")]
pub fn get_tracing_session_var(py: Python<'_>) -> PyResult<Py<PyAny>> {
    #[cfg(feature = "perf_telemetry")]
    {
        let context_var = get_rerun_session_var(py)?;
        Ok(context_var.unbind())
    }
    #[cfg(not(feature = "perf_telemetry"))]
    {
        Ok(py.None())
    }
}

// ---
// Python `ContextVar` plumbing for the active `tracing_session()` id.
//
// All pyo3 use lives in this crate so `re_perf_telemetry` stays
// language-agnostic. The boundary is the `SessionIdReader` closure
// registered at telemetry init (see `python_bridge.rs::init_perf_telemetry`):
// `re_perf_telemetry` invokes that closure to get an
// `Option<RerunTracingSessionId>` without knowing how it was sourced.

/// Name of the Python `ContextVar` carrying the active `tracing_session()` id.
#[cfg(feature = "perf_telemetry")]
const RERUN_SESSION_VAR_NAME: &str = "RERUN_SESSION_ID";

/// Get the rerun session id `ContextVar` object.
///
/// Set by the Python `tracing_session()` context manager. The Rust-side
/// [`re_perf_telemetry::current_rerun_session_id`] helper reads it on every
/// outbound RPC to enrich the W3C `tracestate` with `rerun_session_id=<id>`.
#[cfg(feature = "perf_telemetry")]
fn get_rerun_session_var(py: Python<'_>) -> PyResult<pyo3::Bound<'_, PyAny>> {
    use pyo3::prelude::*;

    static CONTEXT_VAR: parking_lot::Mutex<Option<Py<PyAny>>> = parking_lot::Mutex::new(None);

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
/// the value fails [`re_perf_telemetry::RerunTracingSessionId::parse`].
#[cfg(feature = "perf_telemetry")]
pub(crate) fn current_rerun_session_id_from_contextvar(
    py: Python<'_>,
) -> Option<re_perf_telemetry::RerunTracingSessionId> {
    use pyo3::prelude::*;

    let var = get_rerun_session_var(py).ok()?;
    let value = var.call_method0("get").ok()?;
    if value.is_none() {
        return None;
    }
    let raw = value.extract::<String>().ok()?;
    re_perf_telemetry::RerunTracingSessionId::parse(&raw)
}

/// Increment the process-wide active-tracing-session gate. Called by `tracing_session().__enter__`.
#[pyfunction]
#[pyo3(name = "_inc_active_tracing_sessions")]
pub fn inc_active_tracing_sessions() {
    #[cfg(feature = "perf_telemetry")]
    {
        re_perf_telemetry::inc_active_tracing_session_count();
    }
}

/// Decrement the process-wide active-tracing-session gate. Called by `tracing_session().__exit__`.
#[pyfunction]
#[pyo3(name = "_dec_active_tracing_sessions")]
pub fn dec_active_tracing_sessions() {
    #[cfg(feature = "perf_telemetry")]
    {
        re_perf_telemetry::dec_active_tracing_session_count();
    }
}

/// Emit `rerun tracing session started: <rerun_session_id>` through the Rust `tracing` stack at INFO level.
#[pyfunction]
#[pyo3(name = "_log_tracing_session_started")]
pub fn log_tracing_session_started(rerun_session_id: &str) {
    tracing::info!("rerun tracing session started: {rerun_session_id}");
}

/// Emit a single structured INFO event summarizing the tracing session at scope exit.
///
/// `Option<f64>` fields are `None` when the host platform or runtime can't supply
/// the metric (psutil missing, or `iowait` unavailable on macOS/Windows). Routed
/// through the Rust `tracing` stack so it follows `RUST_LOG` and the fmt-layer
/// pipeline like `_log_tracing_session_started`.
#[pyfunction]
#[pyo3(name = "_log_tracing_session_finished")]
#[pyo3(signature = (rerun_session_id, elapsed_s, cpu_user_s, cpu_system_s, cpu_iowait_s, net_rx_mb))]
pub fn log_tracing_session_finished(
    rerun_session_id: &str,
    elapsed_s: f64,
    cpu_user_s: Option<f64>,
    cpu_system_s: Option<f64>,
    cpu_iowait_s: Option<f64>,
    net_rx_mb: Option<f64>,
) {
    let fmt = |v: Option<f64>| v.map_or_else(|| "na".to_owned(), |x| format!("{x:.3}"));
    tracing::info!(
        rerun_session_id,
        elapsed_s = format!("{elapsed_s:.3}"),
        cpu_user_s = fmt(cpu_user_s),
        cpu_system_s = fmt(cpu_system_s),
        cpu_iowait_s = fmt(cpu_iowait_s),
        net_rx_mb = fmt(net_rx_mb),
        "rerun tracing session finished",
    );
}
