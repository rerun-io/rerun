use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use arrow::array::RecordBatch;
use futures::StreamExt as _;
use pyo3::exceptions::{PyStopIteration, PyValueError};
use pyo3::{Py, PyRef, PyRefMut, PyResult, Python, pyclass, pymethods};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_protos::cloud::v1alpha1::QueryTasksResponse;
use re_protos::cloud::v1alpha1::ext::RegisterWithDatasetTaskDescriptor;
use re_protos::common::v1alpha1::TaskId;
use re_protos::{invalid_schema, missing_field};
use re_redap_client::ApiError;
use tokio::sync::mpsc;
use tracing::Instrument as _;

use super::{PyCatalogClientInternal, to_py_err};
use crate::utils::{get_tokio_runtime, wait_for_future};

/// Default timeout: 8 hours.
const DEFAULT_TIMEOUT_SECS: u64 = 8 * 60 * 60;

/// Result of a single registration task completion.
///
/// Tuple of (uri, segment_id or None, error or None). This is exposed as a
/// `SegmentRegistrationResult` dataclass on the Python side.
type RegistrationResult = (String, Option<String>, Option<String>);

/// Internal handle exposed to Python for tracking registration tasks.
#[pyclass(
    name = "RegistrationHandleInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyRegistrationHandleInternal {
    client: Py<PyCatalogClientInternal>,
    descriptors: Vec<RegisterWithDatasetTaskDescriptor>,

    /// Map task_id -> indices in descriptors (multiple descriptors can share a task_id)
    ///
    /// Note: using vec index here is ok because this struct is essentially immutable, so
    /// out-of-bound errors are unlikely.
    task_id_to_indices: HashMap<String, Vec<usize>>,
}

impl PyRegistrationHandleInternal {
    /// Create a new registration handle from task descriptors.
    pub fn new(
        client: Py<PyCatalogClientInternal>,
        descriptors: impl IntoIterator<Item = RegisterWithDatasetTaskDescriptor>,
    ) -> Self {
        let descriptors: Vec<RegisterWithDatasetTaskDescriptor> = descriptors.into_iter().collect();

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

#[pymethods]
impl PyRegistrationHandleInternal {
    /// Returns a streaming iterator that yields (uri, segment_id, error) tuples
    /// as tasks complete.
    #[pyo3(signature = (timeout_secs=None))]
    fn iter_results(
        &self,
        py: Python<'_>,
        timeout_secs: Option<u64>,
    ) -> PyResult<PyRegistrationIterator> {
        let connection = self.client.borrow(py).connection().clone();
        let task_ids = self.task_ids();
        let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

        // Create a channel to receive results from the async stream
        let (tx, rx) = mpsc::unbounded_channel::<PyResult<Vec<RegistrationResult>>>();

        // Clone data needed for the async task
        let descriptors = self.descriptors.clone();
        let task_id_to_indices = self.task_id_to_indices.clone();

        // Spawn a task to process the stream
        let runtime = get_tokio_runtime();
        runtime.spawn(
            async move {
                let mut client = match connection.client().await {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                };

                let mut response_stream =
                    match client.query_tasks_on_completion(task_ids, timeout).await {
                        Ok(stream) => stream,
                        Err(e) => {
                            let _ = tx.send(Err(to_py_err(e)));
                            return;
                        }
                    };

                while let Some(response) = response_stream.next().await {
                    let batch_result =
                        process_task_response(response, &descriptors, &task_id_to_indices);

                    match batch_result {
                        Ok(results) if !results.is_empty() => {
                            if tx.send(Ok(results)).is_err() {
                                // Receiver dropped, stop processing
                                break;
                            }
                        }
                        Ok(_) => {
                            // Empty batch, continue
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            break;
                        }
                    }
                }
            }
            .in_current_span(),
        );

        Ok(PyRegistrationIterator {
            rx: Arc::new(Mutex::new(rx)),
            buffer: Vec::new(),
        })
    }

    /// Wait for all tasks to complete and return segment_ids in descriptor order.
    /// Raises an error if any registration fails.
    #[pyo3(signature = (timeout_secs=None))]
    fn wait(&self, py: Python<'_>, timeout_secs: Option<u64>) -> PyResult<Vec<String>> {
        let connection = self.client.borrow(py).connection().clone();
        let task_ids = self.task_ids();
        let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

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
                let mut errors: HashMap<usize, String> = HashMap::new();

                while let Some(response) = response_stream.next().await {
                    let results =
                        process_task_response(response, &descriptors, &task_id_to_indices)?;

                    // Record any errors
                    for (uri, _segment_id, error) in results {
                        if let Some(err) = error {
                            // Find the descriptor index for this URI
                            for (idx, desc) in descriptors.iter().enumerate() {
                                if desc.storage_url.to_string() == uri {
                                    errors.insert(idx, err.clone());
                                }
                            }
                        }
                    }
                }

                // Check for any errors
                if !errors.is_empty() {
                    let error_msgs: Vec<String> = errors
                        .iter()
                        .map(|(idx, err)| format!("{}: {}", descriptors[*idx].storage_url, err))
                        .collect();
                    return Err(PyValueError::new_err(format!(
                        "Registration failed for the following URIs:\n{}",
                        error_msgs.join("\n")
                    )));
                }

                // Return segment_ids in original order
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
    response: Result<re_protos::cloud::v1alpha1::QueryTasksOnCompletionResponse, tonic::Status>,
    descriptors: &[RegisterWithDatasetTaskDescriptor],
    task_id_to_indices: &HashMap<String, Vec<usize>>,
) -> PyResult<Vec<RegistrationResult>> {
    let item: RecordBatch = response
        .map_err(|err| {
            ApiError::tonic(
                err,
                "failed waiting for tasks: error receiving completion notifications",
            )
        })
        .map_err(to_py_err)?
        .data
        .ok_or_else(|| {
            let err = missing_field!(QueryTasksResponse, "data");
            let err = ApiError::serialization(
                err,
                "failed waiting for tasks: received item without data",
            );
            to_py_err(err)
        })?
        .try_into()
        .map_err(to_py_err)?;

    let schema = item.schema();
    if !schema.contains(&QueryTasksResponse::schema()) {
        let err = invalid_schema!(QueryTasksResponse);
        let err = ApiError::serialization(
            err,
            "failed waiting for tasks: received item with invalid schema",
        );
        return Err(to_py_err(err));
    }

    let col_indices = [
        QueryTasksResponse::FIELD_TASK_ID,
        QueryTasksResponse::FIELD_EXEC_STATUS,
        QueryTasksResponse::FIELD_MSGS,
    ]
    .iter()
    .map(|name| schema.index_of(name))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| {
        to_py_err(ApiError::serialization(
            err,
            "failed waiting for tasks: missing column on item",
        ))
    })?;

    let projected = item.project(&col_indices).map_err(to_py_err)?;

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
                let (segment_id, error) = if status == "success" {
                    (Some(desc.segment_id.id.clone()), None)
                } else {
                    (None, Some(msg.to_owned()))
                };
                results.push((desc.storage_url.to_string(), segment_id, error));
            }
        }
    }

    Ok(results)
}

/// Iterator that wraps the gRPC completion stream.
#[pyclass(
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

#[pymethods]
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
            let mut rx_guard = rx.lock().unwrap();
            rx_guard.blocking_recv()
        });

        match batch_result {
            Some(Ok(mut results)) => {
                if results.is_empty() {
                    // Empty batch, try again (shouldn't happen but be safe)
                    Err(PyStopIteration::new_err(()))
                } else {
                    // Return first result, buffer the rest in reverse order so pop() returns FIFO
                    results.reverse();
                    let first = results.pop().unwrap();
                    slf.buffer = results;
                    Ok(first)
                }
            }
            Some(Err(e)) => Err(e),
            None => {
                // Stream ended
                Err(PyStopIteration::new_err(()))
            }
        }
    }
}
