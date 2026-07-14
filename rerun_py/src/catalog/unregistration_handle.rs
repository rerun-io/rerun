// This whole module should really be shared with `registration_handle`,
// but there are result handling differences that make it impossible currently.
use pyo3::{Py, PyResult, Python, exceptions::PyValueError, pyclass, pymethods};
use re_protos::{
    cloud::v1alpha1::ext::{QueryTasksDataframe, QueryTasksOnCompletionResponse},
    common::v1alpha1::TaskId,
};
use re_redap_client::TraceId;
use tokio_stream::StreamExt as _;
use tracing::Instrument as _;

use crate::{
    catalog::{PyCatalogClientInternal, registration_handle::format_trace_ids, to_py_err},
    trace_context::read_trace_context_from_python,
    utils::wait_for_future,
};

const DEFAULT_TIMEOUT_SECS: u64 = 60 * 60;

/// Internal handle exposed to Python for tracking unregistration tasks.
#[pyclass(
    name = "UnregistrationHandleInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyUnregistrationHandleInternal {
    client: Py<PyCatalogClientInternal>,
    tasks: Vec<TaskId>,

    /// Trace-id of the request that created this handle.
    request_trace_id: Option<TraceId>,
}

impl PyUnregistrationHandleInternal {
    /// Create a new unregistration handle from task descriptors.
    pub fn new(
        client: Py<PyCatalogClientInternal>,
        tasks: Vec<TaskId>,
        request_trace_id: Option<TraceId>,
    ) -> Self {
        Self {
            client,
            tasks,
            request_trace_id,
        }
    }
}

#[pymethods]
impl PyUnregistrationHandleInternal {
    /// Wait for all tasks to complete.
    /// Raises an error if the unregistration fails.
    #[pyo3(signature = (timeout_secs=None))]
    fn wait(&self, py: Python<'_>, timeout_secs: Option<u64>) -> PyResult<()> {
        let span = read_trace_context_from_python(py, "UnregistrationHandle.wait");

        // This happens when running the SDK against an old server, which does not have asynchronous unregistration.
        if self.tasks.is_empty() {
            return Ok(());
        }

        let connection = self.client.borrow(py).connection().clone();
        let task_ids = self.tasks.clone();
        let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

        // Trace-id of the original registration request.
        let request_trace_id = self.request_trace_id;

        wait_for_future(
            py,
            async move {
                let mut response_stream = connection
                    .client()
                    .await?
                    .query_tasks_on_completion(task_ids, timeout)
                    .await
                    .map_err(to_py_err)?;

                // Trace-id of this completion query.
                let query_trace_id = response_stream.trace_id();

                let mut errors = Vec::new();

                while let Some(response) = response_stream.next().await {
                    let response: QueryTasksOnCompletionResponse =
                        response.map_err(to_py_err)?.try_into().map_err(to_py_err)?;

                    let on_err = |err| {
                        PyValueError::new_err(format!(
                            "invalid QueryTasks response dataframe: {err}"
                        ))
                    };
                    let task_ids = QueryTasksDataframe::COLUMN_TASK_ID
                        .extract(&response.data)
                        .map_err(on_err)?;
                    let statuses = QueryTasksDataframe::COLUMN_EXEC_STATUS
                        .extract(&response.data)
                        .map_err(on_err)?;
                    let msgs = QueryTasksDataframe::COLUMN_MSGS
                        .extract(&response.data)
                        .map_err(on_err)?;

                    for (task_id, status, msg) in itertools::izip!(&task_ids, &statuses, &msgs) {
                        let msg = msg.unwrap_or_default();

                        let error = match status {
                            "success" => None,
                            "cancelled" => Some("unregistration was cancelled".to_owned()),
                            _ => Some(msg.to_owned()),
                        };

                        if let Some(err) = error {
                            errors.push(format!("Unregistration task '{task_id}' failed: {err}"));
                        }
                    }
                }

                // Check for any errors
                if !errors.is_empty() {
                    return Err(PyValueError::new_err(format!(
                        "Unregistration failed.{}\n\nThe following segments failed:\n{}",
                        format_trace_ids(request_trace_id.as_ref(), query_trace_id.as_ref()),
                        errors.join("\n"),
                    )));
                }

                Ok(())
            }
            .instrument(span),
        )
    }

    /// Cancel unregistration.
    /// If the unregistration is already done, this is a noop.
    #[pyo3(signature = ())]
    fn cancel(&self, py: Python<'_>) -> PyResult<()> {
        let span = read_trace_context_from_python(py, "cancel");

        let connection = self.client.borrow(py).connection().clone();
        let task_ids = self.tasks.clone();

        wait_for_future(
            py,
            async move {
                connection
                    .client()
                    .await?
                    .cancel_tasks(task_ids)
                    .await
                    .map_err(to_py_err)?;

                Ok(())
            }
            .instrument(span),
        )
    }
}
