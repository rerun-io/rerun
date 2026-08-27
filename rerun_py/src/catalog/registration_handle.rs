use std::sync::Arc;

use futures::StreamExt as _;
use parking_lot::Mutex;
use pyo3::exceptions::{PyStopIteration, PyValueError};
use pyo3::{PyErr, PyRef, PyRefMut, PyResult, Python, pyclass, pymethods};
use re_redap_client::{RegistrationHandle, TraceId};
use tokio::sync::mpsc;
use tracing::Instrument as _;

use super::to_py_err;
use crate::trace_context::read_trace_context_from_python;
use crate::utils::{get_tokio_runtime, wait_for_future};

/// Default timeout.
///
/// This is the timeout used when set to `None` on the Python side. The idea here is to mimic a
/// blocking for notebook/interactive uses of the SDK, but in practice it's never a thing, as the
/// server always ends up bailing to avoid blocking resources.
const DEFAULT_TIMEOUT_SECS: u64 = 60 * 60;

/// Tuple of (URI, segment ID, error).
type RegistrationResult = (String, String, Option<String>);

/// Internal handle exposed to Python for tracking registration tasks.
#[pyclass(
    name = "RegistrationHandleInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyRegistrationHandleInternal {
    registration: RegistrationHandle,
}

impl PyRegistrationHandleInternal {
    pub fn new(registration: RegistrationHandle) -> Self {
        Self { registration }
    }
}

#[pymethods]
impl PyRegistrationHandleInternal {
    /// Returns a streaming iterator that yields (uri, segment_id, error) tuples
    /// as tasks complete.
    #[pyo3(signature = (timeout_secs=None))]
    fn iter_results(&self, py: Python<'_>, timeout_secs: Option<u64>) -> PyRegistrationIterator {
        let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

        let span = read_trace_context_from_python(py, "RegistrationHandle.iter_results");

        // Spawn a task that queries the completion state and channels it to the iterator object.
        let (tx, rx) = mpsc::channel::<PyResult<RegistrationResult>>(32 * 1024);
        let request_trace_id = self.registration.trace_id();
        let registration = self.registration.clone();
        let runtime = get_tokio_runtime();

        runtime.spawn(
            async move {
                // The query trace-id is already part of any `to_py_err` error message (it lives
                // on `ApiError`); here we additionally surface the original request trace-id.
                let with_trace_id = |err| prepend_request_trace_id(err, request_trace_id.as_ref());

                let mut response_stream = match registration.stream_results(timeout).await {
                    Ok(stream) => stream,
                    Err(err) => {
                        tx.send(Err(with_trace_id(to_py_err(err)))).await.ok();
                        return;
                    }
                };

                while let Some(response) = response_stream.next().await {
                    let result = response
                        .map(|result| {
                            (
                                result.recording_uri,
                                result.segment_id.to_string(),
                                result.error,
                            )
                        })
                        .map_err(|err| with_trace_id(to_py_err(err)));
                    let is_err = result.is_err();

                    if tx.send(result).await.is_err() || is_err {
                        break;
                    }
                }
            }
            .instrument(span),
        );

        PyRegistrationIterator {
            rx: Arc::new(Mutex::new(rx)),
        }
    }

    /// Wait for all tasks to complete and return `segment_ids` in descriptor order.
    /// Raises an error if any registration fails.
    #[pyo3(signature = (timeout_secs=None))]
    fn wait(&self, py: Python<'_>, timeout_secs: Option<u64>) -> PyResult<Vec<String>> {
        let span = read_trace_context_from_python(py, "RegistrationHandle.wait");

        let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

        wait_for_future(
            py,
            async move {
                self.registration
                    .wait(timeout)
                    .await
                    .map(|segment_ids| {
                        segment_ids
                            .into_iter()
                            .map(|segment_id| segment_id.to_string())
                            .collect()
                    })
                    .map_err(to_py_err)
            }
            .instrument(span),
        )
    }

    /// Cancel dataset registration.
    /// If the registration is already done, this is a noop.
    #[pyo3(signature = ())]
    fn cancel(&self, py: Python<'_>) -> PyResult<()> {
        let span = read_trace_context_from_python(py, "cancel");

        wait_for_future(
            py,
            async move { self.registration.cancel().await.map_err(to_py_err) }.instrument(span),
        )
    }
}

/// Prepend the original request trace-id to an error surfaced to Python.
///
/// The trace-id goes first so it stays visible ahead of the (potentially long and
/// private) error details. Returns the error unchanged when no trace-id is known.
fn prepend_request_trace_id(err: PyErr, request_trace_id: Option<&TraceId>) -> PyErr {
    match request_trace_id {
        Some(trace_id) => {
            PyValueError::new_err(format!("Registration request trace-id: {trace_id}\n{err}"))
        }
        None => err,
    }
}

/// Iterator that wraps the gRPC completion stream.
#[pyclass( // NOLINT: ignore[py-cls-eq]
    name = "RegistrationIterator",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyRegistrationIterator {
    /// Channel to receive results from the async stream.
    ///
    /// The arc-mutex here is needed because we release the GIL while polling the stream.
    rx: Arc<Mutex<mpsc::Receiver<PyResult<RegistrationResult>>>>,
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyRegistrationIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(slf: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<RegistrationResult> {
        let rx = slf.rx.clone();

        // Release the GIL while waiting for data.
        let result = py.detach(|| {
            let mut rx_guard = rx.lock();
            rx_guard.blocking_recv()
        });

        match result {
            Some(result) => result,
            None => Err(PyStopIteration::new_err(())),
        }
    }
}
