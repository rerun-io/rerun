// Unregistration only reports how its tasks ended. `registration_handle` maps the same tasks back
// to one result per segment.
use pyo3::{Py, PyResult, Python, pyclass, pymethods};
use re_protos::common::v1alpha1::TaskId;
use re_redap_client::TraceId;
use tracing::Instrument as _;

use crate::{
    catalog::{PyCatalogClientInternal, to_py_err},
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

        let connection = self.client.borrow(py).connection().clone();
        let task_ids = self.tasks.clone();
        let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

        // Trace-id of the original registration request.
        let request_trace_id = self.request_trace_id;

        wait_for_future(
            py,
            async move {
                connection
                    .client()
                    .await?
                    .wait_for_tasks(task_ids, request_trace_id, timeout)
                    .await
                    .map_err(to_py_err)?;
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
