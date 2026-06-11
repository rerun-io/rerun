//! PyO3 bridge for `rerun.experimental.query_metrics()`.
//!
//! See [`rerun_py/rerun_sdk/rerun/experimental/_query_metrics.py`] for the
//! user-facing context manager. This file exposes:
//!
//! - [`PyQueryMetrics`]: a frozen, getter-only Python class mirroring
//!   [`re_datafusion::QuerySnapshot`].
//! - [`PyMetricsCollectorHandle`]: opaque wrapper around
//!   [`re_datafusion::MetricsCollector`] with `drain()` / `snapshot()` methods.
//! - [`new_metrics_collector`]: allocate a fresh handle. The Python wrapper
//!   pushes it onto the `_active_collectors` `ContextVar` for the duration of
//!   the `with` block.
//! - [`active_metrics_collectors`]: read that ContextVar from Rust so
//!   `dataset_view.rs::reader()` can attach the collectors to a freshly-built
//!   `DataframeQueryTableProvider`.
//!
//! The actual snapshot-on-stream-completion logic lives in
//! `re_datafusion::metrics_capture` and `re_datafusion::dataframe_query_provider`.

use std::time::Duration;

use pyo3::prelude::*;
use pyo3::types::PyTuple;
use re_datafusion::{MetricsCollector, QuerySnapshot};

/// Frozen, getter-only mirror of [`re_datafusion::QuerySnapshot`].
///
/// One per query that ran inside a `query_metrics()` scope.
#[pyclass(
    frozen,
    from_py_object,
    eq,
    name = "_QueryMetrics",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyQueryMetrics {
    snap: QuerySnapshot,
}

impl PyQueryMetrics {
    fn new(snap: QuerySnapshot) -> Self {
        Self { snap }
    }
}

#[pymethods]
impl PyQueryMetrics {
    // ---- Plan-time fields ----

    /// The dataset being queried.
    #[getter]
    fn dataset_id(&self) -> &str {
        &self.snap.query_info.dataset_id
    }

    /// Number of unique chunks returned by `query_dataset` (subset of the dataset).
    #[getter]
    fn query_chunks(&self) -> usize {
        self.snap.query_info.query_chunks
    }

    /// Number of distinct segments involved in the query.
    #[getter]
    fn query_segments(&self) -> usize {
        self.snap.query_info.query_segments
    }

    /// Number of distinct layers touched by the query.
    #[getter]
    fn query_layers(&self) -> usize {
        self.snap.query_info.query_layers
    }

    /// Number of columns in the query output schema.
    #[getter]
    fn query_columns(&self) -> usize {
        self.snap.query_info.query_columns
    }

    /// Number of entity paths in the query request.
    #[getter]
    fn query_entities(&self) -> usize {
        self.snap.query_info.query_entities
    }

    /// Total size of all queried chunks in bytes (from chunk metadata).
    #[getter]
    fn query_bytes(&self) -> u64 {
        self.snap.query_info.query_bytes
    }

    /// Min number of chunks touched within any single segment in this query.
    #[getter]
    fn query_chunks_per_segment_min(&self) -> u32 {
        self.snap.query_info.query_chunks_per_segment_min
    }

    /// Max number of chunks touched within any single segment in this query.
    #[getter]
    fn query_chunks_per_segment_max(&self) -> u32 {
        self.snap.query_info.query_chunks_per_segment_max
    }

    /// Mean number of chunks touched per segment in this query.
    #[getter]
    fn query_chunks_per_segment_mean(&self) -> f32 {
        self.snap.query_info.query_chunks_per_segment_mean
    }

    /// Query shape: one of `"static"`, `"latest_at"`, `"range"`, `"dataframe"`, or `"full_scan"`.
    #[getter]
    fn query_type(&self) -> &'static str {
        self.snap.query_info.query_type.as_str()
    }

    /// Name of the sort/filter index (timeline) for this query, if any.
    #[getter]
    fn primary_index_name(&self) -> Option<&str> {
        self.snap.query_info.primary_index_name.as_deref()
    }

    /// Time from sending `query_dataset` until the first response message arrives
    /// (the chunk metadata, not actual chunk data).
    #[getter]
    fn time_to_first_chunk_info(&self) -> Option<Duration> {
        self.snap.query_info.time_to_first_chunk_info
    }

    /// Number of filter expressions the table provider was able to push down to
    /// the server (`Exact` or `Inexact` from `supports_filters_pushdown`).
    #[getter]
    fn filters_pushed_down(&self) -> usize {
        self.snap.query_info.filters_pushed_down
    }

    /// Number of filter expressions that could not be pushed down — applied
    /// client-side by DataFusion via a downstream `FilterExec`.
    #[getter]
    fn filters_applied_client_side(&self) -> usize {
        self.snap.query_info.filters_applied_client_side
    }

    /// True when projection-based entity-path narrowing actually trimmed the
    /// set of entity paths sent to `query_dataset`.
    #[getter]
    fn entity_path_narrowing_applied(&self) -> bool {
        self.snap.query_info.entity_path_narrowing_applied
    }

    // ---- Execution-time fields ----

    /// Wall-clock time from the start of `scan()` until the query finished
    /// (cleanly or via error). Always populated.
    #[getter]
    fn total_duration(&self) -> Duration {
        self.snap.total_duration
    }

    /// Time from scan start until the first chunk reached the consumer. `None`
    /// when no chunk was ever delivered (e.g. early error, empty result).
    #[getter]
    fn time_to_first_chunk(&self) -> Option<Duration> {
        self.snap.time_to_first_chunk
    }

    /// `None` on success. On failure, one of the stable string labels
    /// `"grpc_fetch"`, `"direct_fetch"`, `"decode"`, or `"other"`.
    #[getter]
    fn error_kind(&self) -> Option<&'static str> {
        self.snap.error_kind
    }

    /// Reason a direct (HTTP Range) fetch hit a terminal failure — i.e. a
    /// non-retryable error or retries exhausted. `None` when no direct fetch
    /// terminally failed (can be `None` even when `error_kind` is set, if the
    /// failure was on the gRPC or decode path).
    #[getter]
    fn direct_terminal_reason(&self) -> Option<&'static str> {
        self.snap.direct_terminal_reason.map(|r| r.as_str())
    }

    // ---- Fetch counters ----

    /// Number of gRPC fetch calls the scanner issued.
    #[getter]
    fn fetch_grpc_requests(&self) -> u64 {
        self.snap.fetch_grpc_requests
    }

    /// Sum of `chunk_byte_length` (catalog metadata, compressed on-disk size)
    /// over chunks fetched via gRPC. Excludes framing overhead and bytes
    /// consumed by failed retries — a lower bound on wire traffic.
    #[getter]
    fn fetch_grpc_bytes(&self) -> u64 {
        self.snap.fetch_grpc_bytes
    }

    /// Number of direct (HTTP Range) fetches the scanner issued. Counts each
    /// merged request once, regardless of byte ranges or retry attempts.
    #[getter]
    fn fetch_direct_requests(&self) -> u64 {
        self.snap.fetch_direct_requests
    }

    /// Sum of `chunk_byte_length` (catalog metadata, compressed on-disk size)
    /// over chunks fetched via direct HTTP. Does **not** count filler bytes
    /// that range-merging pulls between adjacent chunks, so actual wire
    /// traffic can exceed this value.
    #[getter]
    fn fetch_direct_bytes(&self) -> u64 {
        self.snap.fetch_direct_bytes
    }

    /// Total number of direct-fetch retry *attempts* across all requests.
    /// A request retried 3 times contributes 3 here.
    #[getter]
    fn fetch_direct_retries(&self) -> u64 {
        self.snap.fetch_direct_retries
    }

    /// Number of distinct direct-fetch requests that needed at least one
    /// retry. Always `≤ fetch_direct_retries`; the ratio between them is the
    /// average retries per retried request.
    #[getter]
    fn fetch_direct_requests_retried(&self) -> u64 {
        self.snap.fetch_direct_requests_retried
    }

    /// Total backoff time slept across all direct-fetch retries.
    #[getter]
    fn fetch_direct_retry_sleep(&self) -> Duration {
        self.snap.fetch_direct_retry_sleep
    }

    /// Sum of per-partition max attempts. For a single-partition query this
    /// is the true max; for multi-partition queries it is an upper bound on
    /// the true max — `MetricsSet::Count` has no `fetch_max` operation, so
    /// cross-partition aggregation sums.
    #[getter]
    fn fetch_direct_max_attempt(&self) -> u64 {
        self.snap.fetch_direct_max_attempt
    }

    /// Number of byte ranges the planner *wanted* to fetch directly, before
    /// adjacent ranges were coalesced. With `fetch_direct_merged_ranges`,
    /// gives the range-merging ratio.
    #[getter]
    fn fetch_direct_original_ranges(&self) -> u64 {
        self.snap.fetch_direct_original_ranges
    }

    /// Number of byte ranges actually issued after merging adjacent ranges
    /// into combined HTTP Range requests. Equals `fetch_direct_requests` for
    /// a single-range-per-request scanner.
    #[getter]
    fn fetch_direct_merged_ranges(&self) -> u64 {
        self.snap.fetch_direct_merged_ranges
    }

    fn __repr__(&self) -> String {
        let qi = &self.snap.query_info;
        format!(
            "QueryMetrics(dataset_id={:?}, query_type={}, query_chunks={}, query_segments={}, \
             query_bytes={}, filters_pushed_down={}, filters_applied_client_side={}, \
             entity_path_narrowing_applied={}, fetch_grpc_bytes={}, fetch_direct_bytes={}, \
             total_duration={:?}, \
             error_kind={:?})",
            qi.dataset_id,
            qi.query_type.as_str(),
            qi.query_chunks,
            qi.query_segments,
            re_format::format_bytes(qi.query_bytes as _),
            qi.filters_pushed_down,
            qi.filters_applied_client_side,
            qi.entity_path_narrowing_applied,
            re_format::format_bytes(self.snap.fetch_grpc_bytes as _),
            re_format::format_bytes(self.snap.fetch_direct_bytes as _),
            self.snap.total_duration,
            self.snap.error_kind,
        )
    }
}

/// Opaque PyO3 handle to a [`MetricsCollector`].
///
/// Held by the Python `query_metrics()` context manager between
/// `__enter__` and `__exit__`, and pushed onto the `_active_collectors`
/// `ContextVar` so `dataset_view.rs::reader()` can pick it up at plan time.
/// Cheap to clone — the inner `MetricsCollector` is itself an `Arc` wrapper,
/// so each attached `DataframeQueryTableProvider` and the Python-side handle
/// share the same buffer.
#[pyclass( // NOLINT: ignore[py-cls-eq] opaque handle — eq would be the same as `is`
    name = "_MetricsCollectorHandle",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyMetricsCollectorHandle {
    pub(crate) collector: MetricsCollector,
}

#[pymethods] // NOLINT: ignore[py-mthd-str] opaque handle
impl PyMetricsCollectorHandle {
    /// Non-destructive copy of all snapshots received so far.
    ///
    /// Suitable for use mid-scope (`collector.queries` in the Python wrapper).
    fn snapshot(&self) -> Vec<PyQueryMetrics> {
        self.collector
            .snapshot()
            .into_iter()
            .map(PyQueryMetrics::new)
            .collect()
    }

    /// Take and clear all snapshots.
    ///
    /// Used by the context manager on `__exit__` to drain any remaining
    /// snapshots into the user-visible Python `MetricsCollector` wrapper.
    fn drain(&self) -> Vec<PyQueryMetrics> {
        self.collector
            .drain()
            .into_iter()
            .map(PyQueryMetrics::new)
            .collect()
    }
}

/// Allocate a fresh [`MetricsCollector`] and wrap it in a Python handle.
///
/// The Python `query_metrics()` context manager pushes the returned handle
/// onto the `_active_collectors` `ContextVar` for the duration of the
/// `with` block; nothing is registered globally.
#[pyfunction]
#[pyo3(name = "_new_metrics_collector")]
pub fn new_metrics_collector() -> PyMetricsCollectorHandle {
    PyMetricsCollectorHandle {
        collector: MetricsCollector::new(),
    }
}

/// Read the `_active_collectors` `ContextVar` defined in
/// `rerun.experimental._query_metrics` and return the underlying Rust
/// collectors.
///
/// Returns an empty `Vec` when no `query_metrics()` scope is active, when the
/// module isn't importable, or when the ContextVar holds an unexpected value.
/// Failures here are never propagated: a broken metrics hookup should never
/// take down a user query.
pub fn active_metrics_collectors(py: Python<'_>) -> Vec<MetricsCollector> {
    let read = || -> PyResult<Vec<MetricsCollector>> {
        let module = py.import("rerun.experimental._query_metrics")?;
        let ctxvar = module.getattr("_active_collectors")?;
        let current = ctxvar.call_method0("get")?;
        let tuple = current.cast::<PyTuple>()?;
        let mut out = Vec::with_capacity(tuple.len());
        for item in tuple {
            let handle: PyRef<'_, PyMetricsCollectorHandle> = item.extract()?;
            out.push(handle.collector.clone());
        }
        Ok(out)
    };

    match read() {
        Ok(v) => v,
        Err(err) => {
            re_log::debug_once!("Failed to read query_metrics ContextVar: {err}");
            Vec::new()
        }
    }
}
