use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt as _;
use parking_lot::Mutex;
use pyo3::exceptions::{PyStopIteration, PyValueError};
use pyo3::{Py, PyRef, PyRefMut, PyResult, Python, pyclass, pymethods};
use re_arrow_util::{ArrowArrayDowncastRef as _, RecordBatchExt as _};
use re_protos::{
    cloud::v1alpha1::QueryTasksResponse,
    cloud::v1alpha1::ext::{QueryTasksOnCompletionResponse, RegisterWithDatasetTaskDescriptor},
    common::v1alpha1::TaskId,
};
use tokio::sync::mpsc;
use tracing::Instrument as _;

use super::{PyCatalogClientInternal, to_py_err};
use crate::utils::{get_tokio_runtime, wait_for_future};

/// Default timeout.
///
/// This is the timeout used when set to `None` on the Python side. The idea here is to mimic a
/// blocking for notebook/interactive uses of the SDK, but in practice it's never a thing, as the
/// server always ends up bailing to avoid blocking resources.
const DEFAULT_TIMEOUT_SECS: u64 = 60 * 60;

/// Result of a single registration task completion.
///
/// Tuple of (uri, `segment_id` or None, error or None). This is exposed as a
/// `SegmentRegistrationResult` dataclass on the Python side.
type RegistrationResult = (String, Option<String>, Option<String>);

/// Internal handle exposed to Python for tracking registration tasks.
#[pyclass( // NOLINT: ignore[py-cls-eq]
    name = "RegistrationHandleInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyRegistrationHandleInternal {
    client: Py<PyCatalogClientInternal>,
    descriptors: Vec<RegisterWithDatasetTaskDescriptor>,

    /// Map `task_id` -> indices in descriptors (multiple descriptors can share a `task_id`)
    ///
    /// Note: using vec index here is ok because this struct is essentially immutable, so
    /// out-of-bound errors are unlikely.
    task_id_to_indices: HashMap<String, Vec<usize>>,
}

impl PyRegistrationHandleInternal {
    /// Create a new registration handle from task descriptors.
    pub fn new(
        client: Py<PyCatalogClientInternal>,
        descriptors: Vec<RegisterWithDatasetTaskDescriptor>,
    ) -> Self {
        let mut task_id_to_indices: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, desc) in descriptors.iter().enumerate() {
            task_id_to_indices
                .entry(desc.task_id.id.clone())
                .or_default()
                .push(idx);
        }

        Self {
            client,
            descriptors,
            task_id_to_indices,
        }
    }

    fn task_ids(&self) -> Vec<TaskId> {
        self.task_id_to_indices
            .keys()
            .map(|id| TaskId { id: id.clone() })
            .collect()
    }
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyRegistrationHandleInternal {
    /// Returns a streaming iterator that yields (uri, segment_id, error) tuples
    /// as tasks complete.
    #[pyo3(signature = (timeout_secs=None))]
    fn iter_results(&self, py: Python<'_>, timeout_secs: Option<u64>) -> PyRegistrationIterator {
        let connection = self.client.borrow(py).connection().clone();
        let task_ids = self.task_ids();
        let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

        // Spawn a task that queries the completion state and channels it to the iterator object.
        let (tx, rx) = mpsc::unbounded_channel::<PyResult<Vec<RegistrationResult>>>();
        let descriptors = self.descriptors.clone();
        let task_id_to_indices = self.task_id_to_indices.clone();
        let runtime = get_tokio_runtime();
        runtime.spawn(
            async move {
                let mut client = match connection.client().await {
                    Ok(c) => c,
                    Err(err) => {
                        #[expect(clippy::let_underscore_must_use)]
                        let _ = tx.send(Err(err));
                        return;
                    }
                };

                let mut response_stream =
                    match client.query_tasks_on_completion(task_ids, timeout).await {
                        Ok(stream) => stream,
                        Err(err) => {
                            #[expect(clippy::let_underscore_must_use)]
                            let _ = tx.send(Err(to_py_err(err)));
                            return;
                        }
                    };

                while let Some(response) = response_stream.next().await {
                    let result = response
                        .map_err(to_py_err)
                        .and_then(|r| r.try_into().map_err(to_py_err))
                        .and_then(|r| process_task_response(r, &descriptors, &task_id_to_indices));

                    match result {
                        Ok(results) if !results.is_empty() => {
                            if tx.send(Ok(results)).is_err() {
                                // Receiver dropped, stop processing
                                break;
                            }
                        }

                        Ok(_) => {
                            // Empty batch, continue
                        }

                        Err(err) => {
                            let _ = tx.send(Err(err)).ok();
                            break;
                        }
                    }
                }
            }
            .in_current_span(),
        );

        PyRegistrationIterator {
            rx: Arc::new(Mutex::new(rx)),
            buffer: Vec::new(),
        }
    }

    /// Wait for all tasks to complete and return `segment_ids` in descriptor order.
    /// Raises an error if any registration fails.
    #[pyo3(signature = (timeout_secs=None))]
    fn wait(&self, py: Python<'_>, timeout_secs: Option<u64>) -> PyResult<Vec<String>> {
        let connection = self.client.borrow(py).connection().clone();
        let task_ids = self.task_ids();
        let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

        // Wait for all the tasks to complete and gather all errors. If any happened, we throw an
        // exception.
        let descriptors = self.descriptors.clone();
        let task_id_to_indices = self.task_id_to_indices.clone();
        wait_for_future(
            py,
            async move {
                let mut response_stream = connection
                    .client()
                    .await?
                    .query_tasks_on_completion(task_ids, timeout)
                    .await
                    .map_err(to_py_err)?;

                // Track errors by descriptor index
                let mut errors: HashMap<&RegisterWithDatasetTaskDescriptor, String> =
                    HashMap::new();

                while let Some(response) = response_stream.next().await {
                    let response = response.map_err(to_py_err)?.try_into().map_err(to_py_err)?;

                    let results =
                        process_task_response(response, &descriptors, &task_id_to_indices)?;

                    for (uri, _segment_id, error) in results {
                        if let Some(err) = error {
                            // Lookup the descriptor index for this URI
                            for desc in &descriptors {
                                if desc.storage_url.to_string() == uri {
                                    errors.insert(desc, err.clone());
                                }
                            }
                        }
                    }
                }

                // Check for any errors
                if !errors.is_empty() {
                    let error_msgs: Vec<String> = errors
                        .iter()
                        .map(|(desc, err)| format!("{}: {err}", desc.storage_url))
                        .collect();
                    return Err(PyValueError::new_err(format!(
                        "Registration failed for the following URIs:\n{}",
                        error_msgs.join("\n")
                    )));
                }

                Ok(descriptors
                    .iter()
                    .map(|d| d.segment_id.id.clone())
                    .collect())
            }
            .in_current_span(),
        )
    }
}

/// Process a single response from the task completion stream.
fn process_task_response(
    response: QueryTasksOnCompletionResponse,
    descriptors: &[RegisterWithDatasetTaskDescriptor],
    task_id_to_indices: &HashMap<String, Vec<usize>>,
) -> PyResult<Vec<RegistrationResult>> {
    let item = response.data;

    let projected = item
        .project_columns(
            [
                QueryTasksResponse::FIELD_TASK_ID,
                QueryTasksResponse::FIELD_EXEC_STATUS,
                QueryTasksResponse::FIELD_MSGS,
            ]
            .into_iter(),
        )
        .map_err(to_py_err)?;

    let (task_ids_col, statuses, msgs) = (
        projected
            .column(0)
            .try_downcast_array_ref::<arrow::array::StringArray>()
            .map_err(to_py_err)?,
        projected
            .column(1)
            .try_downcast_array_ref::<arrow::array::StringArray>()
            .map_err(to_py_err)?,
        projected
            .column(2)
            .try_downcast_array_ref::<arrow::array::StringArray>()
            .map_err(to_py_err)?,
    );

    let mut results = Vec::new();

    for i in 0..projected.num_rows() {
        let task_id = task_ids_col.value(i);
        let status = statuses.value(i);
        let msg = msgs.value(i);

        if let Some(indices) = task_id_to_indices.get(task_id) {
            for &idx in indices {
                let desc = &descriptors[idx];

                let segment_id = Some(desc.segment_id.id.clone());
                let error = (status != "success").then(|| msg.to_owned());

                results.push((desc.storage_url.to_string(), segment_id, error));
            }
        }
    }

    Ok(results)
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
    rx: Arc<Mutex<mpsc::UnboundedReceiver<PyResult<Vec<RegistrationResult>>>>>,

    /// Results are received in batches from gRPC, so we buffer them for the subsequent iterations.
    buffer: Vec<RegistrationResult>,
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyRegistrationIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<RegistrationResult> {
        // First check if we have buffered results
        if let Some(result) = slf.buffer.pop() {
            return Ok(result);
        }

        // Otherwise, wait for the next batch from the stream
        let rx = slf.rx.clone();

        // Release the GIL while waiting for data
        let batch_result = py.allow_threads(|| {
            let mut rx_guard = rx.lock();
            rx_guard.blocking_recv()
        });

        match batch_result {
            Some(Ok(mut results)) => {
                // Reverse first so pop() yields in FIFO order
                results.reverse();
                if let Some(first) = results.pop() {
                    slf.buffer = results;
                    Ok(first)
                } else {
                    Err(PyStopIteration::new_err(()))
                }
            }

            Some(Err(err)) => Err(err),

            None => {
                // Stream ended
                Err(PyStopIteration::new_err(()))
            }
        }
    }
}
