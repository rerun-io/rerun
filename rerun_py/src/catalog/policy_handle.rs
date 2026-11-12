use pyo3::{Py, PyResult, Python, pyclass, pymethods};
use re_protos::common::v1alpha1::TaskId;
use tracing::Instrument as _;

use crate::{
    catalog::{PyCatalogClientInternal, to_py_err},
    trace_context::read_trace_context_from_python,
    utils::wait_for_future,
};

const DEFAULT_TIMEOUT_SECS: u64 = 60 * 60;

/// Internal handle exposed to Python for tracking the tasks that apply a dataset policy.
#[pyclass(
    name = "DatasetPolicyHandleInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyDatasetPolicyHandleInternal {
    client: Py<PyCatalogClientInternal>,
    tasks: Vec<TaskId>,
}

impl PyDatasetPolicyHandleInternal {
    pub fn new(client: Py<PyCatalogClientInternal>, tasks: Vec<TaskId>) -> Self {
        Self { client, tasks }
    }
}

#[pymethods]
impl PyDatasetPolicyHandleInternal {
    /// Wait for all tasks to complete. Raises an error if one fails.
    #[pyo3(signature = (timeout_secs=None))]
    fn wait(&self, py: Python<'_>, timeout_secs: Option<u64>) -> PyResult<()> {
        let span = read_trace_context_from_python(py, "DatasetPolicyHandle.wait");

        let connection = self.client.borrow(py).connection().clone();
        let task_ids = self.tasks.clone();
        let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

        wait_for_future(
            py,
            async move {
                connection
                    .client()
                    .await?
                    .wait_for_tasks(task_ids, None, timeout)
                    .await
                    .map_err(to_py_err)
            }
            .instrument(span),
        )
    }
}
